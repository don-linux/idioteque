# Feature: Paletas oficiales del editor

**Fecha:** 21/08/2026

## Descripción de la feature

En Configuración → Temas ahora se pueden elegir paletas oficiales de
editores, además de Idioteque-dark, Tokyo-dark e Idioteque-light.

Los HEX salen de las fuentes publicadas. No se inventaron colores. El
tema de la terminal no cambia.

Platzi es el Green Mode de la extensión de VS Code
(`codevars.platzi-theme-for-vs-code`, repo `platzi/platzi-theme`).

## Implementado exitosamente

**1. Catálogo oficial**

Se importaron: Platzi, Tokyo Night, Catppuccin Mocha, Nord, Gruvbox
Dark, Everforest Dark, One Dark y Solarized Dark. Cada uno trae chrome
y syntax con los HEX de su fuente.

**2. Platzi**

Fondo `#03091E`, acento `#adeb42`, keywords `#C792EA`. El syntax es el
del JSON oficial; Classic no se suma porque solo cambia la status bar.

**3. El editor pinta más tokens**

CodeMirror usa también keyword, string, number, function, type,
variable, operator, tag e invalid. Siguen siendo variables CSS del
tema.

**4. Pruebas**

Hay pruebas de los HEX clave y de que Rust acepta `platzi` y
`tokyo-night`. El typecheck del frontend pasó.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Temas de terminal
- Importar un JSON o VSIX del usuario
- Platzi Classic como tema aparte
- Temas light de terceros
