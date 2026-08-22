# Feature: Preview con la shell real

**Fecha:** 21/08/2026

## Descripción de la feature

En Configuración → Terminal había una demo falsa: un prompt inventado
(`~/notas` y una flechita). No se veía el prompt de verdad ni la
config de la shell.

Ahora esa preview lanza la shell real de la persona (la de `$SHELL`,
zsh o bash) en una terminal de solo lectura. Se ve el prompt y la
config de verdad.

## Implementado exitosamente

**1. Preview con la shell real**

La vista previa de Configuración abre la misma shell que usa el
sistema. Sale el prompt de verdad, no uno inventado.

**2. Sesiones aparte**

La terminal del editor (Ctrl+J) no se toca ni se mata al abrir
ajustes. Cada una tiene su propia sesión.

**3. Fuente y tema en vivo**

Al cambiar fuente o tema en ajustes, la preview se actualiza sin
reiniciar la shell.

**4. Sin la app de escritorio**

Si se abre la página en el navegador (sin la app), se ve un mensaje
corto en vez del prompt inventado.

**5. Se cierra al salir**

Al salir de ajustes se cierra solo la preview. La terminal del editor
sigue igual.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Selector de zsh o bash (sigue usando `$SHELL`)
- Escribir en la preview (es de solo lectura)
