# Feature: Terminal integrada y live docs

**Fecha:** 19/08/2026

## Descripción de la feature

El editor ya dejaba leer y escribir markdown. Faltaba poder correr un agente
TUI al lado de esa documentación, y que el árbol y el archivo abierto se
enteraran cuando alguien (el agente, la terminal o la propia app) tocaba el
disco.

Ahora hay una terminal real (PTY + xterm) acoplable abajo o a la derecha, se
pueden borrar `.md` desde el árbol, y el workspace se refresca solo.

## Implementado exitosamente

**1. Terminal integrada**

`Ctrl+J` abre o cierra el panel abajo, como el de Visual Studio Code. El
sidebar se queda a altura completa.

`Ctrl+Alt+J` hace lo mismo a la derecha, como la ventana de agents de Cursor.
Si el panel ya está en el otro lado, el atajo lo mueve; no mata el proceso.

Esconder la terminal no corta el shell. Cambiar de carpeta o volver a Inicio
sí lo mata.

Hay un handle para cambiar el alto o el ancho. Esos tamaños duran la sesión,
no se guardan.

El backend abre un PTY con `portable-pty`, lanza `$SHELL` (si no hay, `/bin/sh`)
en la carpeta del workspace, y pone `TERM=xterm-256color`. El frontend es
xterm.js. Sirve para TUI: resize, colores, tty de verdad.

**2. Live docs desde el disco**

Al abrir una carpeta, Rust vigila el root. Ignora el ruido (`.git`,
`node_modules`, `target`, `dist`, `.svelte-kit`) y los temporales del
autoguardado (`*.idioteque.tmp`).

Si aparece, cambia o desaparece un markdown, el árbol se actualiza. Si el
archivo abierto cambió y no hay ediciones sin guardar, el editor lo recarga
sin mandar el cursor al tope. Si el archivo ya no está, se cierra el editor.

Si hay cambios sin guardar, no se pisan.

**3. Borrar y editar desde Idioteque**

Editar sigue siendo el editor con autoguardado.

Borrar es nuevo: cada `.md` del árbol tiene una cruz (visible al pasar el
mouse o si está seleccionado) y la tecla Delete cuando el archivo tiene
foco. Pide confirmación. Solo borra markdown dentro de la carpeta abierta,
con el mismo candado de rutas que la lectura y la escritura. No borra
carpetas.

Después de borrar, el árbol se refresca al momento. Si era el archivo
abierto, el editor se cierra.

El watcher cubre lo que pase fuera: un agente, un `rm` en la terminal, otro
editor.

**4. Pruebas**

Hay pruebas de borrar (existe → se va, no se puede salir del root, no se
puede borrar un `.txt`) y del filtro del watcher (tmp, `node_modules`,
`.git`).

El typecheck del frontend pasó. Las pruebas de Rust pasaron.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Pestañas de terminal
- Crear o renombrar archivos
- Borrar carpetas
- Guardar el tamaño o el lado del panel
- Vista previa del markdown
- Sandbox del shell
