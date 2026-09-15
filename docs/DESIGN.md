# Decisiones de diseño

Bitácora de cómo se ve idioteque y por qué. No es el detalle de implementación.

## Paleta y tipografía

Los colores viven como tokens. Se pueden cambiar en Configuración → Temas. El default es Idioteque Dark, la paleta original de la app.

**Idioteque Dark** (default):

- Fondo: `#1c1e22`
- Superficie (sidebar, cajas): `#24272d`
- Hover: `#2c3038`
- Borde: `#3a404a`
- Texto: `#d2d5db`
- Texto secundario: `#8f96a1`
- Texto apagado: `#6a7080`
- Acento: `#7b9ee8` (también en un velo suave para estados activos)
- Peligro: `#e08b99`

**Idioteque Night** conserva el chrome anterior. Es una paleta original inspirada en Tokyo Night, no un port: fondo `#14161a`, texto `#e4e6ea`, acento `#7aa2f7`, peligro `#f7768e`.

**Idioteque Light** usa blancos rotos neutros (no `#fff`): fondo `#f2f3f5`, texto `#2c3038`, acento `#3d6ec9`.

También hay paletas oficiales importadas (HEX publicados, no aproximaciones): Platzi (Green Mode de `platzi/platzi-theme`), Tokyo Night, Catppuccin Mocha, Nord, Gruvbox Dark, Everforest Dark Medium, One Dark y Solarized Dark. La terminal sigue su catálogo aparte.

Un tema es también un código de color para el grafo de Git (ver `Git`): cambiar de tema cambia el código completo, y ningún tema comparte su paleta con otro. No son un catálogo aparte: viven dentro del tema, así que un tema nuevo llega con sus colores del grafo y un tema borrado se los lleva.

Inter para la interfaz. JetBrains Mono para rutas, editor y terminal.

Botones e iconos son chicos, sin relleno fuerte. El acento aparece al pasar el mouse o cuando algo está activo (por ejemplo, la terminal abierta).

## Tres pantallas, tres rutas

Home, el IDE y Configuración no comparten página ni layout. Cada una tiene la suya. El layout raíz solo deja los tokens y el cascarón; no mete el footer ni los atajos del IDE.

- `/` — selección de carpetas
- `/workspace` — el IDE
- `/configuracion` — ajustes

Si alguien entra a `/workspace` sin carpeta abierta, vuelve a `/`.

## Pantalla de inicio

Es la selección de carpetas. No es el IDE. Vive en `/`.

Arriba: el nombre de la app, una línea que explica que hay que abrir una carpeta, el engrane de Configuración y el botón “Abrir carpeta”.

Abajo: una grilla con el historial. Las cajas miden 16rem y se acomodan en filas; no se estiran a todo el ancho. Cada una muestra el nombre de la carpeta y la ruta padre. Si la carpeta ya no existe, se ve apagada y no abre. La X la quita de la lista; no desaparece sola.

Esta pantalla no tiene barra de acciones abajo. El engrane se queda en el header.

## Vista IDE

Aparece al abrir una carpeta, en `/workspace`. Cuatro zonas: el árbol de archivos, el editor, la terminal y el footer. El editor y el footer se muestran siempre. El árbol y la terminal se redimensionan y se pueden ocultar; cuando falta uno, el hueco no queda ahí: el layout se rearma.

### Árbol de archivos

Arriba, solo el nombre de la carpeta abierta, no la ruta completa (`/home/fernando/notas/2026` se ve como `2026`). La ruta entera está en el tooltip. No hay botones de Inicio, Cambiar ni Configuración aquí: esos viven en el footer.

