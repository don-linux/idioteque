# Feature: Renombrar y borrar desde el árbol

**Fecha:** 03/09/2026

## Descripción de la feature

En el árbol ya se podían crear archivos y carpetas, y borrar un markdown
con el basurero. El clic derecho seguía pintando el menú del browser
(Copiar, Inspeccionar) y no había forma de renombrar ni de borrar una
carpeta. Las letras con cola, como la g de `agents` o la y de
`directory`, se cortaban abajo.

Ahora las filas dejan sitio a esas letras, el clic derecho es de
idioteque, y renombrar o borrar escribe directo al disco.

## Implementado exitosamente

**1. Las letras ya no se cortan**

Las cajas de nombre son un poco más altas. `agents`, `directory` y
nombres parecidos se leen enteros. Si el nombre no cabe a lo ancho,
sigue cortándose con puntos suspensivos.

**2. El clic derecho del browser se fue**

En todo el panel del árbol —header, botones, vacío, filas— el menú
nativo no aparece. No se mezcla con el de la app.

**3. Menú de idioteque**

Clic derecho en un archivo o carpeta abre solo dos opciones, en este
orden: Borrar (Delete) y Renombrar (F2). En el resto del explorador no
sale nada. Delete y F2 no se escriben en las filas, la toolbar ni el
footer: solo en ese menú.

**4. Renombrar en el sitio**

Desde el menú o con F2 en la fila enfocada, el nombre se vuelve un
campo. Enter confirma, Escape o salir cancela. A un archivo se le pone
`.md` si falta. No se puede meter una barra para moverlo a otra carpeta.
Si el nombre choca, el aviso sale abajo y se puede corregir. Las
pestañas abiertas y las carpetas desplegadas siguen a la ruta nueva.

**5. Borrar carpetas**

Delete, el menú o el basurero de un archivo piden confirmación y
escriben al disco. En una carpeta el aviso avisa que se va todo lo de
adentro. Se cierran las pestañas de esos archivos. El basurero al pasar
el mouse se queda solo en los archivos.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Atajos F2 / Delete fuera del árbol (el editor y la terminal no los
  capturan)
- Arrastrar y soltar, o renombrar con `/` para mover
- Quitar el basurero hover de los archivos
