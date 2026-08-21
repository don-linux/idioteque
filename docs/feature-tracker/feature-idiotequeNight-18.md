# Feature: Idioteque Night

**Fecha:** 21/08/2026

## Descripción de la feature

El tema de interfaz que se llamaba Tokyo-dark / tokyo-dark ahora se
llama Idioteque Night (id `idioteque-night`). Es una paleta original
inspirada en Tokyo Night, no un port oficial.

Los nombres que se ven en la app ya no llevan guiones: Idioteque Dark,
Idioteque Night e Idioteque Light (antes Idioteque-dark, Tokyo-dark e
Idioteque-light).

No hay alias ni fallbacks. El id viejo `tokyo-dark` desapareció. Si
aparece, se trata como desconocido y cae al default que ya existía:
`idioteque-dark`.

## Implementado exitosamente

**1. Nuevo nombre**

Tokyo-dark pasó a Idioteque Night. El id guardado es
`idioteque-night`. Los colores siguen siendo los de esa paleta
propia.

**2. Labels limpios**

En el selector se lee Idioteque Dark, Idioteque Night e Idioteque
Light. Sin guiones en lo que ve la persona.

**3. Sin alias del id viejo**

Si alguien todavía tiene `tokyo-dark` en la config, no se traduce. Se
trata como desconocido y vuelve Idioteque Dark.

**4. Catálogo al día**

Se actualizó el listado de temas en el frontend y en Rust. DESIGN.md
usa los nombres limpios.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Temas de la terminal
- Reescribir los feature-tracker 15 y 16 (son bitácora histórica)
- Cambiar los ids kebab-case del resto de temas al guardarlos