Debajo del nombre, tres botones chicos: crear archivo, crear carpeta y refrescar. El de crear carpeta usa el mismo icono de carpeta con + que “Carpetas visibles” junto a idioteque; no son el mismo botón. Siguen ahí aunque la carpeta esté vacía, porque de ahí sale el primer archivo. Refrescar gira sobre sí mismo, en acento, mientras recarga. Leer la carpeta tarda milisegundos, así que el giro se sostiene: da dos vueltas mínimo (~1.6 s) y, si el disco tarda más, sigue hasta cerrar en vuelta completa. Nunca se corta a media vuelta. Con animaciones reducidas del sistema no gira; solo queda el acento. Crear abre una fila con un campo de texto dentro del árbol: en la carpeta enfocada, junto al archivo enfocado, o en la raíz si el foco está en el vacío (ninguna fila marcada). El archivo abierto en el editor no decide el destino. Enter confirma, Escape o salir del campo cancela. A un archivo se le pone `.md` si no lo trae. Si el nombre choca o es inválido, el aviso sale abajo del árbol y la fila se queda para corregir.

El árbol se lee como el de Visual Studio Code: filas de ancho completo pegadas al inicio de la caja, sin viñetas ni cajas anidadas. Cada nivel entra un poco más con una tabulación sutil. Las carpetas llevan flecha y se abren o cierran al clic; empiezan cerradas. Los archivos de la raíz se ven siempre. Las flechas del teclado abren y cierran.

Clic derecho en el explorador no muestra el menú del browser. En una fila de archivo o carpeta abre el menú de idioteque: Borrar (Delete) y Renombrar (F2). En el vacío del listado (incluido el aviso de carpeta vacía) abre otro: Nuevo archivo y Nueva carpeta, siempre en la raíz, y deja el foco en la raíz (ninguna fila marcada). En el header o la toolbar no aparece nada. Esas teclas solo se escriben dentro del menú de fila; en el resto del workspace no se anuncian. Un clic en una fila abre el archivo o expande la carpeta, y también la deja seleccionada para crear. Un clic en el vacío del listado quita la marca y el foco vuelve a la raíz, para poder crear ahí aunque no haya un markdown en la raíz o el editor tenga otro archivo abierto. Un doble clic en ese hueco no selecciona texto ni la fila más cercana. Doble clic en una fila, F2 con la fila enfocada, o Renombrar en el menú, abre el rename en el sitio. Delete borra, con confirmación. Delete también funciona en carpetas: se va la carpeta y todo lo que tenga adentro. El basurero al pasar el mouse sigue en los archivos. Renombrar es el mismo campo inline de crear, con el nombre actual; no se puede mover a otra carpeta con `/`. Arrastrar un archivo o una carpeta a otra carpeta, a un archivo (cae en el padre) o al vacío del árbol (la raíz) sí la mueve. Todo eso escribe directo al disco.

Se muestran todas las carpetas, con markdown dentro o sin él, salvo las de la lista de exclusión (ver `DIRECTORY-BLACKLIST.md`). De archivos, solo markdown. No se ocultan las carpetas de agentes que empiezan con punto. Si no hay nada, un texto lo dice.

El ancho lo decide el usuario, arrastrando el borde derecho. No hay scroll lateral raro: los nombres que no caben se recortan con puntos suspensivos, y si el usuario quiere la caja angosta y los nombres cortados, es su decisión.

El editor nunca queda sin espacio: el árbol deja de crecer antes de aplastarlo, con la terminal a la derecha o sin ella. Y si abres la terminal a la derecha y ya no cabe todo, el árbol cede y se queda con ese ancho; no rebota al cerrar la terminal.

El árbol se esconde con `Ctrl+B`. Cuando está oculto, junto a la palabra “idioteque” aparece el icono de panel izquierdo para volver a mostrarlo. Con el foco dentro de la terminal, `Ctrl+B` es de la terminal (es el prefijo de tmux) y el atajo pasa a ser `Ctrl+Shift+B`, que funciona en cualquier lado.

Si la carpeta abierta tiene subcarpetas, el árbol solo muestra las que el usuario marcó en “Carpetas visibles”. Los `.md` de la raíz siempre aparecen. Sin esa selección, se pinta el árbol completo.

### Editor

Si no hay archivo elegido, el centro dice “Selecciona un archivo.”

Si hay uno abierto, arriba va la ruta y el estado de guardado (sin guardar, guardando, guardado, error). El cuerpo es el editor de markdown a pantalla.

### Terminal

Cerrada al entrar a una carpeta. No se abre sola.

Se pide con el icono del footer o con atajos: `Ctrl+J` la pone abajo, `Ctrl+Alt+J` (o Alt + clic en el icono) a la derecha. El árbol sigue a altura completa cuando el panel está abajo.

