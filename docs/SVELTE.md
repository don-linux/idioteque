# Plan local: validar Svelte 5 con el MCP (autofixer)

Copia este prompt en un plan de Cursor **en tu máquina local**, donde el MCP de Svelte autentica. En cloud este MCP queda en `needsAuth` y el login hace timeout.

Usa **solo** el modelo `cursor-grok-4.6-xhigh-fast`. Prohibido `inherit` u otro slug. El subagente `svelte-file-editor` y cualquier resume llevan `model: "cursor-grok-4.6-xhigh-fast"`.

## Objetivo

Pasar el autofixer de Svelte 5 sobre los `.svelte` que el recorte Linux x86_64 tocó o que aún mencionen otro OS, hasta **cero issues**. No es un rediseño: no toques paletas, IDs de tema ni HEX.

## Herramientas (obligatorio, en este orden)

1. MCP Svelte `list-sections`.
2. `get-documentation` de las secciones que apliquen (Svelte 5 runes, `$props`, components, styling).
3. `svelte-autofixer` sobre cada archivo de la lista, en bucle hasta que no devuelva issues.
4. Skill `svelte-code-writer` / subagente `svelte-file-editor` (mismo modelo).

Si el autofixer pide un cambio que no es Linux64 (por ejemplo volver a partir paths por `\`), no lo apliques; anótalo y sigue.

## Archivos de este recorte

- `src/lib/components/RecentGrid.svelte` — `splitPath` solo con `/`. Un nombre con `\` es el basename, no un padre.
- `src/routes/+layout.svelte` — stack Inter / JetBrains Mono / `system-ui` / `ui-monospace`. Sin `-apple-system` ni `SF Mono`. Deja `-webkit-font-smoothing`.

## Grep local (añade lo que salga)

Cualquier `.svelte` con `\\` como separador, `-apple-system`, `SF Mono`, `Cmd`, `Windows`, `macOS`.

No toques `-webkit-user-select` ni `-webkit-font-smoothing` en `FileTreeRow.svelte`, `FileTreePanel.svelte`, `MarkdownEditor.svelte`.

## Comprobar

- Runes Svelte 5 (`$props`, `$state`) siguen válidos tras el recorte de paths.
- `bun run check` y `bun run test` verdes.
- No reintroducir créditos a productos de otro OS.

## Fuera

- No desacoplar CEF.
- No renombrar `campbell` / `one-half-dark`.
- No editar lockfiles.
