//! Política del sandbox de Chromium en el ADE (contrato 4.1 / 5).
//!
//! El helper SUID y los user namespaces se deciden *antes* de spawnear.
//! El crash+retry queda como red si el probe se equivoca; esos eventos
//! no llegan al frontend.
//!
//! Linux no es solo Ubuntu: el helper `4755` (deb/rpm), userns, AppArmor,
//! SELinux y un host sin MAC (AppImage, Alpine, contenedor) son superficies
//! distintas. `/proc/sys/kernel/apparmor_*` no es la única señal.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::ipc::HostEvent;

/// Códigos con los que el ADE reintenta una vez sin sandbox (contrato 5).
/// `1` es un abort (SIGABRT: `ExitStatus::code() == None` → 1).
/// No recortar esta lista: el retry es el workaround si el probe falla.
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

/// LSM activo. `None` = AppImage / Alpine / kernel sin MAC montado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacKind {
    None,
    AppArmor,
    SeLinux,
    Both,
}

pub fn env_no_sandbox_from(value: Option<&str>) -> bool {
    matches!(value, Some("1"))
}

pub fn env_no_sandbox() -> bool {
    env_no_sandbox_from(std::env::var("IDIOTEQUE_CEF_NO_SANDBOX").ok().as_deref())
}

/// Chromium hace FATAL si el env apunta a un `chrome-sandbox` que no es 4755 root.
/// `mode` puede ser solo permisos (`0o4755`) o `st_mode` completo (`S_IFREG | 0o4755`).
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

pub fn mac_kind_from(apparmor: bool, selinux: bool) -> MacKind {
    match (apparmor, selinux) {
        (false, false) => MacKind::None,
        (true, false) => MacKind::AppArmor,
        (false, true) => MacKind::SeLinux,
        (true, true) => MacKind::Both,
    }
}

/// AppArmor está presente si el módulo, securityfs o `attr/apparmor` lo dicen.
/// Un sysctl `apparmor_*` suelto no cuenta: puede existir en un proc bind-mount
/// de otro host sin que este proceso esté bajo AppArmor.
pub fn apparmor_present_from(
    module_enabled: Option<bool>,
    securityfs_present: bool,
    apparmor_attr_present: bool,
) -> bool {
    module_enabled == Some(true) || securityfs_present || apparmor_attr_present
}

pub fn selinux_present_from(sysfs_present: bool, enforce_readable: bool) -> bool {
    sysfs_present || enforce_readable
}

/// Cualquier señal de restricción AppArmor, no solo
/// `/proc/sys/kernel/apparmor_restrict_unprivileged_userns`.
pub fn apparmor_restricts_from<I>(signals: I) -> bool
where
    I: IntoIterator<Item = bool>,
{
    signals.into_iter().any(|on| on)
}

pub fn is_apparmor_unconfined(profile: &str) -> bool {
    let trimmed = profile.trim().trim_end_matches('\0').trim();
    trimmed.split_whitespace().next() == Some("unconfined")
}

/// `unconfined` + restrict transiciona al perfil `unprivileged_userns`.
/// Un contexto SELinux `unconfined_u:…:unconfined_t:s0` no es ese perfil.
pub fn apparmor_blocks_userns_from(restrict: bool, profile: &str) -> bool {
    restrict && is_apparmor_unconfined(profile)
}

/// SELinux no usa sysctls `apparmor_*`. Si enforcing y el boolean
/// `unpriv_user_ns` está off (RHEL/Fedora), userns no sirven. Sin boolean
/// (archivo ausente) no hay atajo: decide el probe `clone`.
pub fn selinux_blocks_userns_from(enforcing: bool, unpriv_user_ns: Option<bool>) -> bool {
    enforcing && unpriv_user_ns == Some(false)
}

