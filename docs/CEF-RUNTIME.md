# Runtime CEF: base, slots, updater

Cómo vive Chromium dentro de idioteque y qué hacer cuando cambia. El detalle
de nombres, rutas y mensajes está en [`cef/CONTRACT.md`](cef/CONTRACT.md).

## Qué es cada cosa

- **idioteque** es Tauri 2 + wry. No enlaza CEF. Solo lanza y habla con
  `cef-host`.
- **`cef-host`** es un binario aparte (`src-tauri/cef-host`) compilado contra
  el crate `cef` 152.3.0 (API de CEF `15200`). Carga la `libcef` del slot que
  se le indique y se dibuja como ventana hija X11 dentro de la ventana de
  idioteque. El mismo binario hace el health check. Solo Linux: no hay
  host ni pin para Windows o macOS.
- **Base**: el CEF de fábrica que viaja en cada release. Vive en el bundle
  (`<resource_dir>/cef/base/`). Hoy es
  `152.0.6+g708dc14+chromium-152.0.7977.83` (Chromium 152.0.7977.83).
- **`current`**: el motor last-good instalado por el updater en
  `~/.idioteque/cef/current/`. En un install fresco no existe: arranca el base.
- **`candidate`**: la descarga nueva. Nunca pisa `current` hasta pasar la
  prueba.

Un slot es un directorio completo (libcef, `.pak`, `locales/`, sandbox) con su
`manifest.json`. Nunca se mezclan archivos de dos versiones.

## Qué motor abre

Al abrir el navegador se elige el motor efectivo:

1. Si `~/.idioteque/cef/current` es válido y su versión es mayor que la del
   base bundleado, se usa `current`.
2. Si no, se usa el base. Un `current` igual o más viejo se borra.

Así, quien instala un idioteque nuevo con un base más nuevo arranca con ese
base, y el updater sigue subiendo desde ahí.

## Updater

- Se ejecuta 30 s después de arrancar y cada 24 h, o a mano desde
  Configuración → Navegador.
- Lee el índice oficial de CEF (`cef-builds.spotifycdn.com/index.json`), toma
  la stable más nueva de la plataforma (tipo `minimal`), mayor que el motor
  efectivo y no denylistada. Sin allowlist ni tope de milestone.
- Descarga a `candidate/` verificando tamaño y sha1 del índice. Extrae el
  paquete completo, stripea `libcef.so` (1.4 GB → 268 MB), escribe el
  manifest.
- Pre-check: el `CEF_API_VERSION_MIN` del candidate debe ser ≤ 15200. Si CEF
  retiró la API del host, ni se arranca.
- Health check: `cef-host --idq-health-check` con el candidate, sin ventana,
  carga `about:blank`. Pasa si el proceso vive, hace el handshake y sale 0.
- Pasa → promoción atómica (`current` → `current.old`, `candidate` →
  `current`, borrar `current.old`) y aviso "Se ha actualizado a Chromium XXX".
  Si el navegador estaba abierto, la promoción espera a que se cierre o al
  siguiente arranque.
- Falla → `current` no se toca, la versión entra en `denylist.json` y sale el
  aviso de incompatibilidad con las dos versiones (candidate y current). El
  aviso nunca dice "no compiló": el usuario no compiló nada.
- Descargas corruptas, hash malo o falta de disco no denylistan: se reintenta
  en el siguiente ciclo, sin aviso.

La denylist va firmada con `hostApiVersion`. Un release nuevo de idioteque
(host nuevo) la invalida entera: todo vuelve a probarse.

## Directorios

```
~/.idioteque/cef/
  current/           motor instalado (last-good)
  candidate/         descarga en curso o verificada
  current.old/       transitorio durante la promoción
  profile/           perfil de Chromium (cookies, storage)
  health-cache-N/    caché desechable de cada health check
  denylist.json  state.json  logs/
```

