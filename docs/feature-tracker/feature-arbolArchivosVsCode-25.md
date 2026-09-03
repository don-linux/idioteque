# Feature: Árbol de archivos y layout tipo VS Code

**Fecha:** 02/09/2026

## Descripción de la feature

El árbol de la vista workspace era una lista anidada con viñetas,
márgenes y bordes por nivel. Arriba salía la ruta completa, no se podían
crear archivos ni carpetas desde ahí, y para leer los nombres largos
había que pelearse con un scroll lateral.

Ahora el panel se siente como el de Visual Studio Code: el nombre de la
carpeta arriba, tres botones de acción, filas de ancho completo que se
colapsan y un borde que se arrastra para cambiar el ancho. De paso, la
vista workspace se partió en componentes y el layout se recuerda al
reiniciar.

## Implementado exitosamente

**1. Encabezado con el nombre de la carpeta**

Arriba ya no aparece `/home/fernando/carpeta/subcarpeta`. Solo el nombre
de la carpeta abierta. La ruta completa quedó en el tooltip.

**2. Crear archivo, crear carpeta y refrescar**

Debajo del nombre hay tres botones con iconos de Lucide, el paquete que
ya usa la app. Siguen ahí aunque la carpeta esté vacía, para poder crear
el primer archivo.

Al crear, se abre una fila con un campo de texto dentro del árbol: en la
carpeta seleccionada, junto al archivo seleccionado o en la raíz. Enter
confirma y Escape cancela. A los archivos se les agrega `.md` si el
usuario no lo escribe. Si el nombre ya existe o es inválido, el aviso
sale abajo del árbol y la fila se queda para corregirlo.

**3. Filas de ancho completo**

Se fueron las viñetas, los márgenes y los bordes por nivel. Cada carpeta
o archivo es una fila pegada al inicio de la caja, con sangría sutil por
nivel, flecha (chevron) e icono de carpeta o archivo.

Las subcarpetas se colapsan y empiezan cerradas; los archivos de la raíz
se ven siempre. Las flechas del teclado abren y cierran, y Delete borra
el archivo enfocado.

**4. Ancho arrastrable**

Se quitó el scroll lateral raro. El ancho de la caja se ajusta
arrastrando el borde derecho, como en VS Code. Los nombres que no caben
se recortan con puntos suspensivos.

El editor nunca se queda sin espacio: el árbol deja de crecer antes de
aplastarlo, y si abres la terminal a la derecha y ya no cabe todo, el
árbol cede. Lo mismo si achicas la ventana.

**5. Mostrar y ocultar el árbol**

Un icono nuevo en el footer esconde o muestra el árbol, y `Ctrl+B` hace
lo mismo. Ya se pueden minimizar los dos paneles: la terminal y el
árbol.

Con el foco dentro de la terminal, `Ctrl+B` es de la terminal (es el
prefijo de tmux) y el atajo pasa a ser `Ctrl+Shift+B`, que funciona en
cualquier lado.

**6. La vista workspace en componentes**

El panel del árbol, la fila del árbol, el panel del editor y un
separador arrastrable compartido ahora son piezas aparte. El layout se
rearma según qué paneles estén visibles, así no queda un hueco cuando se
esconde el árbol o la terminal. La terminal se sigue redimensionando
abajo y a la derecha.

**7. Todas las carpetas se ven**

Antes el backend escondía las carpetas que no tenían markdown adentro,
así que una carpeta recién creada era invisible. Ahora se listan todas.
Las únicas que siguen ocultas son las de la lista de exclusión (`.git`,
`node_modules`, `target`, `dist`, `.svelte-kit`), que quedó documentada
en `docs/DIRECTORY-BLACKLIST.md` junto con un subagente nuevo
(`.cursor/agents/directory-blacklist.md`) que se invoca cuando haya que
excluir más directorios.

**8. El layout se recuerda**

En `~/.idioteque/config.json` se guardan el ancho del árbol, si el árbol
está visible, el lado que usó la terminal la última vez y su tamaño en
cada lado (uno para abajo, otro para la derecha). Se escribe al soltar el
arrastre o al alternar un panel, no en cada pixel del movimiento.

El lado guardado no cambia los atajos: `Ctrl+J` sigue poniendo la
terminal abajo y `Ctrl+Alt+J` a la derecha.

Lo que no se guarda: si la terminal estaba abierta — siempre entra
cerrada — ni qué carpetas quedaron desplegadas.

Si hay un filtro de “Carpetas visibles”, crear una carpeta en la raíz la
añade a esa lista para que aparezca en el árbol.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Esto se dejó fuera a propósito, no son fallas:

- Borrar o renombrar carpetas desde el árbol
- Arrastrar y soltar archivos
- Navegación completa con flechas dentro del árbol
- Recordar qué carpetas estaban desplegadas