Ocultarla no corta lo que esté corriendo. Volver a Inicio o cambiar de carpeta sí mata la sesión.

Una sola terminal, sin pestañas. Se redimensiona arrastrando su borde, igual que el árbol.

### Lo que el layout recuerda

El ancho del árbol, si el árbol está visible, el lado que usó la terminal la última vez y su tamaño en cada lado (uno para abajo, otro para la derecha) se guardan en `~/.idioteque/config.json` y vuelven al reiniciar. Se escriben al soltar el arrastre o al alternar un panel, no en cada pixel del movimiento. Las carpetas visibles de cada workspace (`workspaceViews`) también se recuerdan.

El lado guardado no cambia los atajos: `Ctrl+J` sigue poniendo la terminal abajo y `Ctrl+Alt+J` a la derecha. Lo que se recupera es el tamaño con el que quedó cada lado.

Lo que no se guarda: si la terminal estaba abierta (entra cerrada, siempre) ni qué carpetas del árbol estaban desplegadas.

El tema por defecto es Tokyo Night Night (el de Ghostty/WezTerm, extras de folke): fondo `#1a1b26`, texto `#c0caf5`. No es el chrome de la app. Se puede cambiar en Configuración.

## Footer del IDE

Solo en la vista IDE. No aparece en la selección de carpetas ni en Configuración.

A la izquierda, la palabra “idioteque”, así escrita, en minúsculas. Si el árbol está oculto, al lado va el icono de panel izquierdo para volver a mostrarlo (también `Ctrl+B`, o `Ctrl+Shift+B` con el foco en la terminal). Si la carpeta abierta tiene subcarpetas, después va el icono de carpeta con + (“Carpetas visibles”) para elegir cuáles se ven en el árbol. Se queda pegada abajo.

A la derecha, una barra de iconos. Sin texto. Cada uno tiene tooltip.

Orden fijo, definido en código (no en la UI ni en la config):

1. Casa — Inicio. Vuelve a la grilla de carpetas. Eso cierra la terminal.
2. Carpeta — Cambiar. Abre el selector nativo para otra carpeta.
3. Engrane — Configuración. Va a la página de ajustes. El workspace no se cierra, así la terminal no se apaga.
4. Terminal — Muestra u oculta el panel. Queda marcado si está visible.
5. Git — Icono de vida. Al pasar el mouse dice si no hay repo o el nombre
   de la carpeta y la rama. El clic no hace nada. No es un panel.

El usuario no reordena. No hay arrastre ni orden guardado. Si se suma un icono, se mete en esa lista de código.

Al pasar el mouse, Casa, Carpeta, Engrane y Terminal muestran el cursor de clic. Git no es accionable, así que el cursor se queda normal. El clic corre siempre: no hay umbral ni “¿era un arrastre?”.

## Configuración

Es una página completa (`/configuracion`), no un panel encima del IDE. Flecha atrás arriba a la izquierda, el título, y a la derecha el botón “Guardar configuración” con el icono de disquete. Si hay una carpeta abierta, esa flecha vuelve al IDE (`/workspace`) y no apaga la terminal. Si no hay carpeta, vuelve a la grilla.

A la izquierda, un menú con las secciones. A la derecha, el contenido de la que elegiste. Si no hay ninguna, el centro dice “Elige una opción para empezar a configurar”. Volver al engrane no recuerda la última sección.

Cada sección es su propia página. Hoy hay Terminal y Temas.

Terminal (`/configuracion/terminal`): fuente del sistema (dropdown; se filtra escribiendo en el panel abierto), tamaño de 10 a 24 píxeles con +/−, un selector de tema (Tokyo Night y otras paletas oficiales), y una vista previa con la paleta ANSI más una terminal xterm de solo lectura (prompt idle) para ver el tema y el tamaño aplicados.

Temas (`/configuracion/temas`): dropdown de la paleta de la interfaz (Idioteque Dark, Idioteque Night, Idioteque Light y las paletas oficiales); se filtra escribiendo en el panel abierto. Abajo hay una vista previa del IDE con markdown de ejemplo. Al elegir se ve el cambio; hay que guardar para que quede.

