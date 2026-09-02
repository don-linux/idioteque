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

Aparece al abrir una carpeta, en `/workspace`. Tres zonas: sidebar, editor y, si la pediste, terminal.

### Sidebar

Arriba, solo la ruta de la carpeta abierta (si no cabe, se recorta). Debajo, el árbol de markdown. No hay botones de Inicio, Cambiar ni Configuración aquí: esos viven en el footer.

Si la carpeta no tiene markdown, un texto lo dice. No se ocultan las carpetas de agentes que empiezan con punto.

Si la carpeta abierta tiene subcarpetas, el árbol solo muestra las que el usuario marcó en “Carpetas visibles”. Los `.md` de la raíz siempre aparecen. Sin esa selección, se pinta el árbol completo.

### Editor

Si no hay archivo elegido, el centro dice “Selecciona un archivo.”

Si hay uno abierto, arriba va la ruta y el estado de guardado (sin guardar, guardando, guardado, error). El cuerpo es el editor de markdown a pantalla.

### Terminal

Cerrada al entrar a una carpeta. No se abre sola.

Se pide con el icono del footer o con atajos: `Ctrl+J` la pone abajo, `Ctrl+Alt+J` (o Alt + clic en el icono) a la derecha. El sidebar sigue a altura completa cuando el panel está abajo.

Ocultarla no corta lo que esté corriendo. Volver a Inicio o cambiar de carpeta sí mata la sesión.

Una sola terminal, sin pestañas. El tamaño y el lado del panel no se recuerdan al reiniciar. Fuente, tamaño de letra y tema de colores sí (en Configuración).

El tema por defecto es Tokyo Night Night (el de Ghostty/WezTerm, extras de folke): fondo `#1a1b26`, texto `#c0caf5`. No es el chrome de la app. Se puede cambiar en Configuración.

## Footer del IDE

Solo en la vista IDE. No aparece en la selección de carpetas ni en Configuración.

A la izquierda, la palabra “idioteque”, así escrita, en minúsculas. Si la carpeta abierta tiene subcarpetas, al lado va el icono de carpeta con + (“Carpetas visibles”) para elegir cuáles se ven en el árbol. Se queda pegada abajo.

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

Aún no hay panel. El footer tiene un icono de Git solo para ver si el
canal responde: hover con el nombre del repo y la rama, o “sin
repositorio”. El clic no abre nada.
