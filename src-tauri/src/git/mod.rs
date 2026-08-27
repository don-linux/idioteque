//! How idioteque talks to Git: a thin CLI wrapper we own.
//!
//! This is not a port of VS Code's `extensions/git` and not Zed's `crates/git`.
//! Both are the reference for the *contract*:
//!
//! - Zed's `GitBinary` for how to spawn (`--no-optional-locks`, `--no-pager`,
//!   no fsmonitor hook, no prompts, inherit the user's env).
//! - VS Code for which questions to ask (`rev-parse`, porcelain status) and
//!   for treating the UI as a model, not a command echo.
//!
//! The output format is porcelain v2 (`status --porcelain=v2 -z -b`), which
//! VS Code still does not use (they parse v1). Same idea, cleaner records.
//!
//! No libgit2 / gitoxide. Writes (stage, commit, push) will reuse `Git::run`.

mod exec;
mod porcelain;
mod status;

pub use status::{GitProbe, GitSnapshot};

#[tauri::command]
pub fn git_probe() -> GitProbe {
    status::probe()
}

#[tauri::command]
pub fn git_status(root: String) -> Result<GitSnapshot, String> {
    status::snapshot(&root)
}
