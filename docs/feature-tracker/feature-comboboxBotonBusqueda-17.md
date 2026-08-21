# Feature: Combobox con botón y búsqueda interna

**Fecha:** 20/08/2026

## Descripción de la feature

Los selectores de fuente (terminal) y tema (interfaz) se podían escribir
en la caja cerrada. El cursor era de texto y hacía falta más de un clic
para abrir la lista.

Ahora el selector cerrado es un botón: un clic abre. La búsqueda vive
solo dentro del panel. Escribir filtra; el valor no cambia hasta elegir
una opción.

El tema de la terminal sigue siendo un dropdown nativo.

## Implementado exitosamente

**1. Botón cerrado**

Muestra la opción elegida y un chevron. El cursor es de clic. Un clic
abre o cierra.

**2. Búsqueda en el panel**

Al abrir, el foco va al campo “Filtrar…”. La lista se acorta al
escribir. Clic, Enter, Escape o clic fuera cierra y limpia el filtro.

**3. Mismos sitios**

Fuente de terminal y tema de la interfaz usan el mismo combobox. Cómo
se usa el componente no cambió.

**4. Diseño**

`docs/DESIGN.md` dice que se filtra en el panel abierto, no en el
selector cerrado.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Hacer buscable el tema de la terminal
- Un componente distinto para fuentes y temas