pub fn mac_blocks_userns_from(
    mac: MacKind,
    apparmor_restrict: bool,
    apparmor_profile: &str,
    selinux_enforcing: bool,
    selinux_unpriv_user_ns: Option<bool>,
) -> bool {
    let apparmor = apparmor_blocks_userns_from(apparmor_restrict, apparmor_profile);
    let selinux = selinux_blocks_userns_from(selinux_enforcing, selinux_unpriv_user_ns);
    match mac {
        MacKind::None => false,
        MacKind::AppArmor => apparmor,
        MacKind::SeLinux => selinux,
        MacKind::Both => apparmor || selinux,
    }
}

/// Perfil AppArmor. `/proc/self/attr/current` solo si el LSM *es* AppArmor;
/// en SELinux ese archivo es el contexto y no se usa como perfil.
pub fn apparmor_profile_from(
    apparmor_attr: Option<&str>,
    generic_attr: Option<&str>,
    mac: MacKind,
) -> String {
    if let Some(profile) = apparmor_attr {
        return profile.to_string();
    }
    if mac == MacKind::AppArmor {
        return generic_attr.unwrap_or("").to_string();
    }
    String::new()
}

/// Gates de kernel (Debian `unprivileged_userns_clone`, RHEL
/// `max_user_namespaces=0`). Archivo ausente ≠ deshabilitado.
pub fn kernel_userns_disabled_from(
    max_user_namespaces: Option<u64>,
    unprivileged_userns_clone: Option<bool>,
) -> bool {
    matches!(max_user_namespaces, Some(0)) || matches!(unprivileged_userns_clone, Some(false))
}

pub fn userns_available_from(kernel_disabled: bool, mac_blocks: bool, clone_ok: bool) -> bool {
    !kernel_disabled && !mac_blocks && clone_ok
}

pub fn parse_sysctl_flag(value: &str) -> Option<bool> {
    match value.trim().trim_end_matches('\0') {
        "1" | "Y" | "y" => Some(true),
        "0" | "N" | "n" => Some(false),
        _ => None,
    }
}

pub fn parse_sysctl_u64(value: &str) -> Option<u64> {
    value.trim().trim_end_matches('\0').parse().ok()
}

/// Boolean SELinux: `current pending` (`1 1`) o un solo `0`/`1`.
pub fn parse_selinux_boolean(value: &str) -> Option<bool> {
    let first = value.split_whitespace().next()?;
    parse_sysctl_flag(first)
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
        let mac = detect_mac();
        let kernel_disabled = kernel_userns_disabled_from(
            read_sysctl_u64("/proc/sys/user/max_user_namespaces"),
            read_sysctl_flag("/proc/sys/kernel/unprivileged_userns_clone"),
        );
        let apparmor_restrict = apparmor_restricts_from([
            flag_file("/proc/sys/kernel/apparmor_restrict_unprivileged_userns"),
            flag_file("/proc/sys/kernel/apparmor_restrict_unprivileged_unconfined"),
        ]);
        let profile = apparmor_profile_from(
            read_trimmed("/proc/self/attr/apparmor/current").as_deref(),
            read_trimmed("/proc/self/attr/current").as_deref(),
            mac,
        );
        let selinux_enforcing = flag_file("/sys/fs/selinux/enforce");
        let unpriv_user_ns = read_trimmed("/sys/fs/selinux/booleans/unpriv_user_ns")
            .and_then(|value| parse_selinux_boolean(&value));
        let mac_blocks = mac_blocks_userns_from(
            mac,
            apparmor_restrict,
            &profile,
            selinux_enforcing,
            unpriv_user_ns,
        );
        let clone_ok = if kernel_disabled || mac_blocks {
            false
        } else {
            probe_unshare_via_clone()
        };
        userns_available_from(kernel_disabled, mac_blocks, clone_ok)
    }
}

#[cfg(target_os = "linux")]
fn detect_mac() -> MacKind {
    mac_kind_from(apparmor_present_live(), selinux_present_live())
}

#[cfg(target_os = "linux")]
fn apparmor_present_live() -> bool {
    apparmor_present_from(
        read_sysctl_flag("/sys/module/apparmor/parameters/enabled"),
        path_is_dir("/sys/kernel/security/apparmor"),
        path_readable("/proc/self/attr/apparmor/current"),
    )
}

