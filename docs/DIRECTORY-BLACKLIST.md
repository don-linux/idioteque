# Directory blacklist

El árbol de archivos muestra **todas** las carpetas de la raíz abierta, tengan
markdown dentro o no. Las únicas que se ocultan son las de esta lista.

## Lista actual

| Carpeta         | Por qué se oculta                                          |
| --------------- | ---------------------------------------------------------- |
| `.git`          | Historial de Git. Miles de archivos internos.              |
| `node_modules`  | Dependencias de JavaScript. Enorme y con markdown ajeno.   |
| `target`        | Compilados de Rust/Cargo.                                  |
| `dist`          | Artefactos de build.                                       |
| `.svelte-kit`   | Archivos generados por SvelteKit.                          |

El match es por **nombre exacto** de la carpeta, en cualquier nivel del árbol, y
es sensible a mayúsculas. No hay patrones ni globs.

Las carpetas ocultas que empiezan con punto (`.cursor`, `.agents`, …) **no** se
ocultan: son contexto de agentes y por eso el editor existe.

## Dónde vive en el código

La lista es la constante `SKIP_DIRS` en
[`src-tauri/src/workspace.rs`](../src-tauri/src/workspace.rs), y se consulta
desde la función `should_skip_dir`. Dos lugares la usan:

- `collect_children`, que arma el árbol para `list_context_tree`.
- `watch_path_matters`, que decide si un evento del vigilante de archivos debe
  refrescar el árbol.

Es un arreglo de tamaño fijo (`[&str; 5]`), así que al agregar una entrada hay
que subir ese número. Los tests `tree_skips_node_modules` y
`watch_ignores_tmp_and_skip_dirs`, en el mismo archivo, cubren el comportamiento.

## Cómo agregar exclusiones

Existe un subagente para esto: [`.cursor/agents/directory-blacklist.md`](../.cursor/agents/directory-blacklist.md).
No es de uso rutinario; se invoca cuando aparece un directorio que mete ruido en
el árbol. Primero cambia el código, después actualiza este documento.
