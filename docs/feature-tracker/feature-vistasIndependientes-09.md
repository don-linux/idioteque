# Feature: Vistas independientes y grid de home

**Fecha:** 20/08/2026

## Descripción de la feature

Home y el IDE vivían en el mismo archivo. Un `if` decidía cuál se veía, y un
solo bloque de estilos cubría las dos pantallas. Las cajas del historial se
estiraban a todo el ancho cuando había una sola carpeta.

Ahora cada pantalla tiene su ruta y su layout. Las cajas de home vuelven a
medir 16rem y se acomodan en una grilla.

## Implementado exitosamente

**1. Tres rutas, tres pantallas**

- `/` es solo la selección de carpetas.
- `/workspace` es solo el IDE.
- `/configuracion` sigue siendo ajustes, con su menú y sus secciones.

No se montan una encima de la otra. El layout raíz solo deja los tokens y el
cascarón. El footer y los atajos de la terminal viven en el layout del IDE.

**2. Navegación entre ellas**

Abrir una carpeta (botón o caja del historial) entra a `/workspace`. La casa
del footer cierra la carpeta y vuelve a la grilla. El engrane sigue yendo a
`/configuracion`.

La flecha atrás de Configuración vuelve al IDE si hay carpeta abierta — la
terminal no se apaga — o a home si no hay ninguna.

Si alguien entra a `/workspace` sin carpeta, la app lo manda a `/`.

**3. Grid compacto en home**

Las cajas del historial miden 16rem. Se van acomodando en filas. Una sola
carpeta ya no ocupa todo el ancho.

**4. Pruebas**

Hay constantes de ruta con test. El typecheck del frontend pasó.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Rehacer Configuración (el menú y Terminal se quedan como están)
- Tokens nuevos o cambio de tema
- Extraer más componentes, salvo el corte por ruta
