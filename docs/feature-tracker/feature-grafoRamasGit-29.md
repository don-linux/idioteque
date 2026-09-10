# Feature: Grafo de ramas Git

**Fecha:** 10/09/2026

## Descripción de la feature

En el mismo espacio del árbol de archivos ahora se puede ver un grafo
de las ramas de Git, al estilo Visual Studio Code.

Se abre con el icono de Git del footer o con Ctrl+G. Ctrl+B vuelve al
árbol.

Arriba hay un selector con todas las ramas locales. La rama actual
siempre está y se pueden marcar otras para comparar.

El grafo muestra commits con nombre y un trozo del hash, los merges, y
hasta qué commit coinciden las ramas con la actual (aunque una esté
adelantada o atrasada).

## Implementado exitosamente

**1. El grafo vive donde estaba el árbol**

Se alterna grafo y árbol en el mismo panel. El tamaño, si está oculto
y el park de la terminal se conservan.

**2. Cómo se abre y se vuelve**

Ctrl+G o clic en el icono Git del footer abre el grafo. Ctrl+B abre
el árbol. En el editor, Ctrl+G ya no busca el siguiente.

**3. Selector de ramas**

Arriba se eligen las ramas locales a comparar. La actual siempre
queda marcada; las demás se pueden sumar o quitar.

**4. El grafo**

Se ven las líneas de cada rama, los merges y el punto hasta donde
coinciden con la actual. Cada commit se lee como
“First commit (d454323)”: el mensaje y un trozo del hash.

## NO se pudo implementar

Quedó fuera de esta entrega, no son fallas del grafo:

- Stage, commit, checkout o merge desde la interfaz
- Ramas remotas y tags
- Vigilar `.git` para refrescar solo: se refresca al enfocar la
  ventana y al reabrir el grafo
