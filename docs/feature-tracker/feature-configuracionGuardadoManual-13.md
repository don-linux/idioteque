# Feature: Guardado manual de Configuración

**Fecha:** 20/08/2026

## Descripción de la feature

En Configuración, cambiar la fuente o el tamaño de la terminal ya no
escribe el archivo solo. El cambio se queda en un borrador. Hay que
pulsar “Guardar configuración” o Ctrl+S. Entonces se aplica, se escribe
`~/.idioteque/config.json` y sale un aviso abajo a la derecha.

El texto de Terminal ahora dice: “Se aplican al guardar o con Ctrl+S.”

Si se sale de Configuración sin guardar, el borrador se tira. No hay
modal. La terminal del IDE sigue como estaba.

El campo de fuente ahora lleva un chevron a la derecha, para que se lea
como un selector y no como una caja de texto.

## Implementado exitosamente

**1. Borrador, no guardado automático**

Fuente y tamaño se editan en un borrador. La vista previa de la página
usa ese borrador. El disco y la terminal del IDE no cambian hasta
guardar.

Pasar de Terminal al menú de Configuración no pierde el borrador. Solo
se descarta al salir de Configuración.

**2. Guardar a propósito**

El botón “Guardar configuración” está arriba a la derecha, con el icono
de disquete. Es más grande que el del footer. También vale Ctrl+S. Si no
hay cambios, el botón se queda apagado y el atajo no hace nada.

**3. Aviso al guardar**

Si el guardado sale bien, un toast abajo a la derecha dice
“Configuración guardada”. Si falla, el error sigue en la página, como
antes. No hay toast de error.

**4. Chevron en la fuente**

El combobox de fuente tiene un chevron a la derecha. Se queda visible
con la lista cerrada. Al abrir, gira.

**5. Texto y diseño**

El párrafo de Terminal dice que los cambios se aplican al guardar o con
Ctrl+S. `docs/DESIGN.md` ya describe el borrador, el botón y el aviso.

**6. Pruebas**

Hay pruebas de cuándo el borrador está sucio y de cómo se arma el toast.
El typecheck del frontend pasó.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Pedir confirmación al salir con un borrador sin guardar
- Cambiar el guardado del orden de iconos del footer
- Toast de error (el error inline se queda)
