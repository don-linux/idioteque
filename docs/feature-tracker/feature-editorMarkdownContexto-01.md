# Feature: Editor de markdown de contexto (MVP)

**Fecha:** 17/08/2026

## Descripción de la feature

Primer MVP de idioteque, el harness de contexto para agentes de IA.

La idea de idioteque no es ser un IDE de código, sino una herramienta para ver y editar
los archivos de contexto que consumen los agentes (carpetas tipo `docs/`, `.agents/`,
`.opencode/`). Más adelante tendrá una terminal integrada para correr OpenCode o Claude
Code desde la misma app.

En esta entrega se armó lo mínimo para que la app sea usable: abrir una carpeta del
disco, ver sus archivos markdown en una lista lateral, y poder editarlos con guardado
automático.

Hecho con Tauri 2 + SvelteKit + Svelte 5.

## Implementado exitosamente

**1. Limpieza del template de Tauri**

Se borró toda la pantalla de ejemplo que venía por defecto (la que saludaba con "Hello
from Rust"), el comando de saludo del lado de Rust, los logos de Vite, Tauri y Svelte, y
el título genérico de la ventana. Se hizo esto primero, antes de construir nada, para no
arrastrar código muerto.

**2. Abrir una carpeta**

Se abre desde el diálogo nativo del sistema operativo, el mismo que usa cualquier otro
programa.

**3. Árbol lateral de archivos**

Muestra únicamente los archivos `.md` que están dentro de `docs/`, `.agents/` y
`.opencode/`. Si una carpeta no tiene archivos markdown, no se muestra.

**4. Editor de markdown**

Se usó CodeMirror 6, que trabaja sobre el texto plano del archivo.

Se descartó a propósito un editor visual tipo Notion. El motivo: el archivo `.md` en
disco es el contrato con el agente. Un editor visual puede cambiar el formato al guardar
(frontmatter, bloques de código, espacios) y eso rompería los archivos de contexto.

**5. Autoguardado**

Guarda solo, medio segundo después de que dejas de escribir. Además fuerza el guardado
antes de cambiar de archivo, para que nunca se pierda contenido ni se mezcle el texto de
un archivo con otro.

**6. Guardado seguro**

El archivo se escribe primero en uno temporal y después se renombra. Así, si la app se
cae justo a mitad de la escritura, el agente nunca se encuentra con un archivo cortado a
la mitad.

**7. Pruebas**

Se verificó que las carpetas ocultas (`.agents` y `.opencode`, que empiezan con punto) sí
se leen bien, que los archivos que están fuera de esas carpetas quedan bloqueados, y que
al editar un archivo y volver a abrirlo los cambios siguen ahí. La app compila y levanta
sin errores.

## NO se pudo implementar

**1. Las carpetas de contexto quedaron fijas en el código (lo más importante a resolver)**

La lista de carpetas que la app puede mostrar quedó escrita a mano: solo `docs/`,
`.agents/` y `.opencode/`.

Al probar abriendo la carpeta `.cursor` del propio proyecto, la app respondió *"Esta
carpeta no tiene docs/, .agents/ ni .opencode/"* y no mostró nada, a pesar de que esa
carpeta sí tiene archivos de contexto de agentes (reglas y definiciones de agentes en
markdown).

Esto no era la intención. Esas tres carpetas eran **ejemplos** del tipo de carpeta que
queremos visualizar, no una lista cerrada. idioteque no debería estar limitado a ellas.

**Siguiente paso más importante:** que el usuario pueda configurar qué carpetas de
contexto quiere ver, o que la app simplemente detecte cualquier carpeta que tenga
markdown.

**2. El plugin de archivos de Tauri no sirvió para carpetas ocultas**

Se descubrió que el plugin de sistema de archivos de Tauri para JavaScript no puede leer
carpetas que empiezan con punto sin pelearse con su configuración de permisos. Como
`.agents` y `.opencode` son justamente así, hubo que mover toda la lectura y escritura de
archivos al lado de Rust.

Salió bien y de paso quedó más seguro, pero fue un desvío que no estaba previsto y costó
tiempo.

**3. Errores menores que se corrigieron sobre la marcha**

- Una búsqueda inicial de archivos del template no encontró dos logos que también había
  que borrar.
- Un estilo del árbol de archivos no se aplicaba, por cómo funciona el anidamiento de
  componentes.
- Las pruebas del backend fallaron la primera vez porque las tres compartían la misma
  carpeta de prueba y se borraban entre ellas.

**4. Falsas alarmas (sin impacto)**

- Al abrir la app aparecieron avisos de "archivo no encontrado" por los logos viejos que
  el navegador interno tenía guardados en caché.
- Salió una alerta de "error" que en realidad era el cierre manual de la app durante las
  pruebas.

## Fuera de alcance en este MVP

Esto se dejó fuera a propósito, no son fallas:

- Terminal integrada
- Vista previa del markdown al lado del editor
- Grafo de relaciones entre archivos
- Detectar cambios hechos por otros programas mientras la app está abierta
- Recordar la última carpeta abierta
- Crear o borrar archivos desde la app
