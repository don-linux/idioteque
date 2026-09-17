# Runtime CEF: base, slots, updater

Cómo vive Chromium dentro de idioteque y qué hacer cuando cambia. El detalle
de nombres, rutas y mensajes está en [`cef/CONTRACT.md`](cef/CONTRACT.md).

## Qué es cada cosa

- **idioteque** es Tauri 2 + wry. No enlaza CEF. Solo lanza y habla con
  `cef-host`.
- **`cef-host`** es un binario aparte (`src-tauri/cef-host`) compilado contra
  el crate `cef` 152.3.0 (API de CEF `15200`). Carga la `libcef` del slot que
  se le indique y se dibuja como ventana hija X11 dentro de la ventana de
  idioteque. El mismo binario hace el health check.
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

`bun run cef:prepare` (lo llaman `beforeDevCommand` y `beforeBuildCommand`):

1. `cargo build -p cef-host --release`. El `build.rs` de `cef-dll-sys`
   descarga la dist pinneada a `src-tauri/.cef-sdk/` (una vez) y compila
   `libcef_dll_wrapper` (cmake + ninja).
2. Copia el binario a `src-tauri/binaries/cef-host-<triple>`
   (`bundle.externalBin`).
3. Copia el runtime desde la SDK a `src-tauri/cef-base/`, verifica el sha1 del
   tarball contra `src-tauri/cef/base.json`, stripea `libcef.so` y escribe el
   `manifest.json` (`bundle.resources` → `cef/base/`).

Nada de esto se commitea. El usuario final no descarga nada: el instalador ya
trae el base.

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
