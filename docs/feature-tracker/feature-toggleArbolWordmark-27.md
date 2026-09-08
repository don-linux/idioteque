# Feature: Toggle del árbol junto al wordmark

**Fecha:** 08/09/2026

## Descripción de la feature

El icono que muestra u oculta el árbol de archivos ya no vive en la
barra de la derecha del footer, junto a inicio, carpeta, engrane,
terminal y git.

Ahora solo aparece cuando el árbol está oculto, a la izquierda, al lado
de la palabra “idioteque” (antes del icono de carpetas visibles, si
está). Un clic vuelve a mostrar el árbol. Si el árbol se ve, el icono
no está.

Los atajos siguen igual: Ctrl+B oculta y muestra el árbol. Si el foco
está en la terminal, Ctrl+B se lo queda tmux y se usa Ctrl+Shift+B.

## Implementado exitosamente

- El icono del árbol salió de la barra de iconos de la derecha.
- Con el árbol visible, ese icono no se muestra.
- Con el árbol oculto, el icono aparece a la izquierda del footer, junto
  a “idioteque”.
- Un clic en ese icono vuelve a mostrar el árbol.
- Ctrl+B y Ctrl+Shift+B (con el foco en la terminal) no cambiaron.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.
