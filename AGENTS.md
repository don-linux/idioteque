# AGENTS.md

## Cursor Cloud specific instructions

`idioteque` is a desktop **markdown editor** built with **Tauri v2** (Rust backend) + **SvelteKit/Svelte 5** frontend (Vite), using **CodeMirror** for editing. The Rust backend (`src-tauri/`) exposes filesystem commands (`list_context_tree`, `read_markdown`, `write_markdown`) and app config (recent folders, stored at `~/.idioteque/config.json`). The frontend calls these via Tauri `invoke` and the native folder-picker dialog.

Standard commands live in `package.json` scripts (`dev`, `build`, `check`, `tauri`) and `src-tauri/Cargo.toml`; use those rather than duplicating them here.

Services / how to run, test, lint, build:

- Frontend dev server: `bun run dev` (Vite on port **1420**, `strictPort: true`). Serving the page in a plain browser at `localhost:1420` will render the UI but `invoke`/dialog calls fail — they only work inside the Tauri runtime.
- Full desktop app: `bun run tauri dev`. The `tauri` script is the wrapper `scripts/tauri.ts`: it runs `bun run cef:prepare` first (seconds when cached, minutes on a cold cache) and only then launches the Tauri CLI, whose `beforeDevCommand` is just `bun run dev`. Do **not** run a standalone `bun run dev` on port 1420 at the same time (the strict port would clash). The CLI gives up if the dev server is not up within 180 s, which is why `cef:prepare` must not live inside `beforeDevCommand`.
- Release bundles: `bun run tauri build` produces deb + rpm (native Tauri targets, CEF included via `resources`/`externalBin`) and then the AppImage through the wrapper: `tauri bundle --bundles appimage` without CEF, injection of `src-tauri/cef-base/` and `cef-host` into `idioteque.AppDir`, repack with `linuxdeploy-plugin-appimage` (cached in `~/.cache/tauri/`). linuxdeploy must never see CEF (`libcef.so => not found` for `cef-host`, and `patchelf` would change the sizes the base manifest checks). The rpm is deliberately uncompressed (`rpm-rs` gzip takes tens of minutes on 350 MB). The whole build phase runs with `TMPDIR=src-tauri/target/tmp` unless you already set one: `tauri-bundler` stages the deb/rpm in `tempdir()` and the AppImage plugin extracts into `$TMPDIR`, and on systemd ≥ 258 `/tmp` is a tmpfs with a per-user quota (`Disk quota exceeded (os error 122)` on a 350 MB bundle). Details in `docs/CEF-RUNTIME.md`.
- Type/lint check: `bun run check` (`svelte-check`).
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml`.

Non-obvious caveats:

- **Package manager is `bun`** (see `bun.lock`), not npm/pnpm. It is installed at `~/.bun/bin` and added to `PATH` via `~/.bashrc`. Non-login shells may not have it on `PATH`; use `~/.bun/bin/bun` if `bun` is not found.
- **Rust must be a modern stable toolchain (≥ 1.85).** A transitive dependency (`dlopen2`) requires `edition2024`. The base image pinned the rustup default to `1.83.0`, which fails to build; the default has been switched to `stable` (currently 1.97.x). If a fresh environment ever reverts to 1.83, run `rustup default stable`.
- **Running the GUI needs a display.** Use `DISPLAY=:1` (the computer-use Desktop). Rendering falls back to software; `libEGL warning: DRI3 ...` messages are harmless. `WEBKIT_DISABLE_COMPOSITING_MODE=1` can help avoid GPU-compositing issues.
- The first `cargo`/`tauri dev` build compiles the whole Tauri/wry/webkit dependency tree (~1 min) and is cached afterward in `src-tauri/target`.
- `ninja-build` and `cmake` are required for `cef-host`; first build downloads ~320 MB to `src-tauri/.cef-sdk` and compiles `libcef_dll_wrapper` (~2–5 min); `bun run cef:prepare` produces `src-tauri/binaries/` and `src-tauri/cef-base/` (both gitignored; `src-tauri/build.rs` fails a release build if they are missing); the deb/rpm/AppImage grow by ~350 MB; deb/rpm declare Chromium's runtime libs (`bundle.linux.deb.depends` / `rpm.depends`), the AppImage only assumes NSS (`libnss3`) from the host; for running the browser in this VM (Xvnc without DRI3) use `IDIOTEQUE_CEF_NO_SANDBOX=1 IDIOTEQUE_CEF_SOFTWARE_GL=1` (software ANGLE/SwiftShader; `IDIOTEQUE_CEF_ARGS` passes extra Chromium switches); the VM's `/dev/shm` is 64 MiB, so `cef-host` picks `--disable-dev-shm-usage` on its own after probing (`cef-host: shm TempDir(..)` in `~/.idioteque/cef/logs/cef-host.log`); `LD_LIBRARY_PATH` is not needed for the app (the app sets it for the host).
