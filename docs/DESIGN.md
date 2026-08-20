# Decisiones de diseño

Bitácora de cómo se ve idioteque y por qué. No es el detalle de implementación.

## Paleta y tipografía

Tema oscuro fijo. Los colores viven como tokens en la app:

- Fondo: `#14161a`
- Superficie (sidebar, cajas): `#191c21`
- Hover: `#22262d`
- Borde: `#2a2f37`
- Texto: `#e4e6ea`
- Texto secundario: `#9aa1ad`
- Texto apagado: `#666d79`
- Acento: `#7aa2f7` (también en un velo suave para estados activos)
- Peligro: `#f7768e`

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

### Editor

Si no hay archivo elegido, el centro dice “Selecciona un archivo.”

Si hay uno abierto, arriba va la ruta y el estado de guardado (sin guardar, guardando, guardado, error). El cuerpo es el editor de markdown a pantalla.

### Terminal

Cerrada al entrar a una carpeta. No se abre sola.

Se pide con el icono del footer o con atajos: `Ctrl+J` la pone abajo, `Ctrl+Alt+J` (o Alt + clic en el icono) a la derecha. El sidebar sigue a altura completa cuando el panel está abajo.

Ocultarla no corta lo que esté corriendo. Volver a Inicio o cambiar de carpeta sí mata la sesión.

Una sola terminal, sin pestañas. El tamaño y el lado del panel no se recuerdan al reiniciar. Fuente y tamaño de letra sí (en Configuración).

Los 16 colores ANSI son Tokyo Night, fijos: el azul del prompt es el acento `#7aa2f7` y el amarillo de git es `#e0af68`. No se eligen en Configuración.

## Footer del IDE

Solo en la vista IDE. No aparece en la selección de carpetas ni en Configuración.

A la izquierda, la palabra “idioteque”, así escrita, en minúsculas. Se queda pegada abajo.

A la derecha, una barra de iconos. Sin texto. Cada uno tiene tooltip.

Orden por defecto:

1. Casa — Inicio. Vuelve a la grilla de carpetas. Eso cierra la terminal.
2. Carpeta — Cambiar. Abre el selector nativo para otra carpeta.
3. Engrane — Configuración. Va a la página de ajustes. El workspace no se cierra, así la terminal no se apaga.
4. Terminal — Muestra u oculta el panel. Queda marcado si está visible.

El usuario puede arrastrar los iconos para cambiar el orden. Ese orden se guarda y vuelve a salir igual al abrir la app. Si más adelante se suman iconos nuevos, aparecen al final sin desarmar el orden que ya eligió.

Un arrastre no dispara la acción del icono. Hay que soltar y hacer clic.

## Configuración

Es una página completa (`/configuracion`), no un panel encima del IDE. Flecha atrás arriba a la izquierda, el título, y a la derecha el botón “Guardar configuración” con el icono de disquete. Si hay una carpeta abierta, esa flecha vuelve al IDE (`/workspace`) y no apaga la terminal. Si no hay carpeta, vuelve a la grilla.

A la izquierda, un menú con las secciones. A la derecha, el contenido de la que elegiste. Si no hay ninguna, el centro dice “Elige una opción para empezar a configurar”. Volver al engrane no recuerda la última sección.

Cada sección es su propia página. Hoy solo está Terminal (`/configuracion/terminal`). Fuente del sistema (se puede filtrar escribiendo), tamaño de 10 a 24 píxeles con +/−, y una vista previa (`❯ git status`) para ver si una Nerd Font pinta los iconos.

Si una sección crece, esa lista hace scroll. Las otras no aparecen mezcladas.

Los cambios se quedan en un borrador. Hay que pulsar “Guardar configuración” o Ctrl+S. Entonces se escriben, sale un aviso abajo a la derecha y, al volver al IDE, la terminal ya usa esa fuente y ese tamaño. Si sales sin guardar, el borrador se descarta.

No hay, a propósito, fuente del editor ni de la interfaz, ni elección de shell, ni tema de colores.
