# Contrato del navegador CEF

Documento de referencia para todos los paquetes del navegador. Si algo de aquí
cambia, se cambia primero aquí y después en el código. Los nombres, rutas,
mensajes y códigos de salida de este archivo son los que usan las pruebas.

## 1. Piezas y fronteras

- **ADE (Tauri 2 + wry)**: la app idioteque de siempre. No enlaza `libcef`.
  Solo spawnea y habla con `cef-host`. Código en `src-tauri/src/cef/`.
- **`cef-host`**: binario aparte (crate `src-tauri/cef-host`, sin dependencias
  de Tauri) que enlaza el crate `cef` 152.3.0 (API version `15200`), carga la
  `libcef` del slot que le indiquen y crea un browser embebido como ventana
  hija X11 de la ventana de idioteque. También hace el health check.
- **Frontend (Svelte 5)**: la vista `browser`, la barra de navegación, los
  atajos y los avisos. Habla con el ADE por `invoke` y `Channel`.

Regla: el ADE nunca importa el crate `cef`; `cef-host` nunca importa `tauri`.

## 2. Plataformas

Clave de plataforma (la misma del índice oficial de CEF):

- `linux64` (implementación completa en esta entrega)
- `linuxarm64`, `windows64`, `macosx64`, `macosarm64` (preparadas, sin implementar)

Se obtiene con `target_os` + `target_arch` en Rust. Todo lo específico de
plataforma en `cef-host` vive en `src/platform/{linux,windows,macos}.rs`.

## 3. Runtime CEF en disco

### 3.1 Base bundleado (solo lectura)

`<resource_dir>/cef/base/` (en dev: `src-tauri/target/debug/cef/base/`). Lo
produce `scripts/cef-prepare.ts` a partir de `src-tauri/cef/base.json` y lo
empaqueta `tauri build` vía `bundle.resources`. Contiene el layout plano de la
sección 3.3 y su `manifest.json` con `source: "bundled"`.

En deb y rpm eso es `/usr/lib/idioteque/cef/base/`. En la AppImage la ruta es
la misma (`$APPDIR/usr/lib/idioteque/cef/base/`) pero no la pone Tauri:
`scripts/tauri.ts` bundlea la AppImage sin CEF (linuxdeploy parchearía las
libs y rompería los tamaños del manifest) y luego inyecta `cef-base/` y
`cef-host` en el AppDir antes de reempaquetarlo. El sidecar se busca junto a
`std::env::current_exe()` (`$APPDIR/usr/bin/`), no junto al `.AppImage`.

Override en desarrollo: `IDIOTEQUE_CEF_BASE_DIR=<dir>`.

### 3.2 Directorio del usuario

`~/.idioteque/cef/` (override: `IDIOTEQUE_CEF_HOME=<dir>`):

- `current/` — motor instalado por el updater (last-good). Ausente en un
  install fresco.
- `candidate/` — descarga en curso, o verificada y pendiente de promoción.
- `current.old/` — transitorio durante la promoción.
- `profile/` — `root_cache_path` de Chromium (cookies, storage, caché).
- `health-cache-<pid>/` — caché desechable de cada health check. Se borra al
  terminar y al arrancar la app.
- `denylist.json`, `state.json`, `logs/cef-host.log` (stderr del host,
  append), `logs/chromium.log` (`log_file` de CEF; Chromium lo trunca al
  arrancar), `logs/updater.log`.

### 3.3 Layout plano de un slot

Un slot (base, `current` o `candidate`) es un directorio completo, nunca se
mezclan archivos de dos versiones:

```
<slot>/
  manifest.json
  libcef.so  chrome-sandbox  libEGL.so  libGLESv2.so
  libvk_swiftshader.so  libvulkan.so.1  vk_swiftshader_icd.json
  v8_context_snapshot.bin
  resources.pak  chrome_100_percent.pak  chrome_200_percent.pak  icudtl.dat
  locales/*.pak
  LICENSE.txt
  include/cef_api_versions.h   (para leer MIN/LAST)
  include/cef_version.h
```

