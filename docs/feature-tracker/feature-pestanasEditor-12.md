# Feature: Pestañas del editor

**Fecha:** 20/08/2026

## Descripción de la feature

Se pueden tener varios archivos abiertos a la vez. Cada uno es una
pestaña. Clic en la pestaña cambia el activo. La cruz cierra esa
pestaña.

Si se cierra una pestaña con cambios sin guardar, sale el mismo aviso
de siempre. Cerrar descarta solo ese archivo. Cancelar se queda.

Hay un icono de deshacer junto al disquete. Solo aparece si se puede
dar un paso atrás. El historial se conserva al cambiar de pestaña.

En el árbol, la cruz se cambió por un bote de basura. Eso borra el
archivo del disco. Borrar también quita la pestaña.

## Implementado exitosamente

**1. Pestañas en el editor**

Al abrir un archivo del árbol se añade una pestaña. Se pueden tener
varios archivos abiertos. Clic en la pestaña cambia el activo. Clic
en la cruz (icono Lucide X) cierra esa pestaña.

Si era la activa, se activa la de la derecha. Si era la última, la de
la izquierda. Si no queda ninguna, el editor vuelve a “Selecciona un
archivo.”

**2. Cerrar pestaña con cambios sin guardar**

Mismo modal que al cerrar la app. Texto: “Este archivo tiene cambios
sin guardar…”. El botón Cerrar descarta solo ese archivo. Cancelar se
queda.

Cerrar la app, ir a Inicio o cambiar de carpeta sigue usando el texto
de varios archivos y el botón Salir. Eso descarta todos.

**3. Icono Undo junto al disquete**

Aparece a la derecha de la wordmark “idioteque”, junto al disquete.
Solo se ve si CodeMirror puede deshacer (undoDepth > 0). Clic da un
paso atrás (Ctrl+Z).

El historial se conserva al cambiar de pestaña (un EditorState por
ruta). Al cerrar o borrar la pestaña, se tira ese historial.

**4. Bote de basura en el árbol**

En el árbol izquierdo, la cruz × se cambió por un bote de basura rojo
(Lucide Trash2, color --danger). Sigue borrando el archivo del disco.
Sale el diálogo nativo “¿Borrar?”. Visible en hover o selección. La
cruz ahora solo cierra pestañas.

**5. Borrar también quita la pestaña**

Borrar un archivo del árbol también quita su pestaña. No sale el
modal de unsaved: el usuario ya confirmó el borrado.

## NO se pudo implementar

Nada. Todo lo previsto quedó hecho.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Persistir pestañas o borradores al cerrar la app
- Ctrl+W, clic medio o reordenar pestañas
- Guardar y cerrar en el modal
- Indicador de sucio en el árbol
