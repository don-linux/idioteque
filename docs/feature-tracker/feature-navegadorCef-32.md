# Feature: Navegador CEF

**Fecha:** 18/09/2026

## Descripción de la feature

idioteque abre Chromium de verdad en una ventana propia, al lado del
editor. No es el webview de la app. `Ctrl+B` (o `Ctrl+Shift+B` desde la
terminal, o el globo del footer) lanza, muestra u oculta esa ventana.
El workspace sigue siendo editor o terminales: no hay superficie
interna de página ni caja de URL en el IDE.

## Diseño actual

**1. Dos procesos, dos ventanas**

`cef-host` carga CEF (crate `cef` 152.3.0, API `15200`) y abre un
toplevel Alloy. Idioteque sigue en su ventana WebKitGTK. Con
`WAYLAND_DISPLAY` las dos son nativas. Sin compositor, el visible no
arranca (“sin compositor Wayland”) y el editor sigue.

**2. Ciclo de vida**

El primer `Ctrl+B` hace spawn (`about:blank`). El siguiente oculta
(unmap) sin matar la página. Otro la vuelve a mostrar. Inicio envía
`close` y mata el proceso.

**3. Atajos**

En el IDE: `Ctrl+B` / `Ctrl+Shift+B` cruzan el IPC como `shortcut`.
En la ventana de página: F5 / Ctrl+R / Ctrl+Shift+R, Alt+←/→, F12 /
Ctrl+Shift+I, Escape (stop). DevTools abre en ventana propia. Una
pestaña; los popups cargan ahí.

**4. Motor de fábrica y updater**

Cada release trae CEF 152.0.6 / Chromium 152.0.7977.83 en el
instalador. Encima hay un updater: índice oficial, sha1, strip de
`libcef.so`, health windowless 30 s, promoción atómica, denylist
firmada con `hostApiVersion`.

**5. Avisos**

Update: "Se ha actualizado a Chromium XXX". Incompatible: copy con
enlace a issue. No se diferirán por una superficie interna: no hay.

**6. Configuración → Navegador**

Chromium actual, base de fábrica, última comprobación, denylist y
“Buscar actualización”. Nada que guardar.

**7. Pipeline**

`bun run cef:prepare` compila el sidecar y prepara el base pinneado
en `src-tauri/cef/base.json`. Contrato: `docs/cef/CONTRACT.md`.
Runtime: `docs/CEF-RUNTIME.md`. Diseño: `docs/DESIGN.md` § Navegador.

## Fuera de alcance

- Pestañas múltiples
- DevTools acoplado dentro de la vista
- Canal beta del updater
- Guardar la URL entre sesiones
- Chrome Alloy / Views / caja de URL en el host (el backup Svelte
  está en `docs/cef/backup-toolbar/` para un plan posterior)
