use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

const MARKDOWN_EXTENSION: &str = "md";

/// Noise we never walk. Hidden agent dirs (`.cursor`, `.agents`, …) are not here.
const SKIP_DIRS: [&str; 5] = [".git", "node_modules", "target", "dist", ".svelte-kit"];

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

fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

fn sort_by_name(nodes: &mut [TreeNode]) {
    nodes.sort_by_key(|node| node.name.to_lowercase());
}

fn child_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

/// Rejects `..`, `.`, and absolute paths before anything reaches the filesystem.
fn join_relative(root: &str, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);

    if relative.as_os_str().is_empty() {
        return Err("Ruta inválida".to_string());
    }

    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
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
        let path = child_path(prefix, &name);

        if file_type.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }

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

    Ok(collect_children(root_path, ""))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQ: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let seq = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("idioteque-ws-{nanos}-{seq}"));
            fs::create_dir_all(&root).expect("create fixture root");
            Self { root }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn write_md(&self, relative: &str, contents: &str) {
            let path = self.path(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(path, contents).expect("write markdown");
        }

        fn mkdir(&self, relative: &str) {
            fs::create_dir_all(self.path(relative)).expect("create dir");
        }

        fn root_str(&self) -> String {
            self.root.to_string_lossy().into_owned()
        }

        fn tree(&self) -> Vec<TreeNode> {
            list_context_tree(self.root_str()).expect("list tree")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn names(nodes: &[TreeNode]) -> Vec<&str> {
        nodes.iter().map(|node| node.name.as_str()).collect()
    }

    fn find<'a>(nodes: &'a [TreeNode], name: &str) -> &'a TreeNode {
        nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing node `{name}`"))
    }

    #[test]
    fn tree_includes_hidden_agent_dirs_and_docs() {
        let fixture = Fixture::new();
        fixture.write_md("README.md", "# root\n");
        fixture.write_md("docs/guide.md", "# guide\n");
        fixture.write_md(".cursor/rules/agent.md", "# cursor\n");
        fixture.write_md(".agents/persona.md", "# agent\n");

        let tree = fixture.tree();
        assert_eq!(names(&tree), [".agents", ".cursor", "docs", "README.md"]);

        let cursor = find(&tree, ".cursor");
        assert_eq!(cursor.path, ".cursor");
        assert_eq!(find(&cursor.children, "rules").path, ".cursor/rules");
        assert_eq!(
            find(&find(&cursor.children, "rules").children, "agent.md").path,
            ".cursor/rules/agent.md"
        );
    }

    #[test]
    fn tree_skips_node_modules() {
        let fixture = Fixture::new();
        fixture.write_md("docs/note.md", "# note\n");
        fixture.write_md("node_modules/pkg/README.md", "# hidden by skip\n");

        let tree = fixture.tree();
        assert_eq!(names(&tree), ["docs"]);
        assert!(tree.iter().all(|node| node.name != "node_modules"));
    }

    #[test]
    fn join_relative_rejects_parent_escape() {
        let fixture = Fixture::new();
        fixture.write_md("docs/note.md", "# note\n");

        let error = join_relative(&fixture.root_str(), "../secret.md").unwrap_err();
        assert_eq!(error, "Ruta inválida");

        let error = join_relative(&fixture.root_str(), "docs/../secret.md").unwrap_err();
        assert_eq!(error, "Ruta inválida");
    }

    #[test]
    fn resolve_markdown_allows_hidden_and_rejects_non_md() {
        let fixture = Fixture::new();
        fixture.write_md(".cursor/rules/agent.md", "ok\n");
        fixture.mkdir("docs");
        fs::write(fixture.path("docs/notes.txt"), "nope").expect("write txt");

        let hidden = resolve_markdown(&fixture.root_str(), ".cursor/rules/agent.md")
            .expect("hidden markdown");
        assert!(hidden.ends_with("agent.md"));

        let error = resolve_markdown(&fixture.root_str(), "docs/notes.txt").unwrap_err();
        assert_eq!(error, "Solo se pueden abrir archivos .md");
    }

    #[test]
    fn read_and_write_markdown_roundtrip() {
        let fixture = Fixture::new();
        fixture.write_md(".cursor/rules/agent.md", "before\n");

        let root = fixture.root_str();
        let path = ".cursor/rules/agent.md".to_string();

        assert_eq!(
            read_markdown(root.clone(), path.clone()).expect("read"),
            "before\n"
        );

        write_markdown(root.clone(), path.clone(), "after\n".to_string()).expect("write");
        assert_eq!(read_markdown(root, path).expect("reread"), "after\n");
    }
}
