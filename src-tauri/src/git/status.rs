use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::exec::{is_not_a_repository, Git};
use super::porcelain::{parse_status_v2, GitFile, ParsedStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProbe {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepository {
    pub toplevel: String,
    pub git_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    pub detached: bool,
    pub initial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    pub files: Vec<GitFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSnapshot {
    pub probe: GitProbe,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<GitRepository>,
}

pub fn probe() -> GitProbe {
    match Git::discover() {
        Ok(git) => GitProbe {
            available: true,
            path: Some(git.path.to_string_lossy().into_owned()),
            version: Some(git.version),
        },
        Err(_) => GitProbe {
            available: false,
            path: None,
            version: None,
        },
    }
}

pub fn snapshot(root: &str) -> Result<GitSnapshot, String> {
    let root = require_directory(root)?;
    let probe = probe();

    if !probe.available {
        return Ok(GitSnapshot {
            probe,
            repository: None,
        });
    }

    let git = Git::discover()?;
    let Some((toplevel, git_dir)) = discover_repo(&git, &root)? else {
        return Ok(GitSnapshot {
            probe,
            repository: None,
        });
    };

    let output = git.require_ok(
        &root,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
            "-z",
        ],
    )?;

    let parsed = parse_status_v2(&output.stdout)?;

    Ok(GitSnapshot {
        probe,
        repository: Some(GitRepository::from_parsed(toplevel, git_dir, parsed)),
    })
}

impl GitRepository {
    fn from_parsed(toplevel: String, git_dir: String, parsed: ParsedStatus) -> Self {
        Self {
            toplevel,
            git_dir,
            branch: parsed.branch,
            oid: parsed.oid,
            detached: parsed.detached,
            initial: parsed.initial,
            upstream: parsed.upstream,
            ahead: parsed.ahead,
            behind: parsed.behind,
            dirty: !parsed.files.is_empty(),
            files: parsed.files,
        }
    }
}

fn discover_repo(git: &Git, root: &Path) -> Result<Option<(String, String)>, String> {
    let output = git.run(root, &["rev-parse", "--show-toplevel", "--absolute-git-dir"])?;

    if !output.success {
        if is_not_a_repository(&output.stderr) {
            return Ok(None);
        }

        return Err(format!("Git falló (rev-parse): {}", output.stderr));
    }

    let text = output.stdout_lossy();
    let mut lines = text.lines().filter(|line| !line.is_empty());
    let toplevel = lines
        .next()
        .ok_or_else(|| "Git no devolvió el toplevel".to_string())?;
    let git_dir = lines
        .next()
        .ok_or_else(|| "Git no devolvió el git-dir".to_string())?;

    Ok(Some((toplevel.to_string(), git_dir.to_string())))
}

fn require_directory(root: &str) -> Result<PathBuf, String> {
    let path = Path::new(root);

    if !path.is_dir() {
        return Err(format!("`{root}` no es una carpeta"));
    }

    fs::canonicalize(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::exec::{lock_discover, EnvRestore, Git};
    use crate::git::porcelain::GitFileKind;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::MutexGuard;
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQ: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        git: Git,
        _lock: MutexGuard<'static, ()>,
    }

    impl Fixture {
        fn new() -> Self {
            let lock = lock_discover();
            let git = Git::discover().expect("git on PATH");
            let seq = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("idioteque-git-{nanos}-{seq}"));
            fs::create_dir_all(&root).expect("create fixture root");
            Self {
                root,
                git,
                _lock: lock,
            }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(path, contents).expect("write");
        }

        fn git(&self, args: &[&str]) {
            let output = Command::new(&self.git.path)
                .current_dir(&self.root)
                .env("GIT_AUTHOR_NAME", "idioteque")
                .env("GIT_AUTHOR_EMAIL", "test@idioteque.local")
                .env("GIT_COMMITTER_NAME", "idioteque")
                .env("GIT_COMMITTER_EMAIL", "test@idioteque.local")
                .args(["-c", "safe.directory=*"])
                .args(args)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn init(&self) {
            self.git(&["init", "-b", "main"]);
            self.git(&["config", "user.name", "idioteque"]);
            self.git(&["config", "user.email", "test@idioteque.local"]);
        }

        fn root_str(&self) -> String {
            self.root.to_string_lossy().into_owned()
        }

        fn snap(&self) -> GitSnapshot {
            snapshot(&self.root_str()).expect("snapshot")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn snapshot_json_uses_camel_case() {
        let json = serde_json::to_string(&GitSnapshot {
            probe: GitProbe {
                available: true,
                path: Some("/usr/bin/git".to_string()),
                version: Some("2.43.0".to_string()),
            },
            repository: None,
        })
        .expect("json");

        assert!(json.contains("\"available\":true"));
        assert!(json.contains("\"path\":\"/usr/bin/git\""));
        assert!(!json.contains("repository"));
    }

    #[test]
    fn probe_reports_the_system_binary() {
        let _lock = lock_discover();
        let probe = probe();
        assert!(probe.available);
        assert!(probe.path.is_some());
        assert!(probe.version.is_some());
    }

    #[test]
    fn missing_explicit_git_makes_probe_unavailable() {
        let _lock = lock_discover();
        let _restore = EnvRestore::set("IDIOTEQUE_GIT", "/no/such/idioteque-git");
        let probe = probe();
        assert!(!probe.available);
        assert_eq!(probe.path, None);
        assert_eq!(probe.version, None);
    }

    #[test]
    fn snapshot_without_git_binary_has_no_repository() {
        let fixture = Fixture::new();
        let _restore = EnvRestore::set("IDIOTEQUE_GIT", "/no/such/idioteque-git");
        let snap = fixture.snap();
        assert!(!snap.probe.available);
        assert_eq!(snap.repository, None);
    }

    #[test]
    fn snapshot_without_repo_is_empty() {
        let fixture = Fixture::new();
        let snap = fixture.snap();
        assert!(snap.probe.available);
        assert_eq!(snap.repository, None);
    }

    #[test]
    fn snapshot_rejects_a_file() {
        let fixture = Fixture::new();
        fixture.write("note.md", "x\n");
        let error = snapshot(fixture.path("note.md").to_str().expect("utf8")).unwrap_err();
        assert!(error.contains("no es una carpeta"));
    }

    #[test]
    fn empty_repo_is_initial_on_main() {
        let fixture = Fixture::new();
        fixture.init();

        let repo = fixture.snap().repository.expect("repo");
        assert_eq!(repo.branch.as_deref(), Some("main"));
        assert!(repo.initial);
        assert!(!repo.dirty);
        assert!(repo.files.is_empty());
        assert_eq!(
            Path::new(&repo.toplevel),
            fs::canonicalize(&fixture.root).expect("root").as_path()
        );
        assert_eq!(
            Path::new(&repo.git_dir),
            fs::canonicalize(fixture.path(".git")).expect("git dir").as_path()
        );
    }

    #[test]
    fn dirty_and_untracked_files_show_up() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.write("a.md", "hello\n");
        fixture.git(&["add", "a.md"]);
        fixture.git(&["commit", "-m", "init"]);
        fixture.write("a.md", "hello\ndirty\n");
        fixture.write("b.md", "new\n");

        let repo = fixture.snap().repository.expect("repo");
        assert!(repo.dirty);
        assert_eq!(repo.files.len(), 2);

        let dirty = repo
            .files
            .iter()
            .find(|file| file.path == "a.md")
            .expect("a.md");
        assert_eq!(dirty.kind, GitFileKind::Ordinary);
        assert_eq!(dirty.staged, ".");
        assert_eq!(dirty.unstaged, "M");

        let untracked = repo
            .files
            .iter()
            .find(|file| file.path == "b.md")
            .expect("b.md");
        assert_eq!(untracked.kind, GitFileKind::Untracked);
    }

    #[test]
    fn rename_keeps_both_paths() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.write("a.md", "hello\n");
        fixture.git(&["add", "a.md"]);
        fixture.git(&["commit", "-m", "init"]);
        fixture.git(&["mv", "a.md", "renamed.md"]);
        fixture.write("renamed.md", "hello\nextra\n");

        let repo = fixture.snap().repository.expect("repo");
        let renamed = repo
            .files
            .iter()
            .find(|file| file.kind == GitFileKind::Renamed)
            .expect("rename");
        assert_eq!(renamed.path, "renamed.md");
        assert_eq!(renamed.original_path.as_deref(), Some("a.md"));
    }

    #[test]
    fn nested_folder_discovers_parent_repo() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.write("docs/guide.md", "# g\n");
        fixture.git(&["add", "docs/guide.md"]);
        fixture.git(&["commit", "-m", "docs"]);

        let nested = snapshot(fixture.path("docs").to_str().expect("utf8")).expect("nested");
        let repo = nested.repository.expect("repo");
        assert_eq!(
            Path::new(&repo.toplevel),
            fs::canonicalize(&fixture.root).expect("root").as_path()
        );
        assert_eq!(repo.branch.as_deref(), Some("main"));
        assert!(!repo.dirty);
    }

    #[test]
    fn snapshot_rejects_empty_and_missing_paths() {
        let error = snapshot("").unwrap_err();
        assert!(error.contains("no es una carpeta"));

        let missing = std::env::temp_dir().join("idioteque-git-missing-root-does-not-exist");
        let error = snapshot(missing.to_str().expect("utf8")).unwrap_err();
        assert!(error.contains("no es una carpeta"));
    }

    #[test]
    fn detached_head_has_no_branch() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.write("a.md", "hello\n");
        fixture.git(&["add", "a.md"]);
        fixture.git(&["commit", "-m", "init"]);
        fixture.git(&["checkout", "--detach", "HEAD"]);

        let repo = fixture.snap().repository.expect("repo");
        assert!(repo.detached);
        assert_eq!(repo.branch, None);
        assert!(repo.oid.is_some());
        assert!(!repo.initial);
    }

    #[test]
    fn worktree_with_git_file_is_still_a_repo() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.write("a.md", "hello\n");
        fixture.git(&["add", "a.md"]);
        fixture.git(&["commit", "-m", "init"]);
        fixture.git(&["worktree", "add", "linked"]);

        let git_file = fixture.path("linked/.git");
        assert!(git_file.is_file(), "worktree uses a .git file");

        let linked = snapshot(fixture.path("linked").to_str().expect("utf8")).expect("worktree");
        let repo = linked.repository.expect("repo");
        assert_eq!(
            Path::new(&repo.toplevel),
            fs::canonicalize(fixture.path("linked"))
                .expect("linked")
                .as_path()
        );
        assert_eq!(repo.branch.as_deref(), Some("linked"));
        assert!(!repo.dirty);
    }
}
