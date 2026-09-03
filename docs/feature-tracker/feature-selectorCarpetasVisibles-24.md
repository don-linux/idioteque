# Feature: Selector persistente de carpetas visibles

**Fecha:** 02/09/2026

## Descripción de la feature

Abrir un directorio con muchas subcarpetas saturaba el sidebar: el árbol
se pinta todo expandido y el backend caminaba el disco sin filtro.

Ahora, si la carpeta que abres tiene al menos una subcarpeta, aparece un
modal para elegir cuáles se cargan. Si no tiene subcarpetas, se abre tal
cual. La selección se recuerda en la config local.

## Implementado exitosamente

**1. Modal al abrir una carpeta con subdirectorios**

Al elegir o reabrir una carpeta con hijos (por ejemplo `src`, `docs`,
`.cursor`), sale “Carpetas visibles” con checkboxes. Primera vez, ninguno
marcado. Hay Todas / Ninguna.

Si cancelas o cierras el modal, la carpeta se abre igual, con el árbol
completo. Sale un aviso arriba a la derecha: el árbol puede saturarse;
usa Carpetas visibles, el icono de carpeta con + junto a idioteque.

Si confirmas, el árbol solo camina esas carpetas. Los `.md` de la raíz
siempre se ven, aunque no marques ninguna.

**2. Icono junto al wordmark**

En el footer del IDE, al lado de “idioteque”, un icono de carpeta con +
llamado “Carpetas visibles”. Solo si hay subcarpetas. No es el icono
Carpeta de “Cambiar carpeta”.

Ahí se puede cambiar la selección en cualquier momento. Cancelar no
escribe nada.

**3. Persistencia**

Las carpetas planas solo van al historial de recents, como antes.

Si confirmas el modal, se guarda `workspaceViews` en
`~/.idioteque/config.json`: ruta canónica y `visibleFolders`. Cancelar no
guarda. Reabrir esa ruta aplica lo guardado, sin modal ni toast.

Quitar un recent no borra la vista. El tope es 48 entradas.

**4. Backend**

`list_workspace_dirs` lista solo hijos inmediatos (sin `node_modules`,
`.git`, `target`, `dist`, `.svelte-kit`). `list_context_tree` acepta
`includeDirs` para no caminar lo que no se eligió.

**5. Pruebas**

Pruebas de Rust del listado, del filtro y de `workspaceViews`. Pruebas
del frontend del copy, de cuándo sale el modal y el toast, y de que los
avisos de configuración siguen abajo a la derecha.

El typecheck del frontend pasó. Las pruebas de Rust y Vitest pasaron.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

- Colapsar nodos o virtualizar el árbol
- Elegir carpetas en más de un nivel
- Reabrir sola la última carpeta al arrancar
- Cambiar el icono Carpeta de “Cambiar carpeta”
