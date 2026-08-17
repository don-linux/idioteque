use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

/// The only directories the frontend is ever allowed to see or touch.
const CONTEXT_ROOTS: [&str; 3] = ["docs", ".agents", ".opencode"];
const MARKDOWN_EXTENSION: &str = "md";

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Dir,
    File,
}

#[derive(Serialize)]
pub struct TreeNode {
    name: String,
    /// Slash separated, relative to the opened workspace root.
    path: String,
    kind: NodeKind,
    children: Vec<TreeNode>,
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(MARKDOWN_EXTENSION))
}

fn sort_by_name(nodes: &mut [TreeNode]) {
    nodes.sort_by_key(|node| node.name.to_lowercase());
}

/// Rejects anything that is not a plain descendant of one of the context roots,
/// before the path ever reaches the filesystem.
fn join_relative(root: &str, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    let mut components = relative.components();

    let context_root = match components.next() {
        Some(Component::Normal(name)) => name.to_string_lossy().into_owned(),
        _ => return Err("Ruta inválida".to_string()),
    };

    if !CONTEXT_ROOTS.contains(&context_root.as_str()) {
        return Err(format!("`{context_root}` no es una carpeta de contexto"));
    }

    if components.any(|component| !matches!(component, Component::Normal(_))) {
        return Err("Ruta inválida".to_string());
    }

    Ok(Path::new(root).join(relative))
}

/// Resolves symlinks too, so a linked parent directory cannot escape the root.
fn resolve_markdown(root: &str, relative: &str) -> Result<PathBuf, String> {
    let target = join_relative(root, relative)?;

    let canonical_root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let canonical_target = fs::canonicalize(&target).map_err(|error| error.to_string())?;

    if !canonical_target.starts_with(&canonical_root) {
        return Err("La ruta sale de la carpeta abierta".to_string());
    }

    if !canonical_target.is_file() || !is_markdown(&canonical_target) {
        return Err("Solo se pueden abrir archivos .md".to_string());
    }

    Ok(canonical_target)
}

fn collect_children(directory: &Path, prefix: &str) -> Vec<TreeNode> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut directories = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}/{name}");

        if file_type.is_dir() {
            let children = collect_children(&entry.path(), &path);
            if children.is_empty() {
                continue;
            }

            directories.push(TreeNode {
                name,
                path,
                kind: NodeKind::Dir,
                children,
            });
        } else if is_markdown(&entry.path()) {
            files.push(TreeNode {
                name,
                path,
                kind: NodeKind::File,
                children: Vec::new(),
            });
        }
    }

    sort_by_name(&mut directories);
    sort_by_name(&mut files);
    directories.append(&mut files);
    directories
}

#[tauri::command]
pub fn list_context_tree(root: String) -> Result<Vec<TreeNode>, String> {
    let root_path = Path::new(&root);

    if !root_path.is_dir() {
        return Err(format!("`{root}` no es una carpeta"));
    }

    Ok(CONTEXT_ROOTS
        .iter()
        .filter(|name| root_path.join(name).is_dir())
        .map(|name| TreeNode {
            name: (*name).to_string(),
            path: (*name).to_string(),
            kind: NodeKind::Dir,
            children: collect_children(&root_path.join(name), name),
        })
        .collect())
}

#[tauri::command]
pub fn read_markdown(root: String, path: String) -> Result<String, String> {
    let target = resolve_markdown(&root, &path)?;

    fs::read_to_string(&target).map_err(|error| format!("No se pudo leer `{path}`: {error}"))
}

#[tauri::command]
pub fn write_markdown(root: String, path: String, contents: String) -> Result<(), String> {
    let target = resolve_markdown(&root, &path)?;

    let parent = target
        .parent()
        .ok_or_else(|| "Ruta inválida".to_string())?;
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Ruta inválida".to_string())?;

    // Write to a sibling temp file and rename, so a crash mid-write cannot
    // leave the agent with a truncated context file.
    let temporary = parent.join(format!(".{file_name}.idioteque.tmp"));

    fs::write(&temporary, contents)
        .map_err(|error| format!("No se pudo escribir `{path}`: {error}"))?;

    fs::rename(&temporary, &target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo guardar `{path}`: {error}")
    })
}
