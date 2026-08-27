use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// Oldest Git we rely on: `--no-optional-locks` landed in 2.15,
/// porcelain v2 in 2.11. We take the stricter floor.
pub const MIN_GIT_VERSION: &str = "2.15.0";

const VERSION_PREFIX: &str = "git version ";

/// The resolved `git` binary. Finding it is separate from talking to a repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Git {
    pub path: PathBuf,
    pub version: String,
}

#[derive(Debug)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub success: bool,
    pub code: Option<i32>,
}

impl Git {
    /// Looks up Git on `PATH`, or `IDIOTEQUE_GIT` when tests need a pin.
    pub fn discover() -> Result<Self, String> {
        let path = find_git()?;
        let version = read_version(&path)?;

        if !version_at_least(&version, MIN_GIT_VERSION) {
            return Err(format!(
                "Git {version} es demasiado antiguo (hace falta {MIN_GIT_VERSION}+)"
            ));
        }

        Ok(Self { path, version })
    }

    /// Runs `git <args>` in `cwd`. Never a shell. Args stay argv.
    ///
    /// Spawn policy follows Zed's `GitBinary`: no pager, no optional index
    /// locks, no fsmonitor hook, no hanging prompts. Env from the app is
    /// inherited (ssh-agent, credential helpers) except the overlays below.
    pub fn run(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, String> {
        let safe_directory = cwd.to_string_lossy().into_owned();

        let output = Command::new(&self.path)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PAGER", "cat")
            .env("LC_ALL", "en_US.UTF-8")
            .env("LANG", "en_US.UTF-8")
            .env("LANGUAGE", "en")
            .arg("--no-optional-locks")
            .arg("--no-pager")
            .args(["-c", "core.fsmonitor=false"])
            .args(["-c", "log.showSignature=false"])
            .args(["-c", &format!("safe.directory={safe_directory}")])
            .args(args)
            .output()
            .map_err(|error| format!("No se pudo ejecutar Git: {error}"))?;

        Ok(GitOutput::from(output))
    }

    /// `rev-parse` / `status` helpers share the same failure text.
    pub fn require_ok(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, String> {
        let output = self.run(cwd, args)?;

        if output.success {
            return Ok(output);
        }

        Err(git_failed(&output, args))
    }
}

impl GitOutput {
    fn from(output: Output) -> Self {
        Self {
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            success: output.status.success(),
            code: output.status.code(),
        }
    }

    pub fn stdout_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

fn git_executable_name() -> &'static str {
    if cfg!(windows) {
        "git.exe"
    } else {
        "git"
    }
}

fn find_git() -> Result<PathBuf, String> {
    if let Ok(explicit) = env::var("IDIOTEQUE_GIT") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok(path);
        }

        return Err("IDIOTEQUE_GIT no apunta a un ejecutable".to_string());
    }

    let name = git_executable_name();
    let path_var = env::var_os("PATH").ok_or_else(|| "No se encontró Git en el PATH".to_string())?;

    for directory in env::split_paths(&path_var) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("No se encontró Git en el PATH".to_string())
}

fn read_version(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("No se pudo ejecutar Git: {error}"))?;

    if !output.status.success() {
        return Err("Git no respondió a --version".to_string());
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    parse_version(&raw).ok_or_else(|| "No se pudo leer la versión de Git".to_string())
}

pub(crate) fn parse_version(raw: &str) -> Option<String> {
    let line = raw.lines().next()?.trim();
    let version = line.strip_prefix(VERSION_PREFIX)?.trim();

    if version.is_empty() {
        return None;
    }

    Some(version.to_string())
}

/// Compares dotted numeric prefixes (`2.43.0.windows.1` vs `2.15.0`).
pub(crate) fn version_at_least(version: &str, minimum: &str) -> bool {
    compare_versions(version, minimum) != std::cmp::Ordering::Less
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left = version_numbers(left);
    let right = version_numbers(right);
    let width = left.len().max(right.len());

    for index in 0..width {
        let l = left.get(index).copied().unwrap_or(0);
        let r = right.get(index).copied().unwrap_or(0);
        match l.cmp(&r) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }

    std::cmp::Ordering::Equal
}

fn version_numbers(version: &str) -> Vec<u64> {
    version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter_map(|part| {
            if part.is_empty() {
                None
            } else {
                part.parse().ok()
            }
        })
        .collect()
}

fn git_failed(output: &GitOutput, args: &[&str]) -> String {
    let command = args.first().copied().unwrap_or("git");
    let detail = if output.stderr.is_empty() {
        format!("código {}", output.code.unwrap_or(-1))
    } else {
        output.stderr.clone()
    };

    format!("Git falló ({command}): {detail}")
}

pub(crate) fn is_not_a_repository(stderr: &str) -> bool {
    stderr.contains("not a git repository")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_strips_prefix() {
        assert_eq!(
            parse_version("git version 2.43.0\n"),
            Some("2.43.0".to_string())
        );
        assert_eq!(
            parse_version("git version 2.43.0.windows.1"),
            Some("2.43.0.windows.1".to_string())
        );
        assert_eq!(parse_version("not git"), None);
    }

    #[test]
    fn version_floor_matches_zed_optional_locks() {
        assert!(version_at_least("2.15.0", MIN_GIT_VERSION));
        assert!(version_at_least("2.43.0", MIN_GIT_VERSION));
        assert!(version_at_least("2.15.0.windows.1", MIN_GIT_VERSION));
        assert!(!version_at_least("2.14.3", MIN_GIT_VERSION));
        assert!(!version_at_least("1.9.1", MIN_GIT_VERSION));
    }

    #[test]
    fn discover_finds_system_git() {
        let git = Git::discover().expect("git on PATH");
        assert!(git.path.is_file());
        assert!(version_at_least(&git.version, MIN_GIT_VERSION));
    }
}
