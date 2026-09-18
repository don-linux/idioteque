# idioteque

Editor markdown de escritorio para **Linux x86_64**. Tauri v2 (Rust) +
SvelteKit / Svelte 5 (Vite) + CodeMirror.

No hay builds para Windows, macOS ni Linux ARM.

## Desarrollo

Ver `AGENTS.md`. Comandos habituales:

- `bun run dev` — frontend en el puerto 1420
- `bun run tauri dev` — app de escritorio (incluye el frontend)
- `bun run check` — svelte-check
- `bun run test` — Vitest
- `cargo test --manifest-path src-tauri/Cargo.toml` — tests Rust