Archivos obligatorios en linux64 (falta alguno → slot inválido):
`libcef.so`, `icudtl.dat`, `v8_context_snapshot.bin`, `resources.pak`,
`chrome_100_percent.pak`, `chrome_200_percent.pak`, `locales/en-US.pak`,
`libEGL.so`, `libGLESv2.so`, `libvk_swiftshader.so`, `libvulkan.so.1`,
`vk_swiftshader_icd.json`, `chrome-sandbox`.

`libcef.so` se distribuye stripeado (`strip --strip-all` o `elf_strip`):
1.43 GB → ~268 MB.

### 3.4 `manifest.json` de un slot

```json
{
  "schema": 1,
  "cefVersion": "152.0.6+g708dc14+chromium-152.0.7977.83",
  "chromiumVersion": "152.0.7977.83",
  "platform": "linux64",
  "apiVersionMin": 13300,
  "apiVersionLast": 15200,
  "source": "bundled",
  "archiveName": "cef_binary_..._linux64_minimal.tar.bz2",
  "archiveSha1": "9711b86c105fb590da576fe5a829802f1a79d520",
  "archiveSize": 321503907,
  "stripped": true,
  "files": [{ "path": "libcef.so", "size": 267902104, "sha256": "…" }],
  "verified": false,
  "verifiedAt": null,
  "createdAt": "2026-09-16T23:00:00Z"
}
```

`source` es `"bundled"` o `"downloaded"`. `verified` solo es `true` en un
`candidate` que pasó el health check y espera promoción. `files` lista todos
los archivos del slot menos `manifest.json`.

### 3.5 Resolución del motor efectivo (`current`)

1. Leer el base bundleado (siempre debe existir; si falta, error fatal del
   navegador, no de la app).
2. Leer `~/.idioteque/cef/current/manifest.json`. Es válido si parsea, la
   plataforma coincide y todos los archivos obligatorios existen con el tamaño
   del manifest.
3. Si el instalado es válido y `cefVersion` del instalado > `cefVersion` del
   base → **instalado**. Si no → **base**, y si había un `current` igual o más
   viejo, se borra en segundo plano.

Orden de versiones: se compara la tupla `MAJOR.MINOR.PATCH` que va antes del
primer `+`; en empate, la versión de Chromium (cuatro números).

### 3.6 `denylist.json`

```json
{
  "schema": 1,
  "hostApiVersion": 15200,
  "entries": [
    {
      "cefVersion": "153.0.1+gabc+chromium-153.0.8000.10",
      "chromiumVersion": "153.0.8000.10",
      "reason": "health-exit-10",
      "at": "2026-10-01T10:00:00Z"
    }
  ]
}
```

Al cargar, si `hostApiVersion` no coincide con el del host actual, la lista se
descarta entera (release nuevo de idioteque = host nuevo = todo vuelve a
probarse). Motivos posibles: `api-version-min-above-host`, `health-exit-<n>`,
`health-timeout`, `health-crashed`, `health-no-handshake`.

Nunca se denylista por descargas corruptas, hash malo, falta de disco o fallo
de strip: eso se reintenta en el siguiente ciclo, sin aviso.

### 3.7 `state.json`

```json
{
  "schema": 1,
  "lastCheckAt": "2026-09-16T23:00:00Z",
  "indexEtag": "\"72725877b82897a3019cd6aabf3b23a1\"",
  "pendingPromotion": null,
  "lastOutcome": "no-newer"
}
```

`pendingPromotion` guarda `{ "cefVersion", "chromiumVersion" }` cuando el
candidate pasó el health check pero había un `cef-host` vivo.

## 4. `cef-host`

### 4.1 Argumentos

Prefijo `--idq-` para no chocar con switches de Chromium. `execute_process`
corre antes de parsear (los subprocesos `--type=renderer` no pasan por aquí).
Argumentos desconocidos se ignoran.

- `--idq-cef-dir <dir>` slot a cargar (obligatorio).
- `--idq-cache-dir <dir>` `root_cache_path` (obligatorio).
- `--idq-parent <xid>` ventana X11 padre (el hueco GDK del ADE, sección 5).
  El host cuelga de ella una ventana intermedia con el visual y colormap por
  defecto del servidor (el hueco lleva el visual GL de GTK y Chromium crea su
  ventana con el visual por defecto: sin la intermedia `CreateWindow` da
  `BadMatch`) y CEF cuelga de la intermedia en `(0, 0)`. Sin `--idq-parent`,
  ventana top-level propia (modo standalone para pruebas).
