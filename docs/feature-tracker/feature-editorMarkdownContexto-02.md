# Feature: Abrir cualquier carpeta y ver su markdown

**Fecha:** 17/08/2026

## Descripción de la feature

Cierre de los dos huecos que quedaron en el MVP del editor.

Antes la app solo miraba tres carpetas fijas (`docs/`, `.agents/`, `.opencode/`).
Si abrías otra, aunque tuviera markdown de agentes, no mostraba nada.

Ahora la carpeta que eliges es el workspace: se listan sus archivos markdown,
se puede navegar por las subcarpetas, y se pueden editar con el mismo autoguardado
de siempre.

## Implementado exitosamente

**1. Cualquier carpeta sirve**

Ya no hay una lista cerrada de carpetas permitidas. Abres un directorio y ves
los `.md` que hay dentro, incluidos los que están en la raíz de lo abierto.

El caso que fallaba quedó resuelto: abrir `.cursor` muestra sus reglas y
definiciones de agentes.

**2. Las carpetas ocultas de agentes se ven siempre**

Los directorios de agentes suelen empezar con punto (`.cursor`, `.agents`,
`.opencode`, `.claude`). La app los recorre. No se ocultan por ser “ocultos”.

Solo se saltan carpetas de ruido que no son contexto: `.git`, `node_modules`,
`target`, `dist` y `.svelte-kit`.

**3. La lectura y escritura se quedó en Rust**

No se volvió al plugin de archivos de Tauri para JavaScript. Ese plugin, en
Linux y macOS, no entra en carpetas con punto salvo que se afloje un ajuste
de permisos. Las carpetas de agentes no pueden depender de eso.

Los comandos de Rust ya leen esas carpetas sin configuración extra, y el
frontend solo puede tocar markdown dentro de la carpeta que el usuario abrió.

**4. Los textos de la app ya no hablan de las tres carpetas de ejemplo**

La pantalla de bienvenida pide abrir una carpeta para ver y editar markdown.
Si la carpeta no tiene ninguno, lo dice así, sin mencionar `docs/` ni
`.agents/`.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

Sigue igual que en el MVP:

- Terminal integrada
- Vista previa del markdown
- Elegir a mano qué carpetas mostrar (ahora se detectan solas)
- Recordar la última carpeta abierta
