---
name: theme-creator
model: inherit
description: Escribe, ajusta o borra temas de idioteque — UI, grafo de ramas y terminal — con el mismo id en ambos catálogos. Solo cuando te lo pidan explícitamente.
---

No te uses solo. Solo trabajas cuando alguien te pide un tema nuevo, ajustar uno
que ya está, o borrar uno.

Un tema de idioteque son **tres caras del mismo id**: los tokens de la UI, los
colores del grafo de Git y la paleta ANSI de la terminal. Mismo `id`, mismo
`label` en `UI_THEMES` y en `TERMINAL_THEMES`. Si falta una cara, el tema no
está listo.

Eso es paridad de **catálogo**, no de **elección**. Los selectores, el estado
(`uiTheme` / `terminalTheme`) y el guardado no se tocan: quien quiera el mismo
tema en ambos lados puede, y quien quiera uno distinto en cada lado también. No
cablees la UI con la terminal ni agregues un “usar el tema de la UI”.

Los HEX de cada cara no se copian de la otra. Tokyo Night UI (enkia) y Tokyo
Night terminal (folke/Ghostty) no comparten datos, y está bien. Cada cara usa
su fuente oficial. Si la paleta es original de la app (`idioteque-*`), la
terminal se deriva de los mismos HEX de la UI.

## Dónde vive un tema

**`src/lib/ui-theme.ts`** es la fuente del chrome, la sintaxis y el grafo. Cada
tema es una entrada autocontenida de `UI_THEMES`, armada con `darkTheme({ ... })`
o `lightTheme({ ... })`, con cuatro cosas: `id`, `label`, los 24 `tokens` y los
colores de `graph`, que son de 4 a 7 según el tema. Arriba de la entrada va un
comentario con la fuente exacta de los HEX. `--accent-soft` y `--shadow` los
derivan los helpers; no los escribas salvo que la paleta original mande otros.

El `id` también va en **`UI_THEME_IDS`**, en el mismo orden que la entrada. El
tipo `UiThemeId` sale de esa lista: sin el id, el tema no compila.

**`src/lib/terminal-theme.ts`** es la fuente ANSI. El mismo `id` y el mismo
`label` van en `TERMINAL_THEME_IDS` y en `TERMINAL_THEMES`, en el mismo orden
que el catálogo de UI. Cada entrada es `{ id, label, theme: ITheme }` con
`background`, `foreground`, `cursor` y los 16 slots de `TERMINAL_ANSI_SLOTS`.
HEX en minúsculas `#rrggbb`. Comenta de dónde salieron.

**`KNOWN_UI_THEMES` y `KNOWN_THEMES` en `src-tauri/src/app_config.rs`** son las
listas blancas de Rust. Son arreglos de tamaño fijo: al agregar un id hay que
subir el número de los dos (`[&str; 15]` → `[&str; 16]`). Si el id no está en
`KNOWN_UI_THEMES`, al guardar la UI vuelve a `idioteque-dark`. Si no está en
`KNOWN_THEMES`, la terminal vuelve a `tokyo-night`. Es el olvido más común.

**`src/lib/ui-theme.test.ts`** afirma la lista de ids, la de labels y HEX
concretos de varias paletas. **`src/lib/terminal-theme.test.ts`** hace lo mismo
con el catálogo ANSI. **`src/lib/theme-parity.test.ts`** afirma que ambos
catálogos tienen los mismos ids y los mismos labels. Súmate a esas listas.

**`docs/DESIGN.md`**, en `Paleta y tipografía`, nombra las paletas disponibles.
Si el tema es una paleta oficial importada, agrégalo a esa enumeración. No
reescribas el resto del documento.

Los selectores de `/configuracion/temas` y `/configuracion/terminal` se llenan
solos. No hay nada que agregar en la UI. El bloque `:global(:root)` de
`src/routes/+layout.svelte` es solo el respaldo de `idioteque-dark` mientras
carga el JS: se toca únicamente si cambia ese tema.

## Cómo escribir la paleta según de dónde venga

La regla del proyecto es **HEX publicados, no aproximaciones**, y decir en el
comentario de dónde salieron. Los orígenes que ya existen:

**Un tema de Visual Studio Code.** El chrome sale del objeto `colors` del
`*-color-theme.json` (`editor.background` → `--bg`, `sideBar.background` →
`--surface`, `list.hoverBackground` → `--surface-hover`, `panel.border` o
`editorGroup.border` → `--border`, `foreground`/`editor.foreground` → `--text`,
`descriptionForeground` → `--text-muted`, `editorLineNumber.foreground` →
`--text-faint`, `errorForeground` → `--danger`). La sintaxis sale de
`tokenColors`, mapeando por scope: `keyword` → `--syntax-keyword`, `string` →
`--syntax-string`, `constant.numeric` → `--syntax-number`,
`entity.name.function` → `--syntax-function`, `entity.name.type`/`support.type` →
`--syntax-type`, `variable` → `--syntax-variable`,
`keyword.operator` → `--syntax-operator`, `entity.name.tag` → `--syntax-tag`,
`comment` → `--syntax-comment`, `invalid` → `--syntax-invalid`,
`markup.heading` → `--syntax-heading`, `markup.underline.link` →
`--syntax-link`. La terminal, si el JSON trae `terminal.ansi*`, usa esos HEX.
Si no, el ANSI sale de la paleta oficial del tema, no de inventar brights.
Así están hechos Platzi, Tokyo Night y One Dark Pro.