#[cfg(target_os = "linux")]
fn selinux_present_live() -> bool {
    selinux_present_from(
        path_is_dir("/sys/fs/selinux"),
        path_readable("/sys/fs/selinux/enforce"),
    )
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().trim_end_matches('\0').to_string())
}

fn read_sysctl_flag(path: &str) -> Option<bool> {
    read_trimmed(path).and_then(|value| parse_sysctl_flag(&value))
}

fn read_sysctl_u64(path: &str) -> Option<u64> {
    read_trimmed(path).and_then(|value| parse_sysctl_u64(&value))
}

fn flag_file(path: &str) -> bool {
    read_sysctl_flag(path).unwrap_or(false)
}

fn path_is_dir(path: &str) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

fn path_readable(path: &str) -> bool {
    std::fs::metadata(path).is_ok()
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
    use std::path::{Path, PathBuf};
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

    fn exit(code: i32) -> HostEvent {
        HostEvent::Exit { code }
    }

    #[cfg(unix)]
    fn write_mode(dir: &Path, name: &str, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, b"x").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(mode);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[test]
    fn helper_requires_setuid_root() {
        assert!(!helper_usable_from(1000, 0o755, true));
        assert!(!helper_usable_from(0, 0o755, true));
        assert!(!helper_usable_from(0, 0o4755, false));
        assert!(!helper_usable_from(0, 0o4000, true));
        assert!(helper_usable_from(0, 0o4755, true));
    }

    #[test]
    fn helper_4755_vs_755_and_st_mode() {
        // 755 nunca sirve, ni root.
        assert!(!helper_usable_from(0, 0o755, true));
        assert!(!helper_usable_from(0, 0o100000 | 0o755, true));
        // 4755 root sí, también con bits S_IFREG de stat(2).
        assert!(helper_usable_from(0, 0o4755, true));
        assert!(helper_usable_from(0, 0o100000 | 0o4755, true));
        // 4755 de usuario (AppImage / tauri dev) es FATAL si se exporta.
        assert!(!helper_usable_from(1000, 0o4755, true));
        assert!(!helper_usable_from(1000, 0o100000 | 0o4755, true));
        // setgid solo, sin setuid.
        assert!(!helper_usable_from(0, 0o2755, true));
        // setuid+setgid root: Chromium acepta el bit 4000.
        assert!(helper_usable_from(0, 0o6755, true));
        // setuid sin ningún bit de ejecución.
        assert!(!helper_usable_from(0, 0o4644, true));
        // setuid + exec de owner (4700) basta; no exigimos 4755 literal.
        assert!(helper_usable_from(0, 0o4700, true));
        // directorio llamado chrome-sandbox.
        assert!(!helper_usable_from(0, 0o4755, false));
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_a_plain_755_file() {
        let tmp = TempDir::new().unwrap();
        let path = write_mode(tmp.path(), "chrome-sandbox", 0o755);
        assert!(!helper_usable(&path));
        assert!(devel_sandbox_env(tmp.path()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_4755_when_not_root_owned() {
        let tmp = TempDir::new().unwrap();
        let path = write_mode(tmp.path(), "chrome-sandbox", 0o4755);
        let meta = std::fs::metadata(&path).unwrap();
        use std::os::unix::fs::MetadataExt;
        assert_ne!(meta.uid(), 0, "el test no corre como root");
        assert!(!helper_usable(&path));
        assert!(devel_sandbox_env(tmp.path()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_missing_file_and_directory() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("chrome-sandbox");
        assert!(!helper_usable(&missing));
        assert!(devel_sandbox_env(tmp.path()).is_none());

        std::fs::create_dir(&missing).unwrap();
        assert!(!helper_usable(&missing));
        assert!(devel_sandbox_env(tmp.path()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn apply_devel_sandbox_env_clears_inherited_when_helper_is_755() {
        let tmp = TempDir::new().unwrap();
        write_mode(tmp.path(), "chrome-sandbox", 0o755);
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("printf %s \"${CHROME_DEVEL_SANDBOX-UNSET}\"");
        cmd.env("CHROME_DEVEL_SANDBOX", "/inherited/chrome-sandbox");
        apply_devel_sandbox_env(&mut cmd, tmp.path());
        let out = cmd.output().expect("sh");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "UNSET");
    }

    #[cfg(unix)]
    #[test]
    fn apply_devel_sandbox_env_clears_inherited_when_helper_missing() {
        let tmp = TempDir::new().unwrap();
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("printf %s \"${CHROME_DEVEL_SANDBOX-UNSET}\"");
        cmd.env("CHROME_DEVEL_SANDBOX", "/opt/google/chrome/chrome-sandbox");
        apply_devel_sandbox_env(&mut cmd, tmp.path());
        let out = cmd.output().expect("sh");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "UNSET");
    }

    #[test]
    fn env_no_sandbox_is_exactly_one() {
        assert!(env_no_sandbox_from(Some("1")));
        assert!(!env_no_sandbox_from(Some("0")));
        assert!(!env_no_sandbox_from(Some("true")));
        assert!(!env_no_sandbox_from(Some("yes")));
        assert!(!env_no_sandbox_from(Some("")));
        assert!(!env_no_sandbox_from(None));
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
    fn apparmor_unconfined_is_not_selinux_context() {
        assert!(is_apparmor_unconfined("unconfined"));
        assert!(is_apparmor_unconfined("unconfined (enforce)"));
        assert!(is_apparmor_unconfined("unconfined (complain)\n"));
        assert!(!is_apparmor_unconfined(
            "unconfined_u:unconfined_r:unconfined_t:s0"
        ));
        assert!(!is_apparmor_unconfined(
            "system_u:system_r:container_t:s0:c0.c1023"
        ));
        assert!(!apparmor_blocks_userns_from(
            true,
            "unconfined_u:unconfined_r:unconfined_t:s0"
        ));
        assert!(apparmor_blocks_userns_from(true, "unconfined (enforce)"));
    }

    #[test]
    fn apparmor_sysctl_alone_is_not_presence() {
        assert!(!apparmor_present_from(None, false, false));
        assert!(!apparmor_present_from(Some(false), false, false));
        assert!(apparmor_present_from(Some(true), false, false));
        assert!(apparmor_present_from(None, true, false));
        assert!(apparmor_present_from(None, false, true));
    }

    #[test]
    fn apparmor_restrict_accepts_any_signal_not_only_userns_sysctl() {
        assert!(!apparmor_restricts_from([false, false]));
        assert!(apparmor_restricts_from([true, false]));
        assert!(apparmor_restricts_from([false, true]));
    }

    #[test]
    fn mac_kind_covers_apparmor_selinux_and_none() {
        assert_eq!(mac_kind_from(false, false), MacKind::None);
        assert_eq!(mac_kind_from(true, false), MacKind::AppArmor);
        assert_eq!(mac_kind_from(false, true), MacKind::SeLinux);
        assert_eq!(mac_kind_from(true, true), MacKind::Both);
        assert!(selinux_present_from(true, false));
        assert!(selinux_present_from(false, true));
        assert!(!selinux_present_from(false, false));
    }

    #[test]
    fn apparmor_profile_ignores_generic_attr_unless_apparmor_is_the_lsm() {
        let selinux = "unconfined_u:unconfined_r:unconfined_t:s0";
        assert_eq!(
            apparmor_profile_from(None, Some(selinux), MacKind::SeLinux),
            ""
        );
        assert_eq!(
            apparmor_profile_from(None, Some(selinux), MacKind::None),
            ""
        );
        assert_eq!(
            apparmor_profile_from(None, Some("unconfined"), MacKind::AppArmor),
            "unconfined"
        );
        assert_eq!(
            apparmor_profile_from(
                Some("cef-host (enforce)"),
                Some("unconfined"),
                MacKind::Both
            ),
            "cef-host (enforce)"
        );
    }

    #[test]
    fn no_mac_does_not_block_even_if_stray_apparmor_sysctl() {
        assert!(!mac_blocks_userns_from(
            MacKind::None,
            true,
            "unconfined",
            true,
            Some(false)
        ));
        assert!(userns_available_from(false, false, true));
    }

    #[test]
    fn apparmor_surface_blocks_unconfined_restrict_not_confined_profile() {
        assert!(mac_blocks_userns_from(
            MacKind::AppArmor,
            true,
            "unconfined",
            false,
            None
        ));
        assert!(!mac_blocks_userns_from(
            MacKind::AppArmor,
            true,
            "cef-host (enforce)",
            false,
            None
        ));
        assert!(!mac_blocks_userns_from(
            MacKind::AppArmor,
            false,
            "unconfined",
            false,
            None
        ));
    }

    #[test]
    fn selinux_surface_uses_boolean_not_apparmor_sysctl() {
        assert!(mac_blocks_userns_from(
            MacKind::SeLinux,
            true,
            "unconfined",
            true,
            Some(false)
        ));
        assert!(!mac_blocks_userns_from(
            MacKind::SeLinux,
            true,
            "unconfined",
            true,
            Some(true)
        ));
        assert!(!mac_blocks_userns_from(
            MacKind::SeLinux,
            true,
            "unconfined",
            true,
            None
        ));
        assert!(!mac_blocks_userns_from(
            MacKind::SeLinux,
            true,
            "unconfined",
            false,
            Some(false)
        ));
        assert!(!selinux_blocks_userns_from(false, Some(false)));
        assert!(selinux_blocks_userns_from(true, Some(false)));
    }

    #[test]
    fn both_macs_block_if_either_surface_blocks() {
        assert!(mac_blocks_userns_from(
            MacKind::Both,
            true,
            "unconfined",
            false,
            Some(true)
        ));
        assert!(mac_blocks_userns_from(
            MacKind::Both,
            false,
            "",
            true,
            Some(false)
        ));
        assert!(!mac_blocks_userns_from(
            MacKind::Both,
            false,
            "unconfined",
            true,
            Some(true)
        ));
    }

    #[test]
    fn kernel_userns_knobs_are_distro_agnostic() {
        assert!(kernel_userns_disabled_from(Some(0), None));
        assert!(kernel_userns_disabled_from(None, Some(false)));
        assert!(kernel_userns_disabled_from(Some(0), Some(true)));
        assert!(!kernel_userns_disabled_from(Some(256), None));
        assert!(!kernel_userns_disabled_from(None, None));
        assert!(!kernel_userns_disabled_from(Some(256), Some(true)));
        assert!(!userns_available_from(true, false, true));
        assert!(!userns_available_from(false, true, true));
        assert!(!userns_available_from(false, false, false));
        assert!(userns_available_from(false, false, true));
    }

    #[test]
    fn parse_kernel_and_selinux_sys_values() {
        assert_eq!(parse_sysctl_flag("1\n"), Some(true));
        assert_eq!(parse_sysctl_flag("0"), Some(false));
        assert_eq!(parse_sysctl_flag("Y"), Some(true));
        assert_eq!(parse_sysctl_flag("N\n"), Some(false));
        assert_eq!(parse_sysctl_flag("maybe"), None);
        assert_eq!(parse_sysctl_u64("256\n"), Some(256));
        assert_eq!(parse_sysctl_u64("0"), Some(0));
        assert_eq!(parse_sysctl_u64("nope"), None);
        assert_eq!(parse_selinux_boolean("1 1\n"), Some(true));
        assert_eq!(parse_selinux_boolean("0 1"), Some(false));
        assert_eq!(parse_selinux_boolean("1"), Some(true));
        assert_eq!(parse_selinux_boolean(""), None);
    }

    #[test]
    fn wants_no_sandbox_on_deb_rpm_and_appimage_surfaces() {
        // AppImage / squashfs: helper 755, userns a veces ok.
        assert!(!wants_no_sandbox_from(false, false, true));
        assert!(wants_no_sandbox_from(false, false, false));
        // deb/rpm con helper 4755 root: sandbox aunque userns esté tapado.
        assert!(!wants_no_sandbox_from(false, true, false));
        // Ubuntu AppArmor unconfined+restrict: userns no usable.
        let apparmor_blocks =
            mac_blocks_userns_from(MacKind::AppArmor, true, "unconfined", false, None);
        let userns = userns_available_from(false, apparmor_blocks, true);
        assert!(!userns);
        assert!(wants_no_sandbox_from(false, false, userns));
        // Fedora SELinux unconfined_t + boolean on: clone decide (aquí ok).
        let selinux_ok = !mac_blocks_userns_from(MacKind::SeLinux, false, "", true, Some(true));
        assert!(userns_available_from(false, !selinux_ok, true));
        // RHEL max_user_namespaces=0.
        assert!(wants_no_sandbox_from(
            false,
            false,
            userns_available_from(true, false, true)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_mac_detection_does_not_treat_apparmor_sysctl_as_sole_signal() {
        let sysctl_exists =
            Path::new("/proc/sys/kernel/apparmor_restrict_unprivileged_userns").is_file();
        let apparmor = apparmor_present_live();
        let selinux = selinux_present_live();
        let mac = detect_mac();
        assert_eq!(mac, mac_kind_from(apparmor, selinux));
        if !apparmor {
            assert_ne!(mac, MacKind::AppArmor);
            assert_ne!(mac, MacKind::Both);
        }
        let _ = sysctl_exists;
        let once = user_namespaces_available();
        assert_eq!(once, user_namespaces_available());
    }

    #[test]
    fn retry_codes_keep_1_11_15_workaround() {
        assert_eq!(SANDBOX_RETRY_CODES, &[1, 11, 15]);
        assert!(is_sandbox_retry_exit(1));
        assert!(is_sandbox_retry_exit(11));
        assert!(is_sandbox_retry_exit(15));
        assert!(!is_sandbox_retry_exit(10));
        assert!(!is_sandbox_retry_exit(16));
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
    fn forward_action_matrix_10_11_15_16() {
        for code in [10, 11, 15, 16] {
            assert_eq!(
                forward_action(&fatal(code), false, false, false),
                ForwardAction::HoldFatal,
                "fatal {code} while retry possible"
            );
        }
        assert_eq!(
            forward_action(&exit(10), false, false, false),
            ForwardAction::FlushFatalThenSend
        );
        assert_eq!(
            forward_action(&exit(11), false, false, false),
            ForwardAction::RetrySandbox
        );
        assert_eq!(
            forward_action(&exit(15), false, false, false),
            ForwardAction::RetrySandbox
        );
        assert_eq!(
            forward_action(&exit(16), false, false, false),
            ForwardAction::FlushFatalThenSend
        );
        for code in [10, 11, 15, 16] {
            assert_eq!(
                forward_action(&exit(code), true, false, false),
                ForwardAction::Send,
                "exit {code} after ready"
            );
            assert_eq!(
                forward_action(&fatal(code), true, false, false),
                ForwardAction::Send,
                "fatal {code} after ready"
            );
            assert_eq!(
                forward_action(&exit(code), false, true, false),
                ForwardAction::Send,
                "exit {code} after retry"
            );
            assert_eq!(
                forward_action(&exit(code), false, false, true),
                ForwardAction::Send,
                "exit {code} launched --idq-no-sandbox"
            );
        }
        assert_eq!(
            forward_action(&ready(), false, false, false),
            ForwardAction::Send
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
        assert_eq!(
            forward_action(&HostEvent::Unknown, false, false, false),
            ForwardAction::Drop
        );
        assert_eq!(
            forward_action(
                &HostEvent::Health {
                    ok: true,
                    cef: "x".into(),
                    chromium: "y".into(),
                    api_version: 15200,
                },
                false,
                false,
                false
            ),
            ForwardAction::Drop
        );
    }
}
