use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

const MARKDOWN_EXTENSION: &str = "md";

/// Noise we never walk. Hidden agent dirs (`.cursor`, `.agents`, …) are not here.
const SKIP_DIRS: [&str; 5] = [".git", "node_modules", "target", "dist", ".svelte-kit"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Dir,
    File,
}

#[derive(Debug, Serialize)]
pub struct TreeNode {
    name: String,
    /// Slash separated, relative to the opened workspace root.
    path: String,
    kind: NodeKind,
    children: Vec<TreeNode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDirs {
    pub root: String,
    pub dirs: Vec<String>,
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

fn strip_windows_extended_prefix(path: &str) -> &str {
    match path.strip_prefix(r#"\\?\"#) {
        Some(rest) if !rest.starts_with("UNC\\") && !rest.starts_with("UNC/") => rest,
        _ => path,
    }
}

fn portable_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    strip_windows_extended_prefix(&raw).to_string()
}

/// Immediate child names only: one normal component, no `..` or slashes.
fn include_dir_set(root: &str, include_dirs: &[String]) -> Result<HashSet<String>, String> {
    let mut allowed = HashSet::with_capacity(include_dirs.len());

    for name in include_dirs {
        join_relative(root, name)?;

        if Path::new(name).components().count() != 1 {
            return Err("Ruta inválida".to_string());
        }

        allowed.insert(name.clone());
    }

    Ok(allowed)
}

fn list_immediate_dirs(directory: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if should_skip_dir(&name) {
            continue;
        }

        dirs.push(name);
    }

    dirs.sort_by_key(|name| name.to_lowercase());
    dirs
}