Si una sección crece, esa lista hace scroll. Las otras no aparecen mezcladas.

Los cambios se quedan en un borrador. Hay que pulsar “Guardar configuración” o Ctrl+S. Entonces se escriben, sale un aviso abajo a la derecha y, al volver al IDE, la terminal ya usa esa fuente, ese tamaño y ese tema, y la interfaz el color elegido. Si sales sin guardar, el borrador se descarta.

No hay, a propósito, fuente del editor ni de la interfaz, ni elección de shell, ni importar un archivo de tema.

## Git

Git no vive en el frontend ni en una librería embebida. Rust lanza el
`git` del sistema, parsea porcelain, y Svelte solo pinta el resultado.

El módulo es nuestro (`src-tauri/src/git`). No es un port de VS Code ni
de Zed. De Zed copiamos cómo se lanza el binario (sin shell, sin pager,
sin locks opcionales, sin prompts). De VS Code, la idea: la UI es un
modelo (rama, staged, dirty), no un eco de comandos. El formato que
leemos es porcelain v2; VS Code todavía usa v1.

Si la carpeta no es un repo, o no hay Git, el snapshot viene vacío.
No es un error. El panel podrá esconderse.

El icono de Git del footer (y Ctrl+G) abre el grafo de ramas en el
mismo hueco del árbol de archivos. Ctrl+B vuelve al árbol. El hover
sigue mostrando el nombre del repo y la rama.

### Colores de las ramas

Cada rama tiene un color, y ese color lo decide el tema. Son el acento
más cuatro a siete secundarios. El acento no se escribe en la paleta
del grafo, se deriva del tema, porque su lugar está reservado: lo usa
la rama actual y nadie más. Los secundarios salen de la paleta
publicada del propio tema, elegidos para contrastar contra el fondo y
entre ellos. Viven dentro de la definición del tema, no en una tabla
aparte: así un tema nuevo llega con sus colores y uno borrado se los
lleva.

El largo varía porque las paletas oficiales no son igual de ricas. Una
con muchos tonos usables da siete; una estrecha da cuatro, y estirarla
pediría colores que no son suyos. Ese largo es parte de la firma del
tema, porque marca cada cuántas ramas se repite un color.

Dos temas nunca se ven igual en el grafo, y eso no depende del criterio
de quien escriba el siguiente: está afirmado por tests. Uno compara la
paleta completa de cada tema contra la de todos los demás; el otro
compara solo el primer secundario, que es el color que más cae al lado
de la rama actual y por eso el que identifica al tema. La regla vieja
pedía el mismo orden de tonos para todos los temas y el resultado eran
once paletas iguales con distinto HEX.

Al elegir tema, el pie de la vista previa de Configuración → Temas
muestra la tira de carriles. Sin ella, la paleta solo se vería en un
repositorio con muchas ramas.

En el selector de ramas cada fila es un cuadrito de color y el nombre.
Ese cuadrito es también la marca de comparación: con contorno si la
rama no está comparada, relleno si lo está. Es un solo control porque
la fila mide 0.7rem de alto y dos cuadritos se leen como ruido. La
rama actual siempre aparece rellena, en acento.

Las ramas toman color en el orden en que Git las lista, dando la
vuelta a los secundarios del tema. Dos ramas vecinas de la lista nunca
comparten color, y la repetición aparece a tantas ramas como colores
tenga el tema: a la octava con siete, a la quinta con cuatro. Que se
repita está bien: los colores son finitos y las ramas de un proyecto
no. Lo que no se repite nunca es el acento. El color tampoco depende de
la selección: marcar o desmarcar una rama no repinta las demás.

En el grafo, cada commit se queda con el color de la primera rama que
lo reclama: primero la actual, después las comparadas, al final el
resto. Por eso el tronco entero de la rama actual queda en acento y la
historia propia de otra rama en el color de esa rama. Lo que no cuelga
de ninguna rama listada —una rama borrada ya fusionada, por ejemplo—
toma un color libre, nunca uno que ya esté en pantalla. El nodo de
HEAD se dibuja hueco.
