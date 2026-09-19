# CEF adversarial pack

Kickoffs cite this path. Tests first. Production code only if a new test fails.

## Políticas vigentes

1. Chromium en Linux usa memoria compartida en `/dev/shm` (fichero anónimo + unlink + mmap). No se copia `libcef` ahí. No se fuerza `--disable-dev-shm-usage` cuando el probe de 128 MiB cabe.
2. Si `/dev/shm` no sirve, el flag oficial manda la reserva a `$TMPDIR`. Si ese temp también falla (`EDQUOT`, systemd ≥ 258), `TMPDIR=<cache>/shm` queda en disco (`CacheDir`).
3. `chrome-sandbox` solo se exporta como helper si es setuid-root. Sin helper usable el primer visible ya puede ir con `--idq-no-sandbox`; el ADE reintenta 1 / 11 / 15.
4. Visible: Ozone `wayland`, Views Alloy, no `--use-native`. Health: Ozone `headless`, windowless, 30 s. `IDIOTEQUE_CEF_ARGS` no voltea esos modos ni mete `use-native` / `ozone-platform` / `ozone-platform-hint` (el hint manda Ozone a X11 si hay `DISPLAY`).
5. `bun run cef:prepare` produce `binaries/` y `cef-base/` antes de `tauri dev` / `tauri build`. La AppImage inyecta el base **después** de linuxdeploy: el plugin no ve `libcef` y no corre `patchelf` sobre esos ELF.
6. Geometría: `--idq-bounds` y `set_bounds` son DIP de Views. El scale no multiplica los bounds; solo viaja como `--idq-scale` si no es `1`. `CSS × scale` era el tamaño físico de X11.
7. DevTools es una ventana Views: `show_dev_tools` sin `WindowInfo` y `OnPopupBrowserViewCreated`. Un `WindowInfo`, aunque sea `default()`, o `use_default_window = 1`, piden el camino nativo.

## Linux surfaces

deb / rpm / AppImage. Ubuntu cloud es un lab, no “Linux”. No codificar AppArmor-only, apt-only, ni “64 MiB shm = always disable”.

## Docs

- Contract: [`CONTRACT.md`](CONTRACT.md)
- Runtime: [`../CEF-RUNTIME.md`](../CEF-RUNTIME.md)
- Cloud: [`../../AGENTS.md`](../../AGENTS.md)
- Chromium shm: [`platform_shared_memory_region_posix.cc`](https://chromium.googlesource.com/chromium/src/+/HEAD/base/memory/platform_shared_memory_region_posix.cc), [`GetShmemTempDir`](https://chromium.googlesource.com/chromium/src/+/HEAD/base/files/file_util.h) / [`kDisableDevShmUsage`](https://chromium.googlesource.com/chromium/src/+/HEAD/base/base_switches.h), [crbug/715363](https://crbug.com/715363)

## Won’t-test log

| Slice | Reason |
| ----- | ------ |
| shm (drop 128 MiB probe) | 64 MiB shm pasa `access()` y Chromium se queda sin reserva. |
| shm (drop `CacheDir`) | El flag oficial + `$TMPDIR` roto sigue sin reservar. |
| GNOME pager / píxeles de `about:blank` | Hace falta compositor. El pack fija identidad, Ozone/Views y `ready`. |
