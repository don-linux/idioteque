//! Política del sandbox de Chromium en el ADE (contrato 4.1 / 5).
//!
//! El helper SUID y los user namespaces se deciden *antes* de spawnear.
//! El crash+retry queda como red si el probe se equivoca; esos eventos
//! no llegan al frontend.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::ipc::HostEvent;

/// Códigos con los que el ADE reintenta una vez sin sandbox (contrato 5).
/// `1` es un abort (SIGABRT: `ExitStatus::code() == None` → 1).
pub const SANDBOX_RETRY_CODES: &[i32] = &[1, 11, 15];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardAction {
    /// Mandar el evento al Channel ahora.
    Send,
    /// Guardar el `fatal` por si el retry no aplica.
    HoldFatal,
    /// Tirar el evento (aún se puede reintentar; no es `fatal` ni `exit`).
    Drop,
    /// `Exit` 1/11/15 antes de `ready`: relanzar con `--idq-no-sandbox`.
    RetrySandbox,
    /// `Exit` que no se reintenta: mandar el `fatal` guardado y luego el `exit`.
    FlushFatalThenSend,
}

pub fn env_no_sandbox() -> bool {
    matches!(std::env::var("IDIOTEQUE_CEF_NO_SANDBOX"), Ok(value) if value == "1")
}

/// Chromium hace FATAL si el env apunta a un `chrome-sandbox` que no es 4755 root.
pub fn helper_usable_from(uid: u32, mode: u32, is_file: bool) -> bool {
    is_file && uid == 0 && (mode & 0o4000) != 0 && (mode & 0o111) != 0
}

pub fn helper_usable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        helper_usable_from(meta.uid(), meta.mode(), meta.is_file())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

pub fn devel_sandbox_env(cef_dir: &Path) -> Option<PathBuf> {
    let path = cef_dir.join("chrome-sandbox");
    if helper_usable(&path) {
        Some(path)
    } else {
        None
    }
}

pub fn apply_devel_sandbox_env(cmd: &mut Command, cef_dir: &Path) {
    match devel_sandbox_env(cef_dir) {
        Some(path) => {
            cmd.env("CHROME_DEVEL_SANDBOX", path);
        }
        None => {
            cmd.env_remove("CHROME_DEVEL_SANDBOX");
        }
    }
}

pub fn sandbox_available_from(helper_usable: bool, userns: bool) -> bool {
    helper_usable || userns
}

pub fn wants_no_sandbox_from(env_forced: bool, helper_usable: bool, userns: bool) -> bool {
    env_forced || !sandbox_available_from(helper_usable, userns)
}

/// `true` si este slot no puede sandboxed: env, helper inútil y sin userns.
pub fn wants_no_sandbox(cef_dir: &Path) -> bool {
    wants_no_sandbox_from(
        env_no_sandbox(),
        helper_usable(&cef_dir.join("chrome-sandbox")),
        user_namespaces_available(),
    )
}

/// Chromium necesita `CAP_SYS_ADMIN` *dentro* del user ns (zygote / `CLONE_NEWPID`).
/// Crear el ns no basta: AppArmor de Ubuntu 24.04+ deja crear y quita la cap.
pub fn userns_usable_from(newuser_ok: bool, newpid_ok: bool) -> bool {
    newuser_ok && newpid_ok
}

/// `unconfined` + `apparmor_restrict_unprivileged_userns=1` transiciona al
/// perfil `unprivileged_userns`, que niega `CAP_SYS_ADMIN`. cef-host hereda
/// el mismo confinamiento que el ADE; no hace falta forkar.
pub fn apparmor_blocks_userns_from(restrict: bool, profile: &str) -> bool {
    restrict && profile.trim() == "unconfined"
}

pub fn user_namespaces_available() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(probe_user_namespaces)
}

fn probe_user_namespaces() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
    #[cfg(target_os = "linux")]
    {
        if apparmor_blocks_userns_from(
            apparmor_restricts_unprivileged_userns(),
            &apparmor_current_profile(),
        ) {
            return false;
        }
        probe_unshare_via_clone()
    }
}

#[cfg(target_os = "linux")]
fn apparmor_restricts_unprivileged_userns() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn apparmor_current_profile() -> String {
    std::fs::read_to_string("/proc/self/attr/apparmor/current")
        .or_else(|_| std::fs::read_to_string("/proc/self/attr/current"))
        .unwrap_or_default()
}

/// `unshare -U` sale 0 bajo AppArmor restrictivo; Chromium no. El segundo
/// `unshare(CLONE_NEWPID)` exige `CAP_SYS_ADMIN` en el ns recién creado.
#[cfg(target_os = "linux")]
fn probe_unshare_via_clone() -> bool {
    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            return false;
        }
        if pid == 0 {
            let newuser_ok = libc::unshare(libc::CLONE_NEWUSER) == 0;
            let newpid_ok = newuser_ok && libc::unshare(libc::CLONE_NEWPID) == 0;
            libc::_exit(if userns_usable_from(newuser_ok, newpid_ok) {
                0
            } else {
                1
            });
        }
        let mut status = 0;
        if libc::waitpid(pid, &mut status, 0) < 0 {
            return false;
        }
        libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0
    }
}

pub fn can_still_retry_sandbox(saw_ready: bool, retried: bool, launched_no_sandbox: bool) -> bool {
    !saw_ready && !retried && !launched_no_sandbox
}

pub fn is_sandbox_retry_exit(code: i32) -> bool {
    SANDBOX_RETRY_CODES.contains(&code)
}

