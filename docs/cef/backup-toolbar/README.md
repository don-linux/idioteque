# Backup: barra Svelte del IDE

Copia de la barra de navegación que vivía en el frontend de idioteque
(atrás, adelante, recargar o detener, campo de URL, DevTools, cerrar).
Era chrome del IDE, no del host Chromium.

El runtime no monta estos archivos. El siguiente plan rehará la barra
en HTML dentro de `cef-host`.

## Contenido

- `BrowserToolbar.svelte` / `BrowserToolbar.test.ts` — componente y tests.
- `browser-url.ts` / `browser-url.test.ts` — normalización y texto compacto de URL.
- `browser-shortcuts-url.ts` — atajo Ctrl+L que enfocaba el campo.
- `chrome-focus.ts` — helpers de foco y teclado de esa barra.
