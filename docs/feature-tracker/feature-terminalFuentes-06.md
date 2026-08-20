# Feature: Fuentes y tamaño de la terminal

**Fecha:** 20/08/2026

## Descripción de la feature

La página de Configuración estaba vacía. La terminal usaba siempre JetBrains
Mono a 13px, así que una Nerd Font instalada (zsh, iconos, powerline) no se
veía bien y al reiniciar había que recordar el tamaño a mano.

Ahora Configuración tiene una sección Terminal. Puedes elegir una fuente del
sistema escribiendo para filtrar la lista, cambiar el tamaño, y ambos se
quedan guardados.

## Implementado exitosamente

**1. Lista de fuentes del sistema**

Rust lee las fuentes instaladas (también las de la carpeta de usuario, donde
suelen estar las Nerd Font). La lista se ordena: primero las mono, después el
resto. Puedes buscar escribiendo en la misma caja.

Si no eliges nada, se usa la fuente de siempre.

**2. Tamaño fijo**

El tamaño va de 10 a 24 píxeles. Hay botones +/− y un número. Se guarda al
cambiarlo.

**3. Se recuerda al volver**

Fuente y tamaño viven en `~/.idioteque/config.json`, junto al historial de
carpetas. Al abrir de nuevo la app, la terminal ya sale como la dejaste.

Hay una vista previa en Configuración (`❯ git status`) para ver si los
iconos de la Nerd Font se pintan.

**4. Acceso desde el workspace**

El engranaje de Configuración también está en la carpeta abierta, para no
tener que volver a Inicio (eso apaga la terminal).

**5. Pruebas**

Hay pruebas de que el historial no se borra al guardar la fuente, de que el
tamaño se recorta si se pasa de rango, y del filtro de la lista.

El typecheck del frontend pasó. Las pruebas de Rust pasaron.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Fuente del editor o de la interfaz
- Elegir el shell (sigue usando `$SHELL`)
- Guardar el tamaño o el lado del panel
- Pestañas de terminal
- Cambiar el tema de colores
