use std::path::Path;

use serde::Serialize;

use super::exec::Git;
use super::status::{discover_repo, probe, require_directory, GitProbe};

const GRAPH_LIMIT: &str = "300";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefRepository {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    pub detached: bool,
    pub initial: bool,
    pub branches: Vec<GitRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefsSnapshot {
    pub probe: GitProbe,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<GitRefRepository>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub hash: String,
    pub short: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitComparison {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_base: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphRepository {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    pub detached: bool,
    pub initial: bool,
    pub commits: Vec<GitCommit>,
    pub comparisons: Vec<GitComparison>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphSnapshot {
    pub probe: GitProbe,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<GitGraphRepository>,
}

pub fn refs(root: &str) -> Result<GitRefsSnapshot, String> {
    let root = require_directory(root)?;
    let probe = probe();

    if !probe.available {
        return Ok(GitRefsSnapshot {
            probe,
            repository: None,
        });
    }

    let git = Git::discover()?;
    let Some(_) = discover_repo(&git, &root)? else {
        return Ok(GitRefsSnapshot {
            probe,
            repository: None,
        });
    };

    Ok(GitRefsSnapshot {
        probe,
        repository: Some(load_refs(&git, &root)?),
    })
}

pub fn graph(root: &str, selected: &[String]) -> Result<GitGraphSnapshot, String> {
    let root = require_directory(root)?;
    let probe = probe();

    if !probe.available {
        return Ok(GitGraphSnapshot {
            probe,
            repository: None,
        });
    }

    let git = Git::discover()?;
    let Some(_) = discover_repo(&git, &root)? else {
        return Ok(GitGraphSnapshot {
            probe,
            repository: None,
        });
    };

    let info = load_refs(&git, &root)?;
    if info.initial {
        return Ok(GitGraphSnapshot {
            probe,
            repository: Some(GitGraphRepository {
                current: info.current,
                oid: info.oid,
                detached: info.detached,
                initial: true,
                commits: Vec::new(),
                comparisons: Vec::new(),
            }),
        });
    }

    let allowed: Vec<&str> = info.branches.iter().map(|branch| branch.name.as_str()).collect();
    let extras: Vec<&str> = selected
        .iter()
        .map(String::as_str)
        .filter(|name| is_safe_ref(name) && allowed.contains(name))
        .filter(|name| info.current.as_deref() != Some(*name))
        .collect();

    let mut args = vec![
        "log",
        "--topo-order",
        "--decorate=full",
        "--no-abbrev-commit",
        "--format=%H%x00%h%x00%P%x00%D%x00%s",
        "-z",
        "-n",
        GRAPH_LIMIT,
        "HEAD",
    ];
    args.extend(extras.iter().copied());

    let output = git.require_ok(&root, &args)?;
    let commits = parse_log(&output.stdout)?;

    let mut comparisons = Vec::new();
    for name in extras {
        comparisons.push(compare_to_head(&git, &root, name)?);
    }

    Ok(GitGraphSnapshot {
        probe,
        repository: Some(GitGraphRepository {
            current: info.current,
            oid: info.oid,
            detached: info.detached,
            initial: false,
            commits,
            comparisons,
        }),
    })
}

fn load_refs(git: &Git, root: &Path) -> Result<GitRefRepository, String> {
    let listed = git.require_ok(
        root,
        &[
            "for-each-ref",
            "--format=%(objectname)%00%(refname:short)%00%(HEAD)",
            "refs/heads",
        ],
    )?;

    let mut branches = parse_for_each_ref(&listed.stdout);
    let initial = !head_exists(git, root);
    let oid = if initial {
        None
    } else {
        rev_parse(git, root, "HEAD")
    };

    let current = branches
        .iter()
        .find(|branch| branch.current)
        .map(|branch| branch.name.clone())
        .or_else(|| {
            if initial {
                symbolic_head(git, root)
            } else {
                None
            }
        });

    if initial {
        if let Some(name) = current.as_deref() {
            if !branches.iter().any(|branch| branch.name == name) {
                branches.push(GitRef {
                    name: name.to_string(),
                    hash: None,
                    current: true,
                });
            }
        }
    }

    let detached = !initial && current.is_none();

    Ok(GitRefRepository {
        current,
        oid,
        detached,
        initial,
        branches,
    })
}

fn head_exists(git: &Git, root: &Path) -> bool {
    git.run(root, &["rev-parse", "--verify", "HEAD"])
        .map(|output| output.success)
        .unwrap_or(false)
}

fn rev_parse(git: &Git, root: &Path, rev: &str) -> Option<String> {
    let output = git.run(root, &["rev-parse", rev]).ok()?;
    if !output.success {
        return None;
    }

    let hash = output.stdout_lossy().trim().to_string();
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}

fn symbolic_head(git: &Git, root: &Path) -> Option<String> {
    let output = git.run(root, &["symbolic-ref", "--short", "HEAD"]).ok()?;
    if !output.success {
        return None;
    }

    let name = output.stdout_lossy().trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn compare_to_head(git: &Git, root: &Path, name: &str) -> Result<GitComparison, String> {
    let merge = git.run(root, &["merge-base", "HEAD", name])?;
    let merge_base = if merge.success {
        let hash = merge.stdout_lossy().trim().to_string();
        if hash.is_empty() {
            None
        } else {
            Some(hash)
        }
    } else {
        None
    };

    let range = format!("HEAD...{name}");
    let counts = git.run(root, &["rev-list", "--count", "--left-right", &range])?;
    let (ahead, behind) = if counts.success {
        parse_left_right(&counts.stdout_lossy())
    } else {
        (0, 0)
    };

    Ok(GitComparison {
        name: name.to_string(),
        merge_base,
        ahead,
        behind,
    })
}

fn parse_for_each_ref(stdout: &[u8]) -> Vec<GitRef> {
    let text = String::from_utf8_lossy(stdout);
    let mut branches = Vec::new();

    for line in text.lines() {
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split('\0');
        let hash = parts.next().unwrap_or("").trim();
        let name = parts.next().unwrap_or("").trim();
        let head = parts.next().unwrap_or("").trim();

        if name.is_empty() || !is_safe_ref(name) {
            continue;
        }

        branches.push(GitRef {
            name: name.to_string(),
            hash: if hash.is_empty() {
                None
            } else {
                Some(hash.to_string())
            },
            current: head == "*",
        });
    }

    branches
}

fn parse_log(stdout: &[u8]) -> Result<Vec<GitCommit>, String> {
    let mut commits = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut current = Vec::new();

    for &byte in stdout {
        if byte == 0 {
            fields.push(String::from_utf8_lossy(&current).into_owned());
            current.clear();
            if fields.len() == 5 {
                commits.push(commit_from_fields(&fields));
                fields.clear();
            }
        } else {
            current.push(byte);
        }
    }

    if !current.is_empty() {
        fields.push(String::from_utf8_lossy(&current).into_owned());
    }

    if fields.len() == 5 {
        commits.push(commit_from_fields(&fields));
    } else if !fields.is_empty() {
        return Err("Git log devolvió un registro incompleto".to_string());
    }

    Ok(commits)
}

fn commit_from_fields(fields: &[String]) -> GitCommit {
    let hash = fields[0].trim().to_string();
    let short = fields[1].trim().to_string();
    let parents = fields[2]
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let refs = parse_decorate(&fields[3]);
    let subject = fields[4].trim().to_string();

    GitCommit {
        hash,
        short,
        parents,
        subject,
        refs,
    }
}

fn parse_decorate(raw: &str) -> Vec<String> {
    let mut refs = Vec::new();

    for piece in raw.split(',') {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("HEAD -> ") {
            if let Some(name) = short_ref(rest) {
                refs.push(name);
            }
            continue;
        }

        if trimmed == "HEAD" {
            refs.push("HEAD".to_string());
            continue;
        }

        if let Some(name) = short_ref(trimmed) {
            refs.push(name);
        }
    }

    refs
}

fn short_ref(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("tag: ") {
        return None;
    }

    let name = trimmed
        .strip_prefix("refs/heads/")
        .or_else(|| trimmed.strip_prefix("refs/remotes/"))
        .unwrap_or(trimmed);

    if name.starts_with("refs/") {
        return None;
    }

    if !is_safe_ref(name) {
        return None;
    }

    Some(name.to_string())
}

fn parse_left_right(text: &str) -> (u32, u32) {
    let mut parts = text
        .trim()
        .split(|ch: char| ch == '\t' || ch.is_whitespace())
        .filter(|part| !part.is_empty());
    let ahead = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let behind = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

fn is_safe_ref(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.contains("..")
        && !name.contains('\0')
        && !name.contains('\n')
        && !name.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::exec::{lock_discover, EnvRestore, Git};
    use std::fs;
    use std::path::PathBuf;
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
            let root = std::env::temp_dir().join(format!("idioteque-graph-{nanos}-{seq}"));
            fs::create_dir_all(&root).expect("create fixture root");
            Self {
                root,
                git,
                _lock: lock,
            }
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
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

        fn commit(&self, file: &str, message: &str) {
            self.write(file, &format!("{message}\n"));
            self.git(&["add", file]);
            self.git(&["commit", "-m", message]);
        }

        fn root_str(&self) -> String {
            self.root.to_string_lossy().into_owned()
        }

        fn refs(&self) -> GitRefsSnapshot {
            refs(&self.root_str()).expect("refs")
        }

        fn graph(&self, selected: &[&str]) -> GitGraphSnapshot {
            let selected: Vec<String> = selected.iter().map(|name| (*name).to_string()).collect();
            graph(&self.root_str(), &selected).expect("graph")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn snapshot_json_uses_camel_case() {
        let json = serde_json::to_string(&GitGraphSnapshot {
            probe: GitProbe {
                available: true,
                path: Some("/usr/bin/git".to_string()),
                version: Some("2.43.0".to_string()),
            },
            repository: Some(GitGraphRepository {
                current: Some("main".to_string()),
                oid: Some("abc".to_string()),
                detached: false,
                initial: false,
                commits: vec![GitCommit {
                    hash: "abc".to_string(),
                    short: "abc1234".to_string(),
                    parents: vec![],
                    subject: "First commit".to_string(),
                    refs: vec!["main".to_string()],
                }],
                comparisons: vec![GitComparison {
                    name: "feat".to_string(),
                    merge_base: Some("abc".to_string()),
                    ahead: 1,
                    behind: 2,
                }],
            }),
        })
        .expect("json");

        assert!(json.contains("\"mergeBase\":\"abc\""));
        assert!(json.contains("\"short\":\"abc1234\""));
        assert!(!json.contains("merge_base"));
    }

    #[test]
    fn refs_without_git_binary_has_no_repository() {
        let fixture = Fixture::new();
        let _restore = EnvRestore::set("IDIOTEQUE_GIT", "/no/such-idioteque-git");
        let snap = fixture.refs();
        assert!(!snap.probe.available);
        assert_eq!(snap.repository, None);
    }

    #[test]
    fn refs_without_repo_is_empty() {
        let fixture = Fixture::new();
        let snap = fixture.refs();
        assert!(snap.probe.available);
        assert_eq!(snap.repository, None);
    }

    #[test]
    fn graph_without_git_binary_has_no_repository() {
        let fixture = Fixture::new();
        let _restore = EnvRestore::set("IDIOTEQUE_GIT", "/no/such-idioteque-git");
        let snap = fixture.graph(&[]);
        assert!(!snap.probe.available);
        assert_eq!(snap.repository, None);
    }

    #[test]
    fn graph_without_repo_is_empty() {
        let fixture = Fixture::new();
        let snap = fixture.graph(&[]);
        assert!(snap.probe.available);
        assert_eq!(snap.repository, None);
    }

    #[test]
    fn initial_repo_lists_main_without_commits() {
        let fixture = Fixture::new();
        fixture.init();

        let repo = fixture.refs().repository.expect("repo");
        assert_eq!(repo.current.as_deref(), Some("main"));
        assert!(repo.initial);
        assert!(!repo.detached);
        assert!(repo.oid.is_none());
        assert!(repo.branches.iter().any(|branch| branch.name == "main" && branch.current));

        let graph = fixture.graph(&[]).repository.expect("graph");
        assert!(graph.initial);
        assert!(graph.commits.is_empty());
        assert!(graph.comparisons.is_empty());
    }

    #[test]
    fn linear_history_keeps_subject_and_short_hash() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.commit("a.md", "First commit");

        let repo = fixture.graph(&[]).repository.expect("graph");
        assert!(!repo.detached);
        assert_eq!(repo.current.as_deref(), Some("main"));
        assert_eq!(repo.commits.len(), 1);
        let commit = &repo.commits[0];
        assert_eq!(commit.subject, "First commit");
        assert!(commit.short.len() >= 7);
        assert!(commit.short.len() < commit.hash.len());
        assert!(commit.hash.starts_with(&commit.short));
        assert!(commit.parents.is_empty());
        assert!(commit.refs.iter().any(|name| name == "main"));
    }

    #[test]
    fn forked_branch_reports_merge_base_and_ahead_behind() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.commit("a.md", "root");
        fixture.git(&["checkout", "-b", "feat"]);
        fixture.commit("feat.md", "on feat");
        fixture.git(&["checkout", "main"]);
        fixture.commit("main.md", "on main");

        let refs = fixture.refs().repository.expect("refs");
        assert_eq!(refs.current.as_deref(), Some("main"));
        assert!(!refs.detached);
        assert_eq!(refs.branches.len(), 2);

        let repo = fixture.graph(&["feat"]).repository.expect("graph");
        assert!(!repo.detached);
        assert_eq!(repo.commits.len(), 3);
        assert_eq!(repo.comparisons.len(), 1);
        let comparison = &repo.comparisons[0];
        assert_eq!(comparison.name, "feat");
        assert!(comparison.merge_base.is_some());
        assert_eq!(comparison.ahead, 1);
        assert_eq!(comparison.behind, 1);

        let root = repo
            .commits
            .iter()
            .find(|commit| commit.subject == "root")
            .expect("root");
        assert_eq!(comparison.merge_base.as_deref(), Some(root.hash.as_str()));
    }

    #[test]
    fn merge_commit_has_two_parents() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.commit("a.md", "root");
        fixture.git(&["checkout", "-b", "feat"]);
        fixture.commit("feat.md", "on feat");
        fixture.git(&["checkout", "main"]);
        fixture.commit("main.md", "on main");
        fixture.git(&["merge", "--no-ff", "-m", "Merge feat", "feat"]);

        let repo = fixture.graph(&["feat"]).repository.expect("graph");
        let merge = repo
            .commits
            .iter()
            .find(|commit| commit.subject == "Merge feat")
            .expect("merge");
        assert_eq!(merge.parents.len(), 2);
        assert_eq!(repo.comparisons[0].ahead, 2);
        assert_eq!(repo.comparisons[0].behind, 0);
        let feat_tip = repo
            .commits
            .iter()
            .find(|commit| commit.subject == "on feat")
            .expect("feat tip");
        assert_eq!(
            repo.comparisons[0].merge_base.as_deref(),
            Some(feat_tip.hash.as_str())
        );
    }

    #[test]
    fn detached_head_lists_commits_without_current_branch() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.commit("a.md", "root");
        fixture.git(&["checkout", "--detach", "HEAD"]);

        let refs = fixture.refs().repository.expect("refs");
        assert!(refs.detached);
        assert_eq!(refs.current, None);
        assert!(refs.oid.is_some());
        assert!(refs.branches.iter().any(|branch| branch.name == "main" && !branch.current));

        let repo = fixture.graph(&[]).repository.expect("graph");
        assert!(repo.detached);
        assert_eq!(repo.current, None);
        assert_eq!(repo.commits.len(), 1);
        assert_eq!(repo.commits[0].subject, "root");
    }

    #[test]
    fn unknown_and_unsafe_refs_are_ignored() {
        let fixture = Fixture::new();
        fixture.init();
        fixture.commit("a.md", "root");

        let repo = fixture
            .graph(&["-evil", "no-such", "main", "feat"])
            .repository
            .expect("graph");
        assert!(repo.comparisons.is_empty());
        assert_eq!(repo.commits.len(), 1);
    }

    #[test]
    fn parse_left_right_reads_tab_counts() {
        assert_eq!(parse_left_right("2\t3\n"), (2, 3));
        assert_eq!(parse_left_right("0 0"), (0, 0));
        assert_eq!(parse_left_right(""), (0, 0));
    }

    #[test]
    fn parse_decorate_keeps_local_heads() {
        assert_eq!(
            parse_decorate("HEAD -> refs/heads/main, refs/heads/feat, tag: v1"),
            vec!["main".to_string(), "feat".to_string()]
        );
        assert_eq!(parse_decorate("HEAD"), vec!["HEAD".to_string()]);
        assert_eq!(
            parse_decorate("HEAD -> main, refs/remotes/origin/main"),
            vec!["main".to_string(), "origin/main".to_string()]
        );
    }

    #[test]
    fn parse_log_rejects_an_incomplete_record() {
        assert!(parse_log(b"").unwrap().is_empty());
        assert!(parse_log(b"onlyhash\0short\0").is_err());
    }

    #[test]
    fn is_safe_ref_rejects_option_like_names() {
        assert!(is_safe_ref("main"));
        assert!(is_safe_ref("feat/x"));
        assert!(!is_safe_ref("-n"));
        assert!(!is_safe_ref("a..b"));
        assert!(!is_safe_ref(""));
        assert!(!is_safe_ref("a\nb"));
        assert!(!is_safe_ref("a\0b"));
        assert!(!is_safe_ref("a\\b"));
    }
}