Overrides para desarrollo: `IDIOTEQUE_CEF_HOME`, `IDIOTEQUE_CEF_BASE_DIR`,
`IDIOTEQUE_CEF_HOST_BIN`, `IDIOTEQUE_CEF_INDEX_URL`,
`IDIOTEQUE_CEF_CHECK_INTERVAL_SECS`, `IDIOTEQUE_CEF_NO_SANDBOX=1`,
`IDIOTEQUE_CEF_ARGS` (switches extra de Chromium, p. ej. `--disable-gpu`).

## Cómo se bundlea el base

`bun run cef:prepare` (lo lanza `bun run tauri dev` antes de arrancar el CLI y
`beforeBuildCommand` en `tauri build`):

1. `cargo build -p cef-host --release`. El `build.rs` de `cef-dll-sys`
   descarga la dist pinneada a `src-tauri/.cef-sdk/` (una vez) y compila
   `libcef_dll_wrapper` (cmake + ninja).
2. Copia el binario a `src-tauri/binaries/cef-host-<triple>`
   (`bundle.externalBin`).
3. Copia el runtime desde la SDK a `src-tauri/cef-base/`, verifica el sha1 del
   tarball contra `src-tauri/cef/base.json`, stripea `libcef.so` y escribe el
   `manifest.json` (`bundle.resources` → `cef/base/`).

Nada de esto se commitea. Chromium no se compila nunca: `libcef.so` viene
precompilada del índice oficial; lo único que se compila en tu máquina es
`cef-host` y el wrapper C++ del SDK. Con la caché caliente (`.cef-sdk/`,
`target/release/cef-host`, `cef-base/manifest.json` con la versión y tamaños
correctos) el paso tarda segundos. El usuario final no descarga ni compila
nada: el instalador ya trae el base.

### `bun run tauri` es un envoltorio

`package.json` apunta `tauri` a `scripts/tauri.ts`, no al CLI directo:

- `bun run tauri dev` corre `cef:prepare` en primer plano y **después** lanza
  `tauri dev`. El CLI abandona si el dev server no responde en 180 s; con
  `cef:prepare` dentro de `beforeDevCommand` la primera descarga y compilación
  de CEF agotaba ese plazo y la app nunca arrancaba. `beforeDevCommand` ahora
  es solo `bun run dev`.
- `bun run tauri build` corre `tauri build` (deb y rpm, con CEF dentro vía
  `resources`/`externalBin`) y luego construye la AppImage en tres pasos:
  1. `tauri bundle --bundles appimage --config '{"bundle":{"resources":[],"externalBin":[]}}'`:
     la AppImage sale **sin** CEF. linuxdeploy hace `ldd` y `patchelf` de
     todo ELF bajo `usr/bin` y `usr/lib`; con CEF dentro fallaría con
     `libcef.so => not found` (cef-host la resuelve en runtime) y cambiaría
     el tamaño de `libcef.so`, `libEGL.so`, `chrome-sandbox`… invalidando el
     `manifest.json` del base.
  2. Inyecta `src-tauri/cef-base/` en `idioteque.AppDir/usr/lib/idioteque/cef/base/`
     y `cef-host` en `usr/bin/`, y comprueba los tamaños contra el manifest.
  3. Reempaqueta el AppDir con `linuxdeploy-plugin-appimage` (el mismo que el
     CLI cachea en `~/.cache/tauri/`; se descarga si falta, o se fija con
     `IDIOTEQUE_APPIMAGE_PLUGIN`). Solo construye el squashfs: no toca ELF.
  `--bundles` del usuario se respeta; la fase especial solo se aplica si la
  lista incluye `appimage`. `bundle.targets` en `tauri.conf.json` es
  `["deb", "rpm"]` a propósito: un `tauri build` sin el envoltorio no intenta
  la AppImage (fallaría).
- `src-tauri/build.rs` comprueba que existan `cef-base/manifest.json` y
  `binaries/cef-host-<triple>`: en release es error (un paquete sin navegador
  no debe salir por saltarse el pipeline), en debug solo aviso.

### rpm sin compresión