fn collect_children(
    directory: &Path,
    prefix: &str,
    include: Option<&HashSet<String>>,
) -> Vec<TreeNode> {
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

            if let Some(allowed) = include {
                if !allowed.contains(&name) {
                    continue;
                }
            }

            let children = collect_children(&entry.path(), &path, None);
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
pub fn list_workspace_dirs(root: String) -> Result<WorkspaceDirs, String> {
    let root_path = Path::new(&root);

    if !root_path.is_dir() {
        return Err(format!("`{root}` no es una carpeta"));
    }

    let canonical = fs::canonicalize(root_path).map_err(|error| error.to_string())?;

    Ok(WorkspaceDirs {
        root: portable_path(&canonical),
        dirs: list_immediate_dirs(&canonical),
    })
}

#[tauri::command]
pub fn list_context_tree(
    root: String,
    include_dirs: Option<Vec<String>>,
) -> Result<Vec<TreeNode>, String> {
    let root_path = Path::new(&root);

    if !root_path.is_dir() {
        return Err(format!("`{root}` no es una carpeta"));
    }

    let allowed = match include_dirs.as_deref() {
        Some(dirs) => Some(include_dir_set(&root, dirs)?),
        None => None,
    };

    Ok(collect_children(root_path, "", allowed.as_ref()))
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

#[tauri::command]
pub fn delete_markdown(root: String, path: String) -> Result<(), String> {
    let target = resolve_markdown(&root, &path)?;

    fs::remove_file(&target).map_err(|error| format!("No se pudo borrar `{path}`: {error}"))
}

pub struct WatchState {
    inner: Mutex<Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>>,
}

impl Default for WatchState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

/// Whether a filesystem event should refresh the markdown tree.
pub(crate) fn watch_path_matters(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    if relative.as_os_str().is_empty() {
        return true;
    }

    for component in relative.components() {
        let name = component.as_os_str().to_string_lossy();

        if should_skip_dir(&name) {
            return false;
        }

        if name.ends_with(".idioteque.tmp") {
            return false;
        }
    }

    true
}

#[tauri::command]
pub fn watch_workspace(
    app: AppHandle,
    state: State<WatchState>,
    root: String,
) -> Result<(), String> {
    let root_path = PathBuf::from(&root);

    if !root_path.is_dir() {
        return Err(format!("`{root}` no es una carpeta"));
    }

    let canonical = fs::canonicalize(&root_path).map_err(|error| error.to_string())?;
    let watch_root = canonical.clone();

    let mut inner = state.inner.lock().map_err(|error| error.to_string())?;
    *inner = None;

    let mut debouncer = new_debouncer(
        Duration::from_millis(250),
        move |result: DebounceEventResult| {
            let Ok(events) = result else {
                return;
            };

            if events
                .iter()
                .any(|event| watch_path_matters(&watch_root, &event.path))
            {
                let _ = app.emit("workspace-fs", ());
            }
        },
    )
    .map_err(|error| format!("No se pudo vigilar la carpeta: {error}"))?;

    debouncer
        .watcher()
        .watch(&canonical, RecursiveMode::Recursive)
        .map_err(|error| format!("No se pudo vigilar la carpeta: {error}"))?;

    *inner = Some(debouncer);
    Ok(())
}

#[tauri::command]
pub fn unwatch_workspace(state: State<WatchState>) -> Result<(), String> {
    let mut inner = state.inner.lock().map_err(|error| error.to_string())?;
    *inner = None;
    Ok(())
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
            list_context_tree(self.root_str(), None).expect("list tree")
        }

        fn tree_filtered(&self, include: &[&str]) -> Vec<TreeNode> {
            list_context_tree(
                self.root_str(),
                Some(include.iter().map(|name| (*name).to_string()).collect()),
            )
            .expect("list filtered tree")
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

    #[test]
    fn delete_markdown_removes_file_and_rejects_escape() {
        let fixture = Fixture::new();
        fixture.write_md("docs/note.md", "bye\n");

        let root = fixture.root_str();
        delete_markdown(root.clone(), "docs/note.md".to_string()).expect("delete");

        assert!(!fixture.path("docs/note.md").exists());
        assert!(read_markdown(root.clone(), "docs/note.md".to_string()).is_err());

        let escape = delete_markdown(root, "../secret.md".to_string()).unwrap_err();
        assert_eq!(escape, "Ruta inválida");
    }

    #[test]
    fn delete_markdown_rejects_non_md() {
        let fixture = Fixture::new();
        fixture.mkdir("docs");
        fs::write(fixture.path("docs/notes.txt"), "nope").expect("write txt");

        let error = delete_markdown(fixture.root_str(), "docs/notes.txt".to_string()).unwrap_err();
        assert_eq!(error, "Solo se pueden abrir archivos .md");
    }

    #[test]
    fn list_workspace_dirs_lists_immediate_children_only() {
        let fixture = Fixture::new();
        fixture.write_md("README.md", "# root\n");
        fixture.write_md("docs/guide.md", "# guide\n");
        fixture.write_md(".cursor/rules/agent.md", "# cursor\n");
        fixture.write_md("src/nested/deep.md", "# deep\n");
        fixture.mkdir("empty");
        fixture.write_md("node_modules/pkg/README.md", "# skip\n");
        fixture.mkdir("target");

        let listed = list_workspace_dirs(fixture.root_str()).expect("list dirs");
        assert_eq!(listed.dirs, [".cursor", "docs", "empty", "src"]);
        assert!(Path::new(&listed.root).is_dir());
        assert!(!listed.dirs.iter().any(|name| name == "nested"));
        assert!(!listed.dirs.iter().any(|name| name == "node_modules"));
        assert!(!listed.dirs.iter().any(|name| name == "target"));
    }

    #[test]
    fn list_workspace_dirs_rejects_files() {
        let fixture = Fixture::new();
        fixture.write_md("note.md", "x\n");
        let file = fixture.path("note.md");

        let error = list_workspace_dirs(file.to_string_lossy().into_owned()).unwrap_err();
        assert!(error.contains("no es una carpeta"));
    }

    #[test]
    fn list_context_tree_include_dirs_keeps_root_md_and_selected() {
        let fixture = Fixture::new();
        fixture.write_md("README.md", "# root\n");
        fixture.write_md("docs/guide.md", "# guide\n");
        fixture.write_md(".cursor/rules/agent.md", "# cursor\n");
        fixture.write_md("src/nested/deep.md", "# deep\n");

        let tree = fixture.tree_filtered(&["docs", ".cursor"]);
        assert_eq!(names(&tree), [".cursor", "docs", "README.md"]);
        assert!(tree.iter().all(|node| node.name != "src"));

        let cursor = find(&tree, ".cursor");
        assert_eq!(
            find(&find(&cursor.children, "rules").children, "agent.md").path,
            ".cursor/rules/agent.md"
        );
    }

    #[test]
    fn list_context_tree_empty_include_is_root_markdown_only() {
        let fixture = Fixture::new();
        fixture.write_md("README.md", "# root\n");
        fixture.write_md("docs/guide.md", "# guide\n");

        let tree = fixture.tree_filtered(&[]);
        assert_eq!(names(&tree), ["README.md"]);
    }

    #[test]
    fn list_context_tree_rejects_nested_include_name() {
        let fixture = Fixture::new();
        fixture.write_md("docs/guide.md", "# guide\n");

        let error = list_context_tree(
            fixture.root_str(),
            Some(vec!["docs/nested".into()]),
        )
        .unwrap_err();
        assert_eq!(error, "Ruta inválida");

        let parent = list_context_tree(fixture.root_str(), Some(vec!["..".into()])).unwrap_err();
        assert_eq!(parent, "Ruta inválida");
    }

    #[test]
    fn watch_ignores_tmp_and_skip_dirs() {
        let root = Path::new("/workspace");

        assert!(watch_path_matters(root, Path::new("/workspace")));
        assert!(watch_path_matters(root, Path::new("/workspace/docs/note.md")));
        assert!(watch_path_matters(
            root,
            Path::new("/workspace/.agents/persona.md")
        ));
        assert!(!watch_path_matters(
            root,
            Path::new("/workspace/docs/.note.md.idioteque.tmp")
        ));
        assert!(!watch_path_matters(
            root,
            Path::new("/workspace/node_modules/pkg/README.md")
        ));
        assert!(!watch_path_matters(root, Path::new("/workspace/.git/HEAD")));
        assert!(!watch_path_matters(root, Path::new("/other/docs/note.md")));
    }
}
