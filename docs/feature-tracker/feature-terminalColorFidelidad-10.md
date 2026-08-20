# Feature: Fidelidad de color de la terminal

**Fecha:** 20/08/2026

## Descripción de la feature

La terminal integrada usa xterm.js, igual que Visual Studio Code, pero el
prompt (Powerlevel10k, segmentos azul y ámbar) se veía apagado. xterm solo
tenía cinco colores de chrome; los 16 ANSI caían en la paleta Tango
(`#3465a4`, `#c4a000`). El renderer era DOM, así que los chevrons Powerline
no usaban `customGlyphs`.

Ahora la terminal tiene la paleta ANSI completa al estilo Tokyo Night
(alineada con el acento y el peligro de la UI) y prueba WebGL, como VS Code.
Si WebGL no arranca o pierde el contexto, se queda en DOM. El color del
prompt mejora igual.

## Implementado exitosamente

**1. Paleta ANSI completa**

Chrome igual que el resto de la app: fondo `#14161a`, texto `#e4e6ea`,
cursor y selección con el acento `#7aa2f7`.

Los 16 ANSI son Tokyo Night. El azul del directorio es `#7aa2f7` y el
amarillo de git sucio es `#e0af68`, no Tango. `minimumContrastRatio` se
queda en 1 para no reescribir los colores.

**2. WebGL con fallback a DOM**

Después de `open()`, se carga `@xterm/addon-webgl`. Si el addon no puede
crearse, si `loadAddon` falla, o si se pierde el contexto, la terminal
sigue en DOM. `customGlyphs` pinta los triángulos Powerline cuando WebGL
sí está.

**3. Pruebas**

Hay tests de que el theme trae los 16 ANSI y no usa Tango, y del helper
del renderer (WebGL ok, create/load tira, context lost, dispose).

El typecheck del frontend pasó. Las pruebas de vitest pasaron.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Selector de tema en Configuración
- Ligatures, Unicode11, cambiar cursor o scrollback
- Filtros CSS de saturación sobre el canvas
- Tocar el PTY o `COLORTERM`
