# Feature: Canal CLI de Git

**Fecha:** 27/08/2026

## Descripción de la feature

Antes de un visor o un panel de cambios, hace falta un canal estable
entre idioteque y el `git` del usuario.

Ese canal vive en Rust. El frontend no lanza Git. No se usa libgit2
ni gitoxide. Se pregunta al binario con salida porcelain y se devuelve
un snapshot tipado.

## Implementado exitosamente

**1. Un módulo propio, no un port**

`src-tauri/src/git` es código nuestro. La forma de lanzar Git sigue a
Zed (argv, sin shell, sin pager, sin colgarse pidiendo clave). Las
preguntas siguen a VS Code (`rev-parse`, status). El parseo es
porcelain v2, no el v1 de VS Code.

**2. Dos comandos Tauri**

`git_probe` dice si hay Git y qué versión es. `git_status` abre una
carpeta y devuelve la rama, si está sucia, y la lista de archivos.
Si no hay repo, el repositorio viene vacío. No explota.

**3. El frontend ya tiene el contrato**

`src/lib/git.ts` tipa el snapshot y sabe leer las columnas staged /
unstaged. Todavía no hay UI.

## NO se pudo implementar

Nada de lo previsto se quedó fuera.

## Fuera de alcance

- Panel de cambios, stage, commit, push
- Decoraciones en el árbol
- Rama en el footer
- Credenciales / askpass
- Vigilar `.git` para refrescar solo
