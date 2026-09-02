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

/// What a caller wants to create, so file specific rules stay out of the resolver.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NewKind {
    File,
    Dir,
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

/// Deepest ancestor that already exists on disk, so a nested `a/b/c` can be created.
fn existing_ancestor(path: &Path) -> Option<&Path> {
    let mut current = path.parent();

    while let Some(candidate) = current {
        if candidate.as_os_str().is_empty() {
            return None;
        }

        if fs::symlink_metadata(candidate).is_ok() {
            return Some(candidate);
        }

        current = candidate.parent();
    }

    None
}

/// Where a new file or directory may land. The target must not exist yet, and the
/// deepest existing ancestor is canonicalized so a symlinked parent cannot escape.
fn resolve_new_target(root: &str, relative: &str, kind: NewKind) -> Result<PathBuf, String> {
    let target = join_relative(root, relative)?;

    if kind == NewKind::File && !is_markdown(&target) {
        return Err("Solo se pueden crear archivos .md".to_string());
    }

    // `symlink_metadata` also catches broken symlinks, which `exists` reports as free.
    if fs::symlink_metadata(&target).is_ok() {
        return Err(format!("`{relative}` ya existe"));
    }

    let canonical_root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let anchor = existing_ancestor(&target).ok_or_else(|| "Ruta inválida".to_string())?;
    let canonical_anchor = fs::canonicalize(anchor).map_err(|error| error.to_string())?;

    if !canonical_anchor.starts_with(&canonical_root) {
        return Err("La ruta sale de la carpeta abierta".to_string());
    }

    let rest = target
        .strip_prefix(anchor)
        .map_err(|_| "Ruta inválida".to_string())?;

    Ok(canonical_anchor.join(rest))
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

            // Every directory outside the blacklist shows, markdown inside or not:
            // otherwise a folder the user just created would be invisible.
            let children = collect_children(&entry.path(), &path);

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

#[tauri::command]
pub fn create_markdown(root: String, path: String) -> Result<(), String> {
    let target = resolve_new_target(&root, &path, NewKind::File)?;

    // `create_new` fails instead of following a symlink that appeared mid-flight.
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map(|_| ())
        .map_err(|error| format!("No se pudo crear `{path}`: {error}"))
}

#[tauri::command]
pub fn create_directory(root: String, path: String) -> Result<(), String> {
    let target = resolve_new_target(&root, &path, NewKind::Dir)?;

    fs::create_dir_all(&target).map_err(|error| format!("No se pudo crear `{path}`: {error}"))
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
    fn tree_shows_directories_without_markdown() {
        let fixture = Fixture::new();
        fixture.write_md("README.md", "# root\n");
        fixture.mkdir("vacia");
        fixture.mkdir("assets");
        fs::write(fixture.path("assets/logo.txt"), "nope").expect("write txt");
        fs::write(fixture.path("assets/nota.rs"), "nope").expect("write rs");

        let tree = fixture.tree();
        assert_eq!(names(&tree), ["assets", "vacia", "README.md"]);

        // The directory shows, but its non markdown files stay out of the tree.
        assert!(find(&tree, "assets").children.is_empty());
        assert!(find(&tree, "vacia").children.is_empty());
    }

    #[test]
    fn tree_shows_nested_directories_without_markdown() {
        let fixture = Fixture::new();
        fixture.mkdir("src/lib/components");

        let tree = fixture.tree();
        assert_eq!(names(&tree), ["src"]);

        let lib = find(&find(&tree, "src").children, "lib");
        assert_eq!(lib.path, "src/lib");
        assert_eq!(names(&lib.children), ["components"]);
        assert_eq!(find(&lib.children, "components").path, "src/lib/components");
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
    fn create_markdown_writes_an_empty_file_the_tree_can_see() {
        let fixture = Fixture::new();
        fixture.mkdir("docs");

        let root = fixture.root_str();
        create_markdown(root.clone(), "docs/nueva.md".to_string()).expect("create");

        assert_eq!(
            read_markdown(root.clone(), "docs/nueva.md".to_string()).expect("read"),
            ""
        );
        assert_eq!(
            names(&find(&fixture.tree(), "docs").children),
            ["nueva.md"]
        );
    }

    #[test]
    fn create_markdown_rejects_duplicates_non_md_and_escapes() {
        let fixture = Fixture::new();
        fixture.write_md("docs/note.md", "hola\n");

        let root = fixture.root_str();

        let duplicate = create_markdown(root.clone(), "docs/note.md".to_string()).unwrap_err();
        assert_eq!(duplicate, "`docs/note.md` ya existe");
        assert_eq!(
            read_markdown(root.clone(), "docs/note.md".to_string()).expect("intact"),
            "hola\n"
        );

        let extension = create_markdown(root.clone(), "docs/note.txt".to_string()).unwrap_err();
        assert_eq!(extension, "Solo se pueden crear archivos .md");

        let escape = create_markdown(root.clone(), "../fuera.md".to_string()).unwrap_err();
        assert_eq!(escape, "Ruta inválida");

        let nested_escape = create_markdown(root, "docs/../../fuera.md".to_string()).unwrap_err();
        assert_eq!(nested_escape, "Ruta inválida");
    }

    #[test]
    fn create_markdown_rejects_a_directory_that_is_in_the_way() {
        let fixture = Fixture::new();
        fixture.mkdir("docs/nota.md");

        let error = create_markdown(fixture.root_str(), "docs/nota.md".to_string()).unwrap_err();
        assert_eq!(error, "`docs/nota.md` ya existe");
    }

    #[test]
    fn create_directory_creates_nested_paths_and_rejects_escapes() {
        let fixture = Fixture::new();
        fixture.write_md("README.md", "# root\n");

        let root = fixture.root_str();
        create_directory(root.clone(), "notas".to_string()).expect("create dir");
        create_directory(root.clone(), "notas/2026/enero".to_string()).expect("create nested");

        assert!(fixture.path("notas/2026/enero").is_dir());

        let tree = fixture.tree();
        let notas = find(&tree, "notas");
        assert_eq!(names(&notas.children), ["2026"]);
        assert_eq!(
            names(&find(&notas.children, "2026").children),
            ["enero"]
        );

        let duplicate = create_directory(root.clone(), "notas".to_string()).unwrap_err();
        assert_eq!(duplicate, "`notas` ya existe");

        let escape = create_directory(root, "../fuera".to_string()).unwrap_err();
        assert_eq!(escape, "Ruta inválida");
    }

    #[cfg(unix)]
    #[test]
    fn create_markdown_refuses_to_follow_a_broken_symlink() {
        let fixture = Fixture::new();
        let outside = std::env::temp_dir().join(format!(
            "idioteque-outside-{}.md",
            FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&outside);
        std::os::unix::fs::symlink(&outside, fixture.path("trampa.md")).expect("symlink");

        let error = create_markdown(fixture.root_str(), "trampa.md".to_string()).unwrap_err();
        assert_eq!(error, "`trampa.md` ya existe");
        assert!(!outside.exists());
    }

    #[cfg(unix)]
    #[test]
    fn create_markdown_rejects_a_symlinked_parent_outside_the_root() {
        let fixture = Fixture::new();
        let outside = std::env::temp_dir().join(format!(
            "idioteque-outside-dir-{}",
            FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&outside).expect("outside dir");
        std::os::unix::fs::symlink(&outside, fixture.path("fuga")).expect("symlink");

        let error = create_markdown(fixture.root_str(), "fuga/nota.md".to_string()).unwrap_err();
        assert_eq!(error, "La ruta sale de la carpeta abierta");
        assert!(!outside.join("nota.md").exists());

        let _ = fs::remove_dir_all(&outside);
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