pub fn should_retry_without_sandbox(
    saw_ready: bool,
    retried: bool,
    launched_no_sandbox: bool,
    code: i32,
) -> bool {
    can_still_retry_sandbox(saw_ready, retried, launched_no_sandbox) && is_sandbox_retry_exit(code)
}

pub fn forward_action(
    event: &HostEvent,
    saw_ready: bool,
    retried: bool,
    launched_no_sandbox: bool,
) -> ForwardAction {
    let hold = can_still_retry_sandbox(saw_ready, retried, launched_no_sandbox);
    match event {
        HostEvent::Ready { .. } => ForwardAction::Send,
        HostEvent::Exit { code }
            if should_retry_without_sandbox(saw_ready, retried, launched_no_sandbox, *code) =>
        {
            ForwardAction::RetrySandbox
        }
        HostEvent::Exit { .. } if hold => ForwardAction::FlushFatalThenSend,
        HostEvent::Fatal { .. } if hold => ForwardAction::HoldFatal,
        _ if hold => ForwardAction::Drop,
        _ => ForwardAction::Send,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fatal(code: i32) -> HostEvent {
        HostEvent::Fatal {
            message: "sandbox".into(),
            code,
        }
    }

    fn ready() -> HostEvent {
        HostEvent::Ready {
            cef: "x".into(),
            chromium: "y".into(),
            api_version: 15200,
            xid: 1,
        }
    }

    #[test]
    fn helper_requires_setuid_root() {
        assert!(!helper_usable_from(1000, 0o755, true));
        assert!(!helper_usable_from(0, 0o755, true));
        assert!(!helper_usable_from(0, 0o4755, false));
        assert!(!helper_usable_from(0, 0o4000, true));
        assert!(helper_usable_from(0, 0o4755, true));
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_a_plain_755_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("chrome-sandbox");
        std::fs::write(&path, b"x").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        assert!(!helper_usable(&path));
        assert!(devel_sandbox_env(tmp.path()).is_none());
    }

    #[test]
    fn probe_policy_helper_or_userns() {
        assert!(!wants_no_sandbox_from(false, true, false));
        assert!(!wants_no_sandbox_from(false, false, true));
        assert!(wants_no_sandbox_from(false, false, false));
        assert!(wants_no_sandbox_from(true, true, true));
        assert!(sandbox_available_from(true, false));
        assert!(sandbox_available_from(false, true));
        assert!(!sandbox_available_from(false, false));
    }

    #[test]
    fn userns_needs_newuser_and_newpid() {
        assert!(userns_usable_from(true, true));
        assert!(!userns_usable_from(true, false));
        assert!(!userns_usable_from(false, true));
        assert!(!userns_usable_from(false, false));
    }

    #[test]
    fn apparmor_restrict_blocks_unconfined_only() {
        assert!(apparmor_blocks_userns_from(true, "unconfined"));
        assert!(apparmor_blocks_userns_from(true, "unconfined\n"));
        assert!(!apparmor_blocks_userns_from(true, "cef-host"));
        assert!(!apparmor_blocks_userns_from(false, "unconfined"));
        assert!(!apparmor_blocks_userns_from(false, ""));
    }

    #[test]
    fn retry_sandbox_only_on_1_11_15() {
        assert!(should_retry_without_sandbox(false, false, false, 15));
        assert!(should_retry_without_sandbox(false, false, false, 1));
        assert!(should_retry_without_sandbox(false, false, false, 11));
        assert!(!should_retry_without_sandbox(false, false, false, 0));
        assert!(!should_retry_without_sandbox(false, false, false, 10));
        assert!(!should_retry_without_sandbox(false, false, false, 13));
        assert!(!should_retry_without_sandbox(false, false, false, 14));
        assert!(!should_retry_without_sandbox(false, false, false, 16));
        assert!(!should_retry_without_sandbox(true, false, false, 15));
        assert!(!should_retry_without_sandbox(false, true, false, 15));
        assert!(!should_retry_without_sandbox(false, false, true, 15));
    }

    #[test]
    fn hold_fatal_then_retry_on_exit_15() {
        assert_eq!(
            forward_action(&fatal(15), false, false, false),
            ForwardAction::HoldFatal
        );
        assert_eq!(
            forward_action(&HostEvent::Exit { code: 15 }, false, false, false),
            ForwardAction::RetrySandbox
        );
        assert_eq!(
            forward_action(&ready(), false, true, false),
            ForwardAction::Send
        );
    }

    #[test]
    fn non_sandbox_exit_flushes_held_fatal() {
        assert_eq!(
            forward_action(&fatal(16), false, false, false),
            ForwardAction::HoldFatal
        );
        assert_eq!(
            forward_action(&HostEvent::Exit { code: 16 }, false, false, false),
            ForwardAction::FlushFatalThenSend
        );
        assert_eq!(
            forward_action(&HostEvent::Exit { code: 10 }, false, false, false),
            ForwardAction::FlushFatalThenSend
        );
    }

    #[test]
    fn drop_other_events_while_retry_possible() {
        let nav = HostEvent::Title { title: "x".into() };
        assert_eq!(
            forward_action(&nav, false, false, false),
            ForwardAction::Drop
        );
        assert_eq!(
            forward_action(&nav, true, false, false),
            ForwardAction::Send
        );
        assert_eq!(
            forward_action(&fatal(15), false, false, true),
            ForwardAction::Send
        );
    }
}
