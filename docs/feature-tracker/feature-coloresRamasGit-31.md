# Feature: Colores de las ramas Git

**Fecha:** 14/09/2026

## Descripción de la feature

Un tema de la interfaz es también un código de color para el grafo de
ramas de Git. Cada tema trae su propia paleta y ningún tema comparte
paleta con otro: cambiar de tema repinta el grafo con colores distintos
de verdad.

El carril 0 es siempre el acento del tema y es de la rama actual, nadie
más. Los secundarios se reparten entre las demás ramas.

El largo de la paleta lo decide cada tema: de cuatro a siete colores
secundarios, según cuántos tonos usables tenga de verdad su paleta
oficial. Ese largo es parte de la firma del tema, porque marca cada
cuántas ramas se repite un color: con siete la repetición cae a la
octava rama, con cuatro a la quinta.

## Implementado exitosamente

**1. Cada tema define su propia paleta del grafo**

La paleta vive dentro de la entrada del tema en `src/lib/ui-theme.ts`,
junto a su chrome y su sintaxis. No hay tabla aparte: un tema nuevo
llega con sus colores y uno borrado se los lleva en cascada, sin dejar
configuración huérfana.

**2. El carril 0 es el acento del tema**

Es el color de la rama actual y de nadie más. No se escribe a mano: se
deriva del tema, así que nunca puede quedar desalineado con el resto de
la interfaz.

**3. El largo de la paleta habla del tema**

Idioteque Night lleva cuatro secundarios porque su clave es estrecha y
estirarla pedía colores que no son suyos. Idioteque Dark y Catppuccin
Mocha llevan siete. Cada tema lleva los que su paleta oficial aguanta.

**4. El primer secundario es la firma del tema**

Es el color que más veces cae al lado de la rama actual, así que es el
que más se nota. Cada uno de los once temas tiene el suyo y ninguno lo
repite.

**5. Los colores salen de la paleta publicada de cada tema**

Se usan los HEX oficiales, no aproximaciones. Se evitan el tono del
acento, el HEX de `--danger` (en el grafo se leería como un error), el
color del texto y los tonos casi iguales entre sí.

**6. Los colores llegan al CSS**

Al aplicar un tema se escriben ocho variables de carril
(`--graph-lane-0` a `--graph-lane-7`). Se escriben las ocho siempre,
aunque el tema traiga menos colores: los estilos inline sobreviven al
cambio de tema y una ranura sin escribir se quedaría con el color del
tema anterior. Las que sobran repiten la paleta desde el principio.

**7. El reparto de carriles no sabe de temas**

`branchLaneColors` y `assignLanes` (en `src/lib/git-branch-colors.ts` y
`src/lib/git-lanes.ts`) reciben cuántos secundarios hay como argumento y
reparten ordinales. `GitGraphPanel` es el único que conoce el tema
activo, y lo lee de la misma fuente que usa `applyTheme`, así que el
reparto de colores y las variables CSS que los pintan nunca hablan de
paletas distintas.

**8. El acento no se presta**

Un índice de carril que se pasa del largo de la paleta vuelve al primer
secundario, nunca al acento: ese lugar es de la rama actual.

**9. Cuadritos de color en el selector de ramas**

Cada fila del selector es un cuadrito del color de la rama más su
nombre. El cuadrito hace de marca de comparación: con contorno si la
rama no está comparada, relleno si lo está. La rama actual siempre
aparece rellena, en acento.

Un solo elemento dice dos cosas: qué color le toca a esa rama en el
grafo y si está entrando en la comparación.

**10. Cada commit se queda con el color de quien lo reclama primero**

El orden es la rama actual, después las comparadas y al final el resto.
Por eso el tronco de la rama actual queda entero en acento. Lo que no
cuelga de ninguna rama listada toma un color libre, nunca uno que ya
esté en pantalla.

**11. Vista previa de la paleta en Configuración → Temas**

El pie de la vista previa muestra la tira de carriles del tema, un
punto por color, con el de la rama actual hueco igual que el nodo de
HEAD en el grafo real. Existe porque el grafo de verdad solo muestra
tantos colores como ramas tenga el repositorio abierto, y en un repo de
una sola rama no habría forma de comparar la paleta de un tema contra
otra al momento de elegir. Vive en `previewLanes` de
`src/lib/theme-preview.ts` y `ThemePreview.svelte`.

**12. Tests**

La no-convergencia entre temas está afirmada, no confiada al criterio de
quien escriba el siguiente tema.

Dentro de cada tema se comprueba que los carriles no se repitan, que
cada uno contraste contra el fondo (razón WCAG por encima de 2.5) y que
ninguna pareja baje de 45 de distancia perceptual “redmean”.

Entre temas se comprueba que la paleta completa de cada tema, comparada
carril contra carril contra la de cualquier otro, pase de 85, y que el
primer secundario pase de 90. Los valores reales alcanzados son 136 y
101, así que hay margen. También se comprueba que los largos no sean
todos iguales, porque si lo fueran el largo no estaría diciendo nada del
tema.

**13. Documentación y un agente para los temas**

`docs/DESIGN.md` lo recoge en “Paleta y tipografía” y en la subsección
“Colores de las ramas”, dentro de “Git”.

El agente `theme-creator` (en `.cursor/agents/theme-creator.md`) tiene
las reglas para escribir la paleta de un tema nuevo: elegir un primer
secundario que ningún otro tema use, decidir el largo según la riqueza
real de la paleta oficial y no rellenar con tonos ajenos para llegar a
siete.

**14. Verificación**

`bun run check` sin errores y `bun run test` con 33 archivos y 370
pruebas en verde.

## NO se pudo implementar

Nada de lo previsto se quedó fuera. Estas cosas quedaron a propósito
para otro momento:

- Los temas de la terminal no se tocaron: son un catálogo aparte y no
  tienen nada que ver con el grafo.
- El tronco de la rama actual es el acento del tema, y ocho de los once
  temas tienen acento azulado. En un repositorio de una sola rama el
  tronco se seguirá viendo azul entre esos temas; lo que cambia de forma
  evidente son los secundarios. Soltar el acento sería otra decisión de
  diseño.
- Dos temas pueden compartir un HEX suelto cuando comparten paleta de
  origen (Idioteque Night nació inspirado en Tokyo Night). Lo
  garantizado es que la paleta completa y el primer color nunca
  coinciden.
- Ramas remotas y tags siguen fuera del selector.
- El backend de Rust no cambió: los colores son decisión del frontend de
  principio a fin.