`bundle.linux.rpm.compression` es `none`. El CLI 2.11 empaqueta el rpm en
proceso con `rpm-rs 0.16`, cuyo gzip tarda decenas de minutos con los ~350 MB
del base (issues tauri-apps/tauri#11478 y #13273). Sin compresión termina en
el orden de un minuto; el rpm pesa el doble que el deb y se acepta.

### Dependencias del sistema

`libcef.so` necesita librerías que no vienen con webkit2gtk/gtk3: NSS/NSPR,
ALSA, DRM/GBM, dbus, cups y varias X11. `tauri.conf.json` las declara en
`bundle.linux.deb.depends` (nombres clásicos de paquete; Ubuntu 24.04+ hace
`Provides` de ellos desde los `t64`) y en `bundle.linux.rpm.depends` como
`Requires` por soname (`libnss3.so()(64bit)`…), que resuelven dnf y zypper en
cualquier distro. `src/lib/linux-bundle-deps.test.ts` exige que las dos listas
cubran los mismos sonames. La AppImage no declara dependencias: linuxdeploy
bundlea gtk y compañía, el resto está en la excludelist oficial (glibc, mesa,
X11, fontconfig, alsa…) y solo asume NSS/NSPR (`libnss3`) del sistema, igual
que Electron y Chrome; está en cualquier escritorio con navegador.

### Navegador embebido: X11 / XWayland y sandbox

El embed es una ventana hija X11. En GNOME Wayland (Ubuntu 26.04 no ofrece
sesión Xorg) Mutter sigue levantando XWayland: idioteque fija
`GDK_BACKEND=x11` si hay `DISPLAY` y CEF usa `--ozone-platform=x11` contra
ese mismo display. No es embed Wayland nativo.

Foco X11 entre el toplevel y el hijo CEF: cefclient GTK envía
`WM_TAKE_FOCUS` al toplevel cuando la barra de URL reclama el teclado
(workaround GTK+X11, cefclient #3782). idioteque no usa ese ClientMessage
para la barra: `browser_focus_app` suelta el grab X11 del ADE, hace
`XSetInputFocus` sobre el toplevel, `grab_focus` del webview wry y manda
`{"cmd":"unfocus"}` (`set_focus(false)` + `XUngrabKeyboard` en el display
del host, que es quien tiene el grab de Ozone). `CefFocusHandler`
avisa cuando el hijo gana (`owner=browser`) o cede (`owner=app`) el foco.
Un clic de página con el chrome dueño de las teclas no espera a
`OnGotFocus` (Ozone a menudo no lo manda): `OnSetFocus` / X11
`ButtonPress`/`FocusIn` en el xid del hijo dispara un `activate`
(`set_focus(true)`, **sin** `XSetInputFocus`) y `focus owner=browser`,
solo esa primera vez. El ADE no hace `grab_focus` del webview en ese
clic. En este embed el hijo Ozone **no** toma el InputFocus de X11 (`getwindowfocus`
sigue en el toplevel aunque el caret esté en la página); las teclas llegan
a CEF por GTK. El `.host` de BrowserView tiene `pointer-events: none`
mientras `browser.alive` (el placeholder sí recibe eventos al arrancar).
Alloy nativo recicla Tab dentro del HTML, así que
`OnTakeFocus` casi nunca dispara: el host consume Tab en `on_pre_key_event`
y pregunta al renderer (`__idiotequeHandleTab`); si el activo es el
primero o el último, avisa con `idioteque://chrome/take-focus?next=` y
`console.info('idioteque:take-focus:')`. El trap se inyecta en
`on_context_created` (renderer, main frame) y otra vez en `on_load_start`.
Ctrl+L lo captura `on_pre_key_event` (`RAWKEYDOWN` o `KEYDOWN`); el wry
no ve el acorde mientras CEF tiene el caret. Tras `unfocus`, el host
traga CHAR de página y los reenvía como `{"event":"keys"}` para la
barra. No se elimina el manejo de `WM_TAKE_FOCUS` si Chromium lo entrega;
no es el camino de la barra.

`chrome-sandbox` solo se exporta como `CHROME_DEVEL_SANDBOX` si es setuid-root.
En `tauri dev` el helper es del usuario; en la AppImage el squashfs no puede
ser setuid. En Ubuntu 24.04+ (`apparmor_restrict_unprivileged_userns=1`)
los user namespaces existen pero AppArmor transiciona `unconfined` al
perfil `unprivileged_userns` y le quita `CAP_SYS_ADMIN`; Chromium aborta
con “No usable sandbox!”. El ADE no se fía de `unshare -U`: prueba
`CLONE_NEWUSER` + `CLONE_NEWPID` en un hijo (o, si el sysctl está activo
y el perfil es `unconfined`, ni siquiera forkea). Sin helper ni userns
usable el primer `cef-host` ya va con `--idq-no-sandbox`. No es un error
ni un crash. Si el probe se equivoca, queda un reintento (el `fatal` de
esa primera muerte no llega a la UI). El stderr del host queda en
`logs/cef-host.log`; el `log_file` de Chromium, en `logs/chromium.log`
(Chromium lo trunca al arrancar: no pueden ser el mismo archivo). Un
sandbox de verdad (helper `4755` o perfil AppArmor `userns` para
`cef-host`; ver
[Chromium](https://chromium.googlesource.com/chromium/src/+/main/docs/security/apparmor-userns-restrictions.md))
es cosa del deb/rpm, no de este runtime.

### Memoria compartida: `/dev/shm`, no `/tmp`

Chromium crea su memoria compartida como ficheros en `/dev/shm`, como
Chrome. `--disable-dev-shm-usage` la manda a `$TMPDIR`/`/tmp` y solo tiene
sentido en contenedores con `/dev/shm` de 64 MiB (Docker, la VM de Cursor
Cloud). En Ubuntu con systemd ≥ 258, `/tmp` es un tmpfs con `usrquota` y
cada usuario tiene un tope del 80 %: si algo tuyo llena `/tmp` (una cache de
compilación, por ejemplo), toda escritura da `EDQUOT` aunque `df` muestre
espacio libre, y con el switch puesto Chromium se queda sin memoria
compartida: `TransferBuffer::Initialize() failed`, renderers que mueren,
`ERR_INSUFFICIENT_RESOURCES` y un `FATAL` del font service que mata el
host. Por eso `cef-host` no ata el switch al sandbox: hace un probe real
(reservar 128 MiB en un fichero borrado) en `/dev/shm`, después en el temp
dir y, si ninguno sirve, usa `TMPDIR=<cache>/shm` en disco. La decisión
está en `logs/cef-host.log` (`cef-host: shm DevShm`). Por la misma razón,
`bun run tauri build` corre con `TMPDIR=src-tauri/target/tmp` (salvo que ya
traigas uno): el staging del deb/rpm y la extracción del plugin de AppImage
no pasan por `/tmp`.

## Cómo migrar el base en un release

Cuando una CEF nueva rompe el last-good y adaptas el host:

1. Sube el crate `cef` en `src-tauri/cef-host/Cargo.toml` a la versión que
   corresponde a la CEF nueva (el sufijo `+X.Y.Z` del crate es la versión de
   CEF).
2. Actualiza `src-tauri/cef/base.json`: `cefVersion`, `chromiumVersion`,
   `hostApiVersion` (el `CEF_API_VERSION_LAST` de esa dist), `apiVersionMin`
   y los `name`/`sha1`/`size` de cada plataforma tal como aparecen en
   `index.json`.
3. Adapta `cef-host` a los cambios de API si los hay.
4. `bun run test` (hay un test que exige que `base.json` y `Cargo.lock`
   coincidan) y `bun run tauri build`.

Quien instale ese release arranca con el base nuevo y su denylist vieja deja
de aplicar.

## Si dejas de mantener el proyecto

El usuario se queda en su last-good, con el navegador funcionando. Un build
viejo sigue pudiendo actualizar el motor mientras las CEF nuevas soporten la
API con la que se compiló su host; cuando dejen de hacerlo, el health check
las rechaza y el motor se queda donde estaba.
