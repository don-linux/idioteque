# Feature: Superficies editor/terminales y grid tipo tmux

**Fecha:** 22/08/2026

## Descripción de la feature

El IDE tenía una sola PTY y un peek (`Ctrl+J` / `Ctrl+Alt+J`). Faltaba
una vista a pantalla completa para varias terminales, sin matar los
procesos al volver al markdown.

Ahora hay dos superficies en la misma ruta `/workspace`. El peek del
editor no cambia. `Ctrl+Shift+J` entra a un canvas de terminales con
un grid que se reequilibra al añadir o quitar panes.

## Implementado exitosamente

**1. Varias sesiones de PTY**

El backend ya guardaba las PTY por id. Se añade `pty_kill_all`, que
solo mata las sesiones `workspace` / `workspace-*` y deja vivo el
preview de ajustes.

El frontend deja de ser un singleton. Cada pane tiene id
`workspace-N`, writer propio y resize propio.

**2. Superficie `terminals`**

`Ctrl+Shift+J` cambia entre editor y terminales. El árbol y el
markdown se parkean; no se desmontan. El footer se queda.

Si hay drafts, un modal pide guardar y continuar. `saveAll` escribe
todos. Si el guardado falla, no se cambia de superficie. Volver al
editor no pide nada y cierra el peek.

En esta superficie `Ctrl+J` no mueve el dock.

**3. Grid tipo tmux**

El layout es el tiled de tmux, con el eje largo primero (Hyprland
smart split). En un canvas ancho: 1, 2 lado a lado, 2+1, 2×2, 3+2,
3×2. La última fila incompleta se estira.

Tope de 6. El `SquarePlus` del footer añade un pane. La cruz del pane
lo cierra. Cerrar el último en terminales deja el canvas vacío.

En el peek del editor solo se ve la sesión enfocada.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Sidecar / binario de tmux
- Arrastre entre panes
- Pestañas de terminal además del grid
