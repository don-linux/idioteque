# Svelte MCP / autofixer (pases locales)

Cloud Agent **no** corre el MCP de Svelte. `svelte-autofixer`, `list-sections` y `get-documentation` fallan o piden auth (`needsAuth`; el login hace timeout). El frontend en Cloud se cierra con `bun run check` y tests. Este archivo guarda los **prompts** para pasar esas herramientas en una máquina local donde el MCP responde.

El plan CEF Wayland (destripar el CEF que ya está en `main`) **no** usa este MCP como gate. A4 en Cloud = `bun run check`. El pase 2 de este archivo se corre **después**, en local. A4b de ese plan actualiza el inventario del pase 2; no borra el pase 1.

Usa **solo** el modelo `cursor-grok-4.6-xhigh-fast`. Prohibido `inherit` u otro slug. El subagente `svelte-file-editor` y cualquier resume llevan `model: "cursor-grok-4.6-xhigh-fast"`.

Hay dos pases. Copia **uno**. No mezcles inventarios.

---

## 1. Tras el recorte Linux x86_64

Copia desde «Objetivo» hasta el final de esta sección, en un plan de Cursor local.

### Objetivo

Pasar el autofixer de Svelte 5 sobre los `.svelte` que el recorte Linux x86_64 tocó o que aún mencionen otro OS, hasta **cero issues**. No es un rediseño: no toques paletas, IDs de tema ni HEX.

### Herramientas (obligatorio, en este orden)

1. MCP Svelte `list-sections`.
2. `get-documentation` de las secciones que apliquen (Svelte 5 runes, `$props`, components, styling).
3. `svelte-autofixer` sobre cada archivo de la lista, en bucle hasta que no devuelva issues.
4. Skill `svelte-code-writer` / subagente `svelte-file-editor` (mismo modelo).

Si el autofixer pide un cambio que no es Linux64 (por ejemplo volver a partir paths por `\`), no lo apliques; anótalo y sigue.

### Archivos de este recorte

- `src/lib/components/RecentGrid.svelte` — `splitPath` solo con `/`. Un nombre con `\` es el basename, no un padre.
- `src/routes/+layout.svelte` — stack Inter / JetBrains Mono / `system-ui` / `ui-monospace`. Sin `-apple-system` ni `SF Mono`. Deja `-webkit-font-smoothing`.

### Grep local (añade lo que salga)

Cualquier `.svelte` con `\\` como separador, `-apple-system`, `SF Mono`, `Cmd`, `Windows`, `macOS`.

No toques `-webkit-user-select` ni `-webkit-font-smoothing` en `FileTreeRow.svelte`, `FileTreePanel.svelte`, `MarkdownEditor.svelte`.

### Comprobar

- Runes Svelte 5 (`$props`, `$state`) siguen válidos tras el recorte de paths.
- `bun run check` y `bun run test` verdes.
- No reintroducir créditos a productos de otro OS.

### Fuera

- No desacoplar CEF.
- No renombrar `campbell` / `one-half-dark`.
- No editar lockfiles.

---

## 2. Tras la implementación CEF (ventana Alloy)

Copia desde «CONTEXTO» hasta el final de esta sección. Única instrucción de ese plan. No implementes CEF. No toques Rust ni `cef-host`. No abras el backup de la toolbar como código vivo.

Cuando el destripado CEF de `main` (ventana Alloy propia, editor al lado) ya está en el working tree local. Antes no hay archivos de esa entrega que pasar. No hay otra rama de la que copiar.

### CONTEXTO

Repo `don-linux/idioteque`. Svelte 5 + SvelteKit. El navegador es un proceso Alloy en otra ventana. En el IDE no hay superficie interna de página ni barra de URL.

Cloud Agent reescribió en `main` los `.svelte` / `.svelte.ts` del navegador **sin** MCP Svelte. Hay que pasarles ahora las herramientas que en Cloud no corren.

### OBJETIVO

Revisar y, si hace falta, corregir **solo** los módulos Svelte de esa entrega hasta que `svelte-autofixer` quede limpio y `bun run check` pase. El comportamiento de producto no cambia: `Ctrl+B` abre, muestra u oculta la ventana CEF; no hay toolbar montada; no hay `div.host`.

### HERRAMIENTAS (obligatorias en local)

Usar **todas**. Si MCP no autentica en esta sesión, parar y pedirlo; no fingir el pase. Mismo modelo: `cursor-grok-4.6-xhigh-fast`.

1. Subagente `svelte-file-editor` para cada `.svelte` o `.svelte.ts` (o un grupo chico que no se pise).
2. Skill `svelte-code-writer`.
3. Skill `svelte-core-bestpractices`.
4. MCP Svelte, en este orden, **por archivo**:
   - `list-sections`
   - `get-documentation` de las secciones que apliquen (runes, props, events, each, attachments, etc.)
   - `svelte-autofixer` sobre el código
   - aplicar el arreglo
   - `svelte-autofixer` otra vez
   - repetir hasta que no devuelva issues ni sugerencias
5. Equivalente CLI si el MCP del IDE no está, pero el servidor sí:

```bash
npx @sveltejs/mcp list-sections
npx @sveltejs/mcp get-documentation "$state,$derived,$effect,$props"
npx @sveltejs/mcp svelte-autofixer ./ruta/al/archivo.svelte --svelte-version 5
```

Con runes en un string de shell, escapar `$` como `\$`.

No llamar `playground-link` salvo que lo pida el humano.

### INVENTARIO

Completar con `git diff origin/main -- '*.svelte' '*.svelte.ts'` (o el base de la PR). Pasar **todos** los tocados o nuevos. Guía de nombres (ajustar a los paths reales):

- `src/lib/browser.svelte.ts`
- `src/lib/browser-shortcuts.ts` (solo si es `.svelte.ts`; si es `.ts` plano, no autofixer)
- `src/lib/browser-errors.ts` (igual)
- componentes nuevos de estado/error del navegador en `src/lib/components/`
- `src/routes/workspace/+layout.svelte` y `+page.svelte` si A4 los tocó
- `src/lib/components/FooterActions.svelte` si el globo o el toggle viven ahí
- cualquier otro `.svelte` / `.svelte.ts` de la PR del navegador

**Fuera:**

- `docs/cef/backup-toolbar/` (archivo muerto; no es runtime)
- `BrowserToolbar.svelte` / `BrowserView.svelte` si solo están en el backup
- Rust, `cef-host`, tests que no sean Svelte, docs de CEF salvo que un autofix obligue un import
- El pase 1 (recorte Linux): no lo mezcles aquí

### QUÉ NO HACER

- No montar la toolbar. No importar el backup.
- No crear superficie interna `browser` ni `div.host`.
- No cambiar atajos, IPC, ni copy de producto.
- No reintroducir la lista prohibida del plan CEF (el mismo barrido que el grupo C). No documentar backends de display ajenos a Wayland ni protocolos de foco compartido.
- No “mejorar” UI de paso.
- No tocar paletas, IDs de tema ni HEX.

### GATES

1. `svelte-autofixer` limpio en cada archivo del inventario.
2. `bun run check`.
3. Tests frontend de esos archivos, si existen, verdes.

`/loop` por archivo: autofixer → cambio → autofixer, hasta limpio. Después `bun run check` en el lote.

### HECHO

Una lista de archivos pasados, issues que encontró el autofixer y qué se cambió. Si un archivo ya estaba limpio, decirlo. Commit aparte, solo frontend Svelte.
