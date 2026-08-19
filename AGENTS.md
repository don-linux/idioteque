# AGENTS.md

## Cursor Cloud specific instructions

`idioteque` is a desktop **markdown editor** built with **Tauri v2** (Rust backend) + **SvelteKit/Svelte 5** frontend (Vite), using **CodeMirror** for editing. The Rust backend (`src-tauri/`) exposes filesystem commands (`list_context_tree`, `read_markdown`, `write_markdown`) and app config (recent folders, stored at `~/.idioteque/config.json`). The frontend calls these via Tauri `invoke` and the native folder-picker dialog.

Standard commands live in `package.json` scripts (`dev`, `build`, `check`, `tauri`) and `src-tauri/Cargo.toml`; use those rather than duplicating them here.

Services / how to run, test, lint, build:

- Frontend dev server: `bun run dev` (Vite on port **1420**, `strictPort: true`). Serving the page in a plain browser at `localhost:1420` will render the UI but `invoke`/dialog calls fail — they only work inside the Tauri runtime.
- Full desktop app: `bun run tauri dev`. This runs `beforeDevCommand` (`bun run dev`) itself, so do **not** run a standalone `bun run dev` on port 1420 at the same time (the strict port would clash).
- Type/lint check: `bun run check` (`svelte-check`).
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml`.

Non-obvious caveats:

- **Package manager is `bun`** (see `bun.lock`), not npm/pnpm. It is installed at `~/.bun/bin` and added to `PATH` via `~/.bashrc`. Non-login shells may not have it on `PATH`; use `~/.bun/bin/bun` if `bun` is not found.
- **Rust must be a modern stable toolchain (≥ 1.85).** A transitive dependency (`dlopen2`) requires `edition2024`. The base image pinned the rustup default to `1.83.0`, which fails to build; the default has been switched to `stable` (currently 1.97.x). If a fresh environment ever reverts to 1.83, run `rustup default stable`.
- **Running the GUI needs a display.** Use `DISPLAY=:1` (the computer-use Desktop). Rendering falls back to software; `libEGL warning: DRI3 ...` messages are harmless. `WEBKIT_DISABLE_COMPOSITING_MODE=1` can help avoid GPU-compositing issues.
- The first `cargo`/`tauri dev` build compiles the whole Tauri/wry/webkit dependency tree (~1 min) and is cached afterward in `src-tauri/target`.
