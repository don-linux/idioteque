# Feature: Dock de la terminal y botón en el footer

**Fecha:** 19/08/2026

## Descripción de la feature

La terminal integrada ya existía, pero no se comportaba como se esperaba:
al abrir una carpeta el panel aparecía solo, y el pie de la app no tenía
una forma clara de mostrarlo u ocultarlo.

Esta entrega deja la terminal cerrada al entrar a una carpeta. Se abre
solo cuando la pides (atajo o icono). Puedes ponerla abajo o a la derecha.
Esconderla no corta lo que esté corriendo. El footer queda ordenado: el
nombre de la app a la izquierda y el botón de terminal a la derecha.

## Implementado exitosamente

**1. Al abrir una carpeta, la terminal empieza cerrada**

No se muestra sola. Solo aparece si pulsas `Ctrl+J`, `Ctrl+Alt+J` o el
icono del footer.

**2. Atajo abajo: `Ctrl+J`**

Si está cerrada, se abre abajo (bajo el editor). El sidebar sigue a
altura completa.

Si ya está abajo, se cierra.

Si está a la derecha, se mueve abajo.

**3. Atajo a la derecha: `Ctrl+Alt+J`**

Igual que el de abajo, pero el panel se acopla a la derecha, en una
columna como la de agents de Cursor.

Si está cerrada, se abre a la derecha. Si ya está a la derecha, se
cierra. Si está abajo, se mueve a la derecha.

**4. El icono del footer hace lo mismo**

Clic = `Ctrl+J` (abajo).

Alt + clic = `Ctrl+Alt+J` (derecha).

El botón solo se ve cuando hay una carpeta abierta. Queda marcado si el
panel está visible.

**5. Cerrar el panel no mata el proceso**

Al esconder la terminal, el proceso y lo que ya se escribió siguen
vivos. Un agente TUI no se corta solo por ocultar el panel.

**6. Cambiar de carpeta o volver a Inicio sí lo mata**

Ahí se cierra de verdad la sesión de la terminal.

**7. Una sola sesión**

No hay pestañas. Es una terminal, no varias.

**8. Footer**

A la izquierda, al inicio del pie, va la palabra “idioteque”.

A la derecha, al final, va el botón de terminal (icono de consola).

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Pestañas de terminal
- Guardar el tamaño o el lado del panel
- Terminal en la pantalla de inicio
