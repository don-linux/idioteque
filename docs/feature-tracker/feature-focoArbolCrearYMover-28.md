# Feature: Foco del árbol, crear y mover

**Fecha:** 08/09/2026

## Descripción de la feature

El árbol ya creaba archivos y carpetas, pero los botones de la toolbar
siempre iban a la raíz o al archivo abierto en el editor. No había forma
de decir “estoy en esta carpeta” sin abrir un markdown. El clic derecho
en el vacío no servía para crear. Y para mover había que esperar: el
rename no acepta `/`.

Ahora el árbol recuerda en qué fila estás. Crear cae ahí. Clic derecho
en el vacío abre Nuevo archivo y Nueva carpeta en la raíz. Se pueden
arrastrar archivos y carpetas a otra carpeta, encima de un archivo
(cae en el padre) o al vacío (la raíz). El rename en el sitio sigue
sin mover con `/`.

De paso, en el editor la selección del mouse se ve (el color de acento
es más visible), el texto llena el panel, y un reload del archivo no
aplasta el rango seleccionado.

## Implementado exitosamente

**1. Foco del árbol, no solo el archivo abierto**

Clic en una carpeta la selecciona y la abre o cierra. Esa fila queda
marcada aunque el editor tenga otro archivo. Los botones Crear archivo
y Crear carpeta de la toolbar usan esa selección: si es una carpeta,
crean adentro; si es un archivo, crean al lado, en el padre. Sin
selección, caen en la raíz.

**2. Clic derecho en el vacío**

Clic derecho en el espacio libre del árbol, o en “Esta carpeta está
vacía”, muestra Nuevo archivo y Nueva carpeta. Siempre en la raíz. Las
filas siguen con Borrar y Renombrar. El header y la toolbar no abren
menú.

**3. Arrastrar y soltar**

Se pueden arrastrar archivos y carpetas a otra carpeta, encima de un
archivo (cae en el padre) o al vacío (la raíz). El movimiento se
escribe al disco. No se puede soltar una carpeta dentro de sí misma.
Si el nombre ya existe, el aviso sale abajo del árbol. Las pestañas
abiertas siguen la ruta nueva. El rename con `/` no se tocó.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Vista WYSIWYG del markdown
- Quitar el basicSetup de CodeMirror
- Mover escribiendo `/` en el rename