- `--idq-bounds x,y,w,h` en píxeles físicos relativos al padre.
- `--idq-scale <f>` device scale factor (se pasa como
  `--force-device-scale-factor`).
- `--idq-url <url>` URL inicial (por defecto `about:blank`).
- `--idq-health-check` modo health check (sección 4.5).
- `--idq-no-sandbox` pasa `no_sandbox = 1`.
- `--idq-log <file>` `log_file` de CEF (Chromium lo trunca al arrancar).
  El ADE lo apunta a `logs/chromium.log`; el stderr del host
  (`cef-host fatal …`) va aparte a `logs/cef-host.log`.
- `--idq-info` imprime `{"event":"info","apiVersion":15200,"cefCompiled":"152.0.6+…"}`
  y sale 0, sin inicializar CEF.

Variables de entorno que pone el ADE al spawnear: `LD_LIBRARY_PATH=<slot>`
(delante de lo que ya hubiera). `CHROME_DEVEL_SANDBOX=<slot>/chrome-sandbox`
solo si ese helper es setuid-root (`uid 0` y bit `0o4000`); si no, se quita
aunque viniera heredado (un helper `755` hace que Chromium haga FATAL en vez
de usar user namespaces). En la AppImage el squashfs no puede ser setuid.

Antes de spawnear, el ADE decide el sandbox: helper setuid-root **o** user
namespaces *usables*. El probe no es `unshare -U` (sale 0 bajo AppArmor de
Ubuntu 24.04+ aunque el ns no sirva): en un hijo hace `unshare(CLONE_NEWUSER)`
y después `unshare(CLONE_NEWPID)`, que exige `CAP_SYS_ADMIN` dentro del ns
(lo que Chromium necesita para el zygote). Atajo: si
`apparmor_restrict_unprivileged_userns=1` y el perfil es `unconfined`,
el ADE no forkea — cef-host hereda el mismo confinamiento. Si no hay
helper ni userns usable, o si `IDIOTEQUE_CEF_NO_SANDBOX=1`, el primer
lanzamiento ya lleva `--idq-no-sandbox` (`BrowserBoot.noSandbox = true`).
No es un error: en `tauri dev` y en la AppImage no hay setuid. El health
check y el updater usan el mismo predicado (un candidate no se denylista
por falta de sandbox). Quien quiera sandbox real en el paquete: helper
`4755` o un perfil AppArmor propio (ver
[Chromium: AppArmor userns restrictions](https://chromium.googlesource.com/chromium/src/+/main/docs/security/apparmor-userns-restrictions.md)).

Si el probe se equivoca y el host sale `15` / `11` / abort `1` sin `ready`,
el ADE reintenta **una** vez con `--idq-no-sandbox`. El `fatal` de ese
primer intento no llega al frontend (Svelte se queda en “Arrancando
Chromium…”). Un exit `10` / `13` / `14` / `16` no se reintenta: ahí sí se
reenvía el `fatal` pendiente y el `exit`.

Variables opcionales: `IDIOTEQUE_CEF_ARGS` (switches extra de Chromium
separados por espacio, p. ej. `--disable-gpu`), `IDIOTEQUE_CEF_NO_SANDBOX=1`,
`IDIOTEQUE_CEF_SOFTWARE_GL=1` (ANGLE/SwiftShader por software, para servidores X
sin DRI3 como la VM de desarrollo; una máquina sin sandbox conserva su GPU).

**Memoria compartida.** Chromium no usa `memfd`: cada región (transfer
buffers de la GPU, data pipes de Mojo, fuentes) es un fichero que crea y
borra en `/dev/shm`, o en `$TMPDIR`/`/tmp` si lleva
`--disable-dev-shm-usage`. Ese switch es un parche para contenedores con
`/dev/shm` de 64 MiB y **no depende de `no_sandbox`**. El host lo decide
antes de `initialize` con un probe real (`shm.rs`): crea un fichero, lo
borra y reserva 128 MiB con `fallocate`, lo que detecta permisos, tamaño y
cuota por usuario (un tmpfs con `usrquota`, systemd ≥ 258, devuelve
`EDQUOT` aunque `df` diga que sobra sitio). Orden: `/dev/shm` → sin switch
(lo mismo que Chrome); si no sirve, el temp dir → `--disable-dev-shm-usage`;
si tampoco, `--disable-dev-shm-usage` + `TMPDIR=<cache-dir>/shm` en disco,
que los subprocesos heredan. La decisión queda en stderr
(`cef-host: shm DevShm|TempDir(..)|CacheDir(..)`) y los fallos del probe
justo antes. Sin memoria compartida los renderers mueren
(`render-crashed`), la red da `ERR_INSUFFICIENT_RESOURCES` y un `CHECK` del
font service tumba el proceso browser. `IDIOTEQUE_CEF_ARGS=--disable-dev-shm-usage`
sigue forzándolo.

### 4.2 Arranque

1. `dup(1)` → fd del protocolo; `dup2(2, 1)` para que Chromium y sus
   subprocesos escriban todo su ruido a stderr. El protocolo sale por el fd
   duplicado, una línea JSON por mensaje, `\n` final, sin nada más.
2. Si `--idq-info` → imprimir y salir 0.
3. `api_hash(CEF_API_VERSION_LAST, 0)`; `execute_process` (si devuelve ≥ 0,
   somos subproceso: salir con ese código).
4. Verificar el slot: `manifest.json` parsea, archivos obligatorios presentes
   (si no → exit `14`). `version_info()` de la libcef cargada debe coincidir
   con `manifest.cefVersion` (si no → exit `13`).
5. `Settings`: `resources_dir_path = slot`, `locales_dir_path = slot/locales`,
   `root_cache_path = --idq-cache-dir`, `browser_subprocess_path =
   current_exe`, `log_file`, `log_severity = WARNING`, `background_color`
   opaco oscuro (`0xFF1C1E22`), `windowless_rendering_enabled = 1` solo en
   health check, `no_sandbox` según flag/env.
6. `initialize` falla → exit `11`. Si el fallo es del sandbox (mensaje de
   Chromium sobre SUID/namespaces en stderr, o `initialize` falla con
   sandbox activo y el ADE lo reintenta) → exit `15`. Un abort (SIGABRT)
   porque `CHROME_DEVEL_SANDBOX` apuntaba a un helper inválido llega al ADE
   como `Exit` con código `1`.
7. Sin `DISPLAY`/sin X11 → exit `16`.

### 4.3 Protocolo host → ADE (stdout)

Cada mensaje es un objeto JSON con `event`:

- `{"event":"ready","cef":"152.0.6+…","chromium":"152.0.7977.83","apiVersion":15200,"xid":123456}`
  — una vez, tras `on_after_created` del browser principal.
- `{"event":"nav","url":"https://…","canGoBack":true,"canGoForward":false,"loading":true}`
  — en `on_loading_state_change` y `on_address_change`.
- `{"event":"title","title":"…"}`
- `{"event":"load-end","status":200}`
- `{"event":"load-error","code":-105,"text":"ERR_NAME_NOT_RESOLVED","url":"…"}`
  (no se emite para `ERR_ABORTED`).
- `{"event":"shortcut","chord":"ctrl+b"}` — chords reenviados: `ctrl+b`,
  `ctrl+shift+b`, `ctrl+l`. El host los consume (no llegan a la página).
- `{"event":"focus","owner":"browser"}` — `CefFocusHandler::OnGotFocus` del
  browser principal. El ADE/Svelte hace blur del input de la barra para que
  solo CEF reciba teclas.
- `{"event":"focus","owner":"app","next":true}` — `OnTakeFocus`: Tab (o
  Shift+Tab con `next:false`) salió de la página. El ADE enfoca la URL o el
  último control de la barra y reclama X11.
- `nav`, `title`, `load-end`, `load-error` y `focus` solo se emiten para el
  browser principal: la ventana de DevTools y otros popups no alimentan la
  barra.
- `{"event":"render-crashed","status":"…"}`
- `{"event":"health","ok":true,"cef":"…","chromium":"…","apiVersion":15200}`
  — solo en modo health check, justo antes de salir 0.
- `{"event":"fatal","message":"…","code":11}` — justo antes de salir ≠ 0.
- `{"event":"info","apiVersion":15200,"cefCompiled":"152.0.6+…"}` — solo con `--idq-info`.

### 4.4 Protocolo ADE → host (stdin)

Objetos JSON con `cmd`, una por línea. El hilo lector los pasa al hilo UI de
CEF con `post_task`.

- `{"cmd":"navigate","url":"…"}`
- `{"cmd":"back"}`, `{"cmd":"forward"}`, `{"cmd":"stop"}`
- `{"cmd":"reload","ignoreCache":false}`
- `{"cmd":"set_bounds","x":0,"y":0,"w":1200,"h":700}` (píxeles físicos,
  relativos al hueco: el ADE ya colocó el hueco en coordenadas lógicas)
- `{"cmd":"show"}`, `{"cmd":"hide"}` — `XMapWindow`/`XUnmapWindow` +
  `was_hidden(false/true)`; tras `show`, `XRaiseWindow`.
- `{"cmd":"focus"}` — `XSetInputFocus` + `set_focus(true)`.
- `{"cmd":"unfocus"}` — `set_focus(false)` únicamente. **No** llama
  `XSetInputFocus`: el ADE ya movió el foco X11 al toplevel.
- `{"cmd":"devtools"}` — abre DevTools si no está, lo cierra si está.
- `{"cmd":"close"}` — cierre ordenado: `close_browser(true)`, `quit_message_loop`,
  `shutdown`, exit 0.

EOF en stdin = el ADE murió → cierre ordenado y exit 0.

### 4.5 Modo health check

`cef-host --idq-health-check --idq-cef-dir <candidate> --idq-cache-dir <health-cache-N>`:

- Sin ventana: `windowless_rendering_enabled = 1`,
  `WindowInfo::set_as_windowless(0)`, `RenderHandler` mínimo
  (`get_view_rect` 800×600, `on_paint` vacío).
- Carga `about:blank`. En `on_load_end` del frame principal emite
  `{"event":"health","ok":true,…}`, cierra y sale `0`.
- Watchdog interno de 30 s → exit `12`.
- El ADE espera como máximo 45 s. Pasa si y solo si: hubo `health ok:true`,
  el proceso salió y el código fue `0`.

### 4.6 Teclado dentro de CEF

`on_pre_key_event` con `KEYEVENT_RAWKEYDOWN`:

- `Ctrl+B`, `Ctrl+Shift+B`, `Ctrl+L` → emitir `shortcut` y consumir.
- `F12`, `Ctrl+Shift+I` → DevTools (toggle) y consumir.
- `F5`, `Ctrl+R` → reload; `Ctrl+Shift+R` → reload ignorando caché.
- `Alt+←` / `Alt+→` → back / forward.
- `Escape` → stop.
- Todo lo demás pasa a la página.

Menú contextual: los items por defecto de CEF más "Inspeccionar" (abre
DevTools en el punto del clic) y "Recargar".

Popups (`on_before_popup`): se cancelan y la URL se carga en el frame
principal (una sola pestaña). DevTools sí abre su ventana propia.

Un solo dueño de teclado: o el chrome wry/Svelte o el hijo CEF, nunca los
dos. `browser_focus_app` hace `XSetInputFocus(toplevel)` y después
`unfocus`. Un clic en la página emite `focus owner=browser` y el frontend
hace blur del campo URL. `{"cmd":"focus"}` entrega a CEF tanto X11 como
`set_focus(true)`.

### 4.7 Códigos de salida

- `0` ok (incluye cierre ordenado y health check correcto)
- `10` API incompatible (`cef_api_hash` rechazó `15200`, o
  `apiVersionMin` del slot > `15200`)
- `11` `initialize` falló
- `12` timeout interno del health check
- `13` `version_info()` de la libcef ≠ `manifest.cefVersion`
- `14` archivos obligatorios ausentes o `manifest.json` inválido
- `15` sandbox no disponible
- `16` sin X11 (`DISPLAY` vacío o conexión fallida)
- `2` argumentos inválidos

## 5. Comandos Tauri (ADE)

Todos devuelven `Result<_, String>` con mensajes en español, como `pty_*`.

- `browser_spawn { url: String, bounds: Bounds, scale: f64, on_event: Channel<BrowserEvent> } -> Result<BrowserBoot, String>`
  — spawnea el host con el motor efectivo. Solo puede haber uno (`id`
  implícito `"browser"`); si ya existe, lo mata primero. `BrowserBoot =
  { cef: String, chromium: String, apiVersion: u32, source: "bundled"|"installed", noSandbox: bool }`.
  Si no hay helper setuid ni user namespaces, el primer spawn ya lleva
  `--idq-no-sandbox` y `noSandbox` sale `true`. Si aun así el host muere
  antes de `ready` con exit `15`, abort `1` o `initialize` `11`, reintenta
  una vez con `--idq-no-sandbox` y no reenvía el `fatal` del primer
  intento. El embed es X11: en una sesión Wayland el ADE fija
  `GDK_BACKEND=x11` si hay `DISPLAY` (XWayland). Sin `DISPLAY`, no lo pisa
  y `browser_spawn` falla con un error claro.
- `browser_command { cmd: BrowserCommand }` — `BrowserCommand` es el enum de
  4.4 serializado con `#[serde(tag = "cmd", rename_all = "snake_case")]`.
- `browser_set_bounds { x, y, w, h, scale }` — recibe CSS px y scale; el ADE
  multiplica y redondea antes de mandar `set_bounds`.
- `browser_set_visible { visible: bool }` → oculta/muestra el hueco GDK y manda
  `show`/`hide`.
- `browser_focus_app` — devuelve el foco X11 al toplevel de idioteque
  (`XSetInputFocus`) y, en el mismo comando, manda `unfocus` al host. Mientras
  la ventana de CEF tiene el foco, el webview no recibe teclas; el frontend lo
  llama una vez por `focusin` de la barra Svelte (si `focusOwner` no es ya
  `app`) y tras un `shortcut` del host (Ctrl+L).
- `browser_kill` — `close`, espera 2 s, `SIGKILL` si sigue.
- `cef_runtime_info -> CefRuntimeInfo`:
  `{ current: SlotInfo, base: SlotInfo, candidate: SlotInfo | null, denylist: DenyEntry[], lastCheckAt: string | null, pendingPromotion: {...} | null, hostApiVersion: 15200, platform: "linux64", hostAlive: bool }`
  con `SlotInfo = { cefVersion, chromiumVersion, source, path, verified }`.
- `cef_check_updates -> Result<(), String>` — lanza un ciclo del updater en
  segundo plano (si no hay otro corriendo).

`BrowserEvent` (Channel) es el enum de 4.3 con `#[serde(tag = "event", rename_all = "kebab-case")]`
más `{"event":"exit","code":n}` cuando el proceso termina.

`Bounds = { x: f64, y: f64, w: f64, h: f64 }` en CSS px.

Hueco GDK: `browser_spawn` crea bajo el toplevel un `GdkWindow` hijo nativo
(GDK pinta el toplevel con `IncludeInferiors`, así que una ventana X ajena
colgada directamente quedaría tapada; un hijo que GDK conoce se descuenta de su
región de recorte). Su XID es el `--idq-parent` del host. El ADE lo mueve con
coordenadas lógicas en `browser_set_bounds`, lo oculta/muestra en
`browser_set_visible` y lo destruye en `browser_kill`.

## 6. Evento global `cef-update`

`app.emit("cef-update", payload)`:

```json
{ "kind": "updated", "chromium": "153.0.8000.10", "cef": "153.0.1+…" }
```

```json
{
  "kind": "incompatible",
  "candidateChromium": "153.0.8000.10",
  "candidateCef": "153.0.1+…",
  "currentChromium": "152.0.7977.83",
  "currentCef": "152.0.6+…",
  "reason": "health-exit-10"
}
```

Se emite `updated` justo después de promover y `incompatible` justo después de
denylistar. No se emite nada en ningún otro caso.

## 7. Textos de los avisos

Éxito (toast, esquina inferior derecha, 8 s):

```
Se ha actualizado a Chromium XXX
```

Fallo (aviso pegajoso, botón cerrar, enlace "Abrir issue"):

```
Se intentó actualizar a Chromium XXX, no es compatible con esta versión de idioteque.

Chromium continuará en YYY. Puedes abrir un issue reportando:
```

seguido de un bloque con `Candidato: XXX` y `Actual: YYY`. `XXX` y `YYY` son
versiones de Chromium (`candidateChromium`, `currentChromium`). Nunca dice
"no compiló".

Enlace del issue: `https://github.com/don-linux/idioteque/issues/new` con
`title=CEF XXX no compatible con idioteque <version>` y `body` con candidate
CEF/Chromium, current CEF/Chromium, `hostApiVersion`, `reason`, plataforma.

Mientras la superficie `browser` está visible, la ventana X11 de CEF tapa los
toasts: los avisos `cef-update` se encolan y se muestran al salir del navegador.

## 8. Updater

- Disparadores: 30 s después de arrancar la app, después cada 24 h
  (`IDIOTEQUE_CEF_CHECK_INTERVAL_SECS` lo cambia), y `cef_check_updates`.
- Índice: `IDIOTEQUE_CEF_INDEX_URL` o `https://cef-builds.spotifycdn.com/index.json`,
  con `Accept-Encoding: gzip` e `If-None-Match` (ETag en `state.json`).
- Selección: plataforma actual → `channel == "stable"` → archivo
  `type == "minimal"` → versión > motor efectivo → no denylistada → la mayor.
- Espacio: antes de bajar exige 2 GB libres en `~/.idioteque/cef`.
- Overrides de desarrollo: `IDIOTEQUE_CEF_STARTUP_DELAY_SECS`,
  `IDIOTEQUE_CEF_SKIP_STRIP=1` (para slots sintéticos en pruebas).
- Descarga a `candidate/download.tar.bz2` verificando `size` y `sha1` del
  índice en streaming. Extracción de `Release/*`, `Resources/*`,
  `include/cef_api_versions.h`, `include/cef_version.h`, `LICENSE.txt` al
  layout plano; strip de `libcef.so`; `manifest.json` con `source:
  "downloaded"`; se borra el tarball.
- Pre-check: `apiVersionMin` del candidate ≤ `hostApiVersion` (15200). Si no →
  denylist con `api-version-min-above-host` + evento `incompatible`, sin
  arrancar nada.
- Health check (4.5). Falla → denylist + `incompatible`; `candidate/` se borra.
- Pasa → si no hay `cef-host` vivo, promoción atómica; si lo hay,
  `manifest.verified = true` + `state.pendingPromotion`, y se promueve al
  cerrar el navegador o al siguiente arranque (antes de spawnear).
- Promoción: `rename(current, current.old)` (si existe) → `rename(candidate,
  current)` → borrar `current.old` → evento `updated`.
- Recuperación al arrancar: `current` ausente y `current.old` presente →
  `rename(current.old, current)`; `candidate` sin `verified` → borrar;
  `candidate` con `verified` → promover; `health-cache-*` → borrar.

## 9. Frontend

- `WorkspaceSurface = "editor" | "terminals" | "browser"` en
  `src/lib/workspace-surface.svelte.ts`. `terminal.surface` delega ahí.
- Atajos: `Ctrl+B` navegador (toggle; en la terminal `Ctrl+Shift+B`), `Ctrl+T`
  árbol (en la terminal `Ctrl+Shift+T`), `Ctrl+G` grafo, `Ctrl+J`/`Ctrl+Alt+J`/
  `Ctrl+Shift+J` terminal como hoy. Al salir del navegador se vuelve a la
  superficie anterior.
- Vista: franja superior Svelte (`BrowserToolbar`: atrás, adelante,
  recargar/parar, URL, DevTools) y un `div.host` que ocupa el resto. El rect
  del `div.host` (CSS px) × `devicePixelRatio` son los bounds del host.
- Footer: acción `browser` (icono `Globe`) después de `terminal`.
- Ocultar el navegador no mata el proceso; volver a Inicio sí.
- Modales del ADE (`unsavedExit`, `FolderVisibilityModal`) ocultan la ventana
  CEF mientras están abiertos.
