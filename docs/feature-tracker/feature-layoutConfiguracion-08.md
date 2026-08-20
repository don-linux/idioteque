# Feature: Layout de Configuración

**Fecha:** 20/08/2026

## Descripción de la feature

La ventana de Configuración era una sola columna: el título y, debajo, la
sección Terminal. El resto de la pantalla quedaba vacío. Si se sumaban más
opciones, todo se mezclaba en un listado largo.

Ahora es un layout clásico: menú lateral con secciones a la izquierda y el
contenido a la derecha. Al entrar no hay ninguna sección elegida. Terminal
sigue igual (fuente, tamaño y vista previa) y se abre sola en su propia
página.

## Implementado exitosamente

**1. Menú a la izquierda, contenido a la derecha**

Configuración ya no es una columna única. A la izquierda está la lista de
secciones. A la derecha, solo lo de la sección que elijas.

Si una sección crece, el scroll queda en ese panel. El menú no se mezcla
con el contenido.

**2. Al entrar no hay nada elegido**

El engrane de Home y el del footer siguen yendo a `/configuracion`. Ahí no
hay sección marcada. El panel derecho dice: “Elige una opción para empezar
a configurar”.

No se recuerda la última sección. Cada vez que entras, empiezas de nuevo
en esa pantalla.

**3. Terminal en su propia sección**

Clic en Terminal abre `/configuracion/terminal`. Ahí están la fuente, el
tamaño y la vista previa, igual que antes. Se guardan solos.

No se agregaron secciones nuevas. El cascarón queda listo para que las
siguientes no se mezclen en un solo listado.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Secciones nuevas (Editor, Apariencia, etc.)
- Recordar la última sección al volver
- Cambiar a dónde apuntan Home y el footer (siguen yendo a `/configuracion`)
