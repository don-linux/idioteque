# Svelte MCP / autofixer (pase local)

Cloud Agent **no** corre el MCP de Svelte. `svelte-autofixer`, `list-sections` y `get-documentation` fallan o piden auth ahí. El frontend de la entrega CEF se cierra en Cloud con `bun run check` y tests. Este archivo es el **prompt** para el pase que sí usa esas herramientas, en una máquina local donde el MCP responde.

No es una guía de producto. No monta la toolbar. No crea superficie interna de página.

---

## Cuándo

Cuando la implementación CEF (ventana Alloy propia, editor al lado) ya está en tu working tree local. Antes de eso no hay archivos nuevos que pasar.

Modelo: el mismo que el resto de esa entrega (`cursor-grok-4.6-xhigh-fast`), o el que uses en Cursor local.

---

## Prompt para pegar en Cursor local

Copia desde «CONTEXTO» hasta el final. Única instrucción. No implementes CEF. No toques Rust ni `cef-host`. No abras el backup de la toolbar como código vivo.

### CONTEXTO

Repo `don-linux/idioteque`. Svelte 5 + SvelteKit. El navegador es un proceso Alloy en otra ventana. En el IDE no hay superficie interna de página ni barra de URL.

Cloud Agent escribió o portó `.svelte` / `.svelte.ts` **sin** MCP Svelte. Hay que pasarles ahora las herramientas que en Cloud no corren.

### OBJETIVO

Revisar y, si hace falta, corregir **solo** los módulos Svelte de esa entrega hasta que `svelte-autofixer` quede limpio y `bun run check` pase. El comportamiento de producto no cambia: `Ctrl+B` abre, muestra u oculta la ventana CEF; no hay toolbar montada; no hay `div.host`.

### HERRAMIENTAS (obligatorias en local)

Usar **todas**. Si MCP no autentica en esta sesión, parar y pedirlo; no fingir el pase.

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

### QUÉ NO HACER

- No montar la toolbar. No importar el backup.
- No crear superficie interna `browser` ni `div.host`.
- No cambiar atajos, IPC, ni copy de producto.
- No reintroducir la lista prohibida del plan CEF (el mismo barrido que el grupo C). No documentar backends de display ajenos a Wayland ni protocolos de foco compartido.
- No “mejorar” UI de paso.

### GATES

1. `svelte-autofixer` limpio en cada archivo del inventario.
2. `bun run check`.
3. Tests frontend de esos archivos, si existen, verdes.

`/loop` por archivo: autofixer → cambio → autofixer, hasta limpio. Después `bun run check` en el lote.

### HECHO

Una lista de archivos pasados, issues que encontró el autofixer y qué se cambió. Si un archivo ya estaba limpio, decirlo. Commit aparte, solo frontend Svelte.
