# Feature: Temas de color de la terminal

**Fecha:** 20/08/2026

## Descripción de la feature

En Configuración → Terminal ahora se puede elegir un tema de colores
para la terminal integrada (xterm.js). El default es Tokyo Night Night
original (el de Ghostty/WezTerm), no el híbrido que mezclaba Tokyo Night
con los colores de la app.

Hay un selector con temas ya hechos y oficiales. El tema se guarda con
el resto de la config (hay que pulsar Guardar o Ctrl+S) y vuelve a salir
al reabrir la app. Es un detalle estético.

## Implementado exitosamente

**1. Tokyo Night original como default**

El default es Tokyo Night Night original: fondo `#1a1b26` y texto
`#c0caf5`. El híbrido anterior no era el original; mezclaba Tokyo Night
con los colores de la app.

**2. Selector de temas oficiales**

En Terminal hay un selector con temas ya hechos: Tokyo Night, Dracula,
Nord, Gruvbox Dark, Catppuccin Mocha, One Half Dark, Solarized Dark y
Campbell.

**3. Vista previa**

En Configuración se ve la paleta ANSI y una terminal xterm de solo
lectura con un prompt idle. Al elegir un tema se recrea la instancia;
el tamaño de fuente se refleja en vivo. No acepta clics ni teclado.

**4. Se recuerda al volver**

El tema vive en `~/.idioteque/config.json`, con el resto de la config.
Hay que pulsar Guardar o Ctrl+S. Cambiar la fuente no borra el tema. Si
el id no se reconoce, vuelve a Tokyo Night.

**5. La terminal del IDE lo usa**

Al guardar, la terminal del IDE pinta con ese tema, también el marco y
el padding.

**6. Pruebas**

Hay pruebas del catálogo, de la persistencia y del prompt compacto de
la vista previa. El typecheck del frontend pasó.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Importar un JSON de Windows Terminal o un archivo de Ghostty
- Temas Storm/Moon u otras variantes
- Cambiar el tema de la interfaz de la app
- El híbrido viejo no se guarda como tema aparte