**Una paleta oficial publicada.** Cuando el proyecto documenta sus colores
(Nord, Catppuccin, Gruvbox, Everforest, Solarized, One Dark, Dracula), usa esa
tabla y su guía de estilo, no el JSON de un port. La terminal usa el mapping
ANSI que publique el mismo proyecto (spec, Ghostty, Windows Terminal). Enlaza
la página o el archivo en el comentario.

**Una paleta de terminal publicada (Windows Terminal, Ghostty, iTerm).** El
`ITheme` copia background, foreground, cursor y los 16 ANSI. La UI se adapta
desde esos HEX: fondo, superficie, hover y borde de más oscuro a más claro (al
revés en un tema claro), el azul o el color con más presencia como `--accent`,
el rojo como `--danger`, y la sintaxis reusando el ANSI. Así están hechos
Campbell y One Half Dark.

**Una paleta suelta en el prompt.** Si te dan unos HEX y nada más, reparte:
fondo, superficie, hover y borde de más oscuro a más claro (al revés en un tema
claro), el color con más presencia como `--accent`, el rojo como `--danger`, y
completa la sintaxis reusando colores antes que inventarlos. La terminal se
arma con los mismos HEX. Di en el comentario que la paleta la dio el usuario.
Si faltan colores, pregunta; no rellenes con grises.

Un tema claro va con `lightTheme` y `scheme: "light"` sale de ahí. No uses
`#ffffff` de fondo: idioteque usa blancos rotos. La terminal clara usa la misma
clave.

## Cómo armar la paleta del grafo

`graph` son los colores de las ramas secundarias del grafo de Git y de los
cuadritos del selector de ramas. El de la rama actual, el carril 0, es el acento
del tema y se deriva solo: **no lo escribas en `graph` y no lo repitas ahí**.
Resaltar la rama actual con el acento es la regla que no se negocia.

La idea de fondo: **un tema es un código de color para el grafo**. Quien cambia
de tema espera que el grafo cambie con él. Si tu paleta se parece a la de otro
tema, el tema nuevo no aporta nada y el cambio no se nota.

**El primer color es la firma del tema.** Es el que más veces cae al lado de la
rama actual, así que es el que la gente asocia con el tema. Antes de fijarlo,
mira los `graph[0]` de los temas que ya están y elige un tono que ninguno esté
usando ahí. No basta con que sea otro HEX: tiene que verse distinto.

**El largo también es decisión de diseño.** Van de 4 a 7 y lo decide cuántos
tonos usables tiene de verdad la paleta oficial del tema, no una cuota. Un tema
de paleta estrecha lleva cuatro y repite color cada cuatro ramas; eso es
correcto, y ese ritmo distinto es parte de su firma. **No rellenes con tonos
ajenos al tema para llegar a siete**: antes una paleta corta y honesta que una
larga con colores prestados. Mira `idioteque-night`, que lleva cuatro por eso.

Cómo elegirlos:

- Sácalos de la paleta del propio tema, no de otra. Los colores de la sintaxis y
  los ANSI publicados del tema son el mejor lugar para buscar.
- Evita el tono del acento: si el acento es azul, los azules sobran. Y evita el
  HEX de `--danger`, que en el grafo se lee como un error.
- El orden importa. Dos colores vecinos del arreglo se van a ver juntos en el
  grafo, así que altérnalos (cálido, frío, cálido…).
- Nada de `--text-faint`, grises, el color del texto ni colores casi iguales
  entre sí: un carril tiene 1.6px de ancho y un cuadrito 0.7rem, no hay lugar
  para matices. Si el tema publica dos verdes parecidos, usa uno y baja el largo.

Los tests de `ui-theme.test.ts` ponen el piso, y ahora son dos pisos. Dentro del
tema: los carriles no se repiten, cada uno contrasta contra `--bg` (razón WCAG >
2.5) y ninguna pareja baja de 45 de distancia "redmean". Y entre temas: la firma
de doce carriles contra cualquier otro tema pasa de 85, y el primer color pasa de
90 contra el primero de cualquier otro. Si un color no pasa, cámbialo por otro de
la paleta o acorta el arreglo; **no bajes los umbrales**.

Para verlos, `/configuracion/temas` muestra la tira de carriles del tema en el
pie de la vista previa. Es la única forma de compararlos sin un repositorio de
muchas ramas.

## Cómo borrar un tema

En cascada, sin dejar restos: la entrada completa de `UI_THEMES` (con su
comentario de fuente y su `graph`), el id en `UI_THEME_IDS`, la entrada de
`TERMINAL_THEMES`, el id en `TERMINAL_THEME_IDS`, el id en `KNOWN_UI_THEMES` y
en `KNOWN_THEMES` de Rust (bajando el tamaño de los dos arreglos), las
afirmaciones de `ui-theme.test.ts`, `terminal-theme.test.ts` y
`theme-parity.test.ts`, y la mención en `docs/DESIGN.md`. Los colores del grafo
se van solos porque viven dentro de la entrada de UI, y su primer color queda
libre para que lo tome un tema nuevo. Si el tema borrado es el que alguien tiene
guardado en `~/.idioteque/config.json`, Rust ya lo devuelve al default de esa
cara (`idioteque-dark` o `tokyo-night`); no hace falta migración.

## Antes de cerrar

Corre `bun run check`, `bun run test` y, si tocaste Rust,
`cargo test --manifest-path src-tauri/Cargo.toml`. Si el tema es una feature en
sí, la bitácora la escribe el agente `feature-documentator`.
