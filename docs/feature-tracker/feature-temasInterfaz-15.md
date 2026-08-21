# Feature: Temas de la interfaz

**Fecha:** 21/08/2026

## Descripción de la feature

En Configuración hay una sección nueva: Temas. Ahí se elige el color de
la app, no el de la terminal.

El default es Idioteque-dark, una paleta original más suave. Tokyo-dark
deja el chrome de antes (el inspirado en Tokyo Night). Idioteque-light
usa blancos rotos, no blanco puro.

Al hacer clic se ve el cambio en la página y en una vista previa del
IDE con un markdown de ejemplo. Hay que pulsar Guardar o Ctrl+S para
que quede. Si se sale sin guardar, vuelve el tema que ya estaba.

## Implementado exitosamente

**1. Tres temas**

Idioteque-dark (default), Tokyo-dark (los colores de antes) e
Idioteque-light. El editor de markdown también pinta con esos tokens.

**2. Sección Temas**

El menú de Configuración tiene Temas. El selector es el mismo combobox
que el de fuentes. Abajo hay una miniatura del IDE con markdown de
ejemplo.

**3. Borrador, no guardado automático**

Elegir un tema cambia lo que se ve. El disco no se toca hasta Guardar
o Ctrl+S. Si se sale, el borrador se tira.

**4. Se recuerda al volver**

El tema vive en `~/.idioteque/config.json`. Si el id no se reconoce,
vuelve a Idioteque-dark. Cambiar la fuente o el tema de la terminal no
borra el de la interfaz.

**5. Pruebas**

Hay pruebas del catálogo, del borrador y de la persistencia. El
typecheck del frontend pasó.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Cambiar los colores ANSI de la terminal al cambiar el tema de la UI
- Fuente del editor o de la interfaz
- Importar un archivo de tema
