//! `CHROME_DEVEL_SANDBOX` solo si el helper del slot es setuid-root (contrato 4.1).
//!
//! Chromium hace FATAL si el env apunta a un `chrome-sandbox` que no es 4755
//! root (tauri dev, AppImage squashfs, helper de usuario). El ADE decide
//! `--idq-no-sandbox` (userns / MAC). Aquí solo se exporta o se quita el env.
//!
//! Linux no es solo Ubuntu: AppArmor, SELinux y un host sin MAC (AppImage,
//! Alpine, contenedor) son superficies distintas. El MAC nunca hace usable un
//! helper `755` ni suprime uno `4755` root. `/proc/sys/kernel/apparmor_*` no
//! es la única señal y no cuenta como presencia.

use std::path::{Path, PathBuf};

/// LSM activo. `None` = AppImage / Alpine / kernel sin MAC montado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacKind {
    None,
    AppArmor,
    SeLinux,
    Both,
}

/// Qué hacer con el env del proceso host (lo heredan zygote / renderers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevelSandboxEnv {
    Set(PathBuf),
    Unset,
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

pub fn devel_sandbox_env(slot: &Path) -> Option<PathBuf> {
    let path = slot.join("chrome-sandbox");
    if helper_usable(&path) {
        Some(path)
    } else {
        None
    }
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
/// Un sysctl `apparmor_*` suelto no cuenta (proc bind-mount de otro host).
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

pub fn parse_sysctl_flag(value: &str) -> Option<bool> {
    match value.trim().trim_end_matches('\0') {
        "1" | "Y" | "y" => Some(true),
        "0" | "N" | "n" => Some(false),
        _ => None,
    }
}

/// El MAC no concede un helper no-SUID ni tapa uno SUID. Tabla para deb
/// (AppArmor), rpm (SELinux) y AppImage (sin MAC).
pub fn should_export_devel_sandbox(helper_ok: bool, mac: MacKind) -> bool {
    match mac {
        MacKind::None | MacKind::AppArmor | MacKind::SeLinux | MacKind::Both => helper_ok,
    }
}

pub fn devel_sandbox_decision(
    helper_ok: bool,
    mac: MacKind,
    slot_helper: &Path,
) -> DevelSandboxEnv {
    if should_export_devel_sandbox(helper_ok, mac) {
        DevelSandboxEnv::Set(slot_helper.to_path_buf())
    } else {
        DevelSandboxEnv::Unset
    }
}

/// Siempre pisa o quita. Un path heredado a un helper `755` es FATAL.
pub fn apply_devel_sandbox_decision(decision: &DevelSandboxEnv) {
    match decision {
        DevelSandboxEnv::Set(path) => std::env::set_var("CHROME_DEVEL_SANDBOX", path),
        DevelSandboxEnv::Unset => std::env::remove_var("CHROME_DEVEL_SANDBOX"),
    }
}

/// Si el helper del slot no es usable, quita el env (aunque viniera heredado).
/// Si es usable, exporta el del slot (no se conserva un path heredado distinto).
pub fn apply_devel_sandbox_env(slot: &Path) {
    let helper = slot.join("chrome-sandbox");
    apply_devel_sandbox_decision(&devel_sandbox_decision(
        helper_usable(&helper),
        detect_mac(),
        &helper,
    ));
}

pub fn detect_mac() -> MacKind {
    #[cfg(target_os = "linux")]
    {
        mac_kind_from(apparmor_present_live(), selinux_present_live())
    }
    #[cfg(not(target_os = "linux"))]
    {
        MacKind::None
    }
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

#[cfg(target_os = "linux")]
fn read_sysctl_flag(path: &str) -> Option<bool> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| parse_sysctl_flag(&value))
}

#[cfg(target_os = "linux")]
fn path_is_dir(path: &str) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn path_readable(path: &str) -> bool {
    std::fs::File::open(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    static SLOT_SEQ: AtomicU64 = AtomicU64::new(0);

    const MACS: [MacKind; 4] = [
        MacKind::None,
        MacKind::AppArmor,
        MacKind::SeLinux,
        MacKind::Both,
    ];

    fn temp_slot(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "idq-host-sandbox-{}-{}-{}",
            std::process::id(),
            SLOT_SEQ.fetch_add(1, Ordering::Relaxed),
            tag
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
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

    fn with_devel_env<F: FnOnce()>(f: F) {
        let _guard = ENV_LOCK.lock().expect("CHROME_DEVEL_SANDBOX env lock");
        let prev = std::env::var_os("CHROME_DEVEL_SANDBOX");
        f();
        match prev {
            Some(value) => std::env::set_var("CHROME_DEVEL_SANDBOX", value),
            None => std::env::remove_var("CHROME_DEVEL_SANDBOX"),
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

    #[test]
    fn helper_4755_vs_755_and_st_mode() {
        assert!(!helper_usable_from(0, 0o755, true));
        assert!(!helper_usable_from(0, 0o100000 | 0o755, true));
        assert!(helper_usable_from(0, 0o4755, true));
        assert!(helper_usable_from(0, 0o100000 | 0o4755, true));
        assert!(!helper_usable_from(1000, 0o4755, true));
        assert!(!helper_usable_from(1000, 0o100000 | 0o4755, true));
        assert!(!helper_usable_from(0, 0o2755, true));
        assert!(helper_usable_from(0, 0o6755, true));
        assert!(!helper_usable_from(0, 0o4644, true));
        assert!(helper_usable_from(0, 0o4700, true));
        assert!(!helper_usable_from(0, 0o4755, false));
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_plain_755_file() {
        let dir = temp_slot("755");
        let path = write_mode(&dir, "chrome-sandbox", 0o755);
        assert!(!helper_usable(&path));
        assert!(devel_sandbox_env(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_4755_when_not_root_owned() {
        let dir = temp_slot("user-4755");
        let path = write_mode(&dir, "chrome-sandbox", 0o4755);
        let meta = std::fs::metadata(&path).unwrap();
        use std::os::unix::fs::MetadataExt;
        assert_ne!(meta.uid(), 0, "el test no corre como root");
        assert!(!helper_usable(&path));
        assert!(devel_sandbox_env(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_missing_file_and_directory() {
        let dir = temp_slot("missing");
        let missing = dir.join("chrome-sandbox");
        assert!(!helper_usable(&missing));
        assert!(devel_sandbox_env(&dir).is_none());

        std::fs::create_dir(&missing).unwrap();
        assert!(!helper_usable(&missing));
        assert!(devel_sandbox_env(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
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
    fn apparmor_sysctl_alone_is_not_presence() {
        assert!(!apparmor_present_from(None, false, false));
        assert!(!apparmor_present_from(Some(false), false, false));
        assert!(apparmor_present_from(Some(true), false, false));
        assert!(apparmor_present_from(None, true, false));
        assert!(apparmor_present_from(None, false, true));
    }

    #[test]
    fn parse_kernel_flags() {
        assert_eq!(parse_sysctl_flag("1\n"), Some(true));
        assert_eq!(parse_sysctl_flag("0"), Some(false));
        assert_eq!(parse_sysctl_flag("Y"), Some(true));
        assert_eq!(parse_sysctl_flag("N\n"), Some(false));
        assert_eq!(parse_sysctl_flag("maybe"), None);
    }

    #[test]
    fn non_suid_helper_unsets_on_apparmor_selinux_and_no_mac() {
        let slot = Path::new("/slot/chrome-sandbox");
        for mac in MACS {
            assert!(
                !should_export_devel_sandbox(false, mac),
                "755 + {mac:?} no exporta"
            );
            assert_eq!(
                devel_sandbox_decision(false, mac, slot),
                DevelSandboxEnv::Unset,
                "755 + {mac:?} quita el env"
            );
        }
    }

    #[test]
    fn suid_helper_exports_on_apparmor_selinux_and_no_mac() {
        let slot = Path::new("/slot/chrome-sandbox");
        for mac in MACS {
            assert!(
                should_export_devel_sandbox(true, mac),
                "4755 root + {mac:?} exporta"
            );
            assert_eq!(
                devel_sandbox_decision(true, mac, slot),
                DevelSandboxEnv::Set(slot.to_path_buf()),
                "4755 root + {mac:?} pisa el path del slot"
            );
        }
    }

    #[test]
    fn appimage_and_rpm_and_deb_surfaces_follow_suid_only() {
        // AppImage / squashfs: helper 755, sin MAC.
        assert_eq!(
            devel_sandbox_decision(false, MacKind::None, Path::new("/app/chrome-sandbox")),
            DevelSandboxEnv::Unset
        );
        // deb Ubuntu: AppArmor, helper de usuario.
        assert_eq!(
            devel_sandbox_decision(false, MacKind::AppArmor, Path::new("/usr/chrome-sandbox")),
            DevelSandboxEnv::Unset
        );
        // rpm Fedora/RHEL: SELinux, helper 755 del slot.
        assert_eq!(
            devel_sandbox_decision(false, MacKind::SeLinux, Path::new("/usr/chrome-sandbox")),
            DevelSandboxEnv::Unset
        );
        // deb/rpm con helper 4755 root: se exporta aunque el MAC tape userns.
        assert_eq!(
            devel_sandbox_decision(true, MacKind::AppArmor, Path::new("/slot/chrome-sandbox")),
            DevelSandboxEnv::Set(PathBuf::from("/slot/chrome-sandbox"))
        );
        assert_eq!(
            devel_sandbox_decision(true, MacKind::SeLinux, Path::new("/slot/chrome-sandbox")),
            DevelSandboxEnv::Set(PathBuf::from("/slot/chrome-sandbox"))
        );
    }

    #[test]
    fn apply_decision_unsets_inherited_and_overwrites_stale_path() {
        with_devel_env(|| {
            std::env::set_var("CHROME_DEVEL_SANDBOX", "/inherited/chrome-sandbox");
            apply_devel_sandbox_decision(&DevelSandboxEnv::Unset);
            assert!(
                std::env::var_os("CHROME_DEVEL_SANDBOX").is_none(),
                "inherited 755/system path must go"
            );

            std::env::set_var("CHROME_DEVEL_SANDBOX", "/wrong/755-helper");
            apply_devel_sandbox_decision(&DevelSandboxEnv::Set(PathBuf::from(
                "/slot/chrome-sandbox",
            )));
            assert_eq!(
                std::env::var("CHROME_DEVEL_SANDBOX").unwrap(),
                "/slot/chrome-sandbox",
                "usable slot helper overwrites a stale inherited path"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn apply_unsets_inherited_when_helper_is_755() {
        with_devel_env(|| {
            let dir = temp_slot("apply-755");
            write_mode(&dir, "chrome-sandbox", 0o755);
            std::env::set_var("CHROME_DEVEL_SANDBOX", "/inherited/chrome-sandbox");
            apply_devel_sandbox_env(&dir);
            assert!(
                std::env::var_os("CHROME_DEVEL_SANDBOX").is_none(),
                "755 helper must unset inherited CHROME_DEVEL_SANDBOX"
            );
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[cfg(unix)]
    #[test]
    fn apply_unsets_inherited_when_helper_missing() {
        with_devel_env(|| {
            let dir = temp_slot("apply-missing");
            std::env::set_var("CHROME_DEVEL_SANDBOX", "/opt/google/chrome/chrome-sandbox");
            apply_devel_sandbox_env(&dir);
            assert!(std::env::var_os("CHROME_DEVEL_SANDBOX").is_none());
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[cfg(unix)]
    #[test]
    fn apply_unsets_inherited_when_helper_is_user_4755() {
        with_devel_env(|| {
            let dir = temp_slot("apply-user-4755");
            write_mode(&dir, "chrome-sandbox", 0o4755);
            std::env::set_var("CHROME_DEVEL_SANDBOX", dir.join("chrome-sandbox"));
            apply_devel_sandbox_env(&dir);
            assert!(
                std::env::var_os("CHROME_DEVEL_SANDBOX").is_none(),
                "user-owned 4755 is FATAL if exported"
            );
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_mac_does_not_treat_apparmor_sysctl_as_sole_signal() {
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
        // Sysctl suelto (o ausente) no cambia la presencia.
        let _ = sysctl_exists;
        assert_eq!(
            apparmor,
            apparmor_present_from(
                read_sysctl_flag("/sys/module/apparmor/parameters/enabled"),
                path_is_dir("/sys/kernel/security/apparmor"),
                path_readable("/proc/self/attr/apparmor/current"),
            )
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_generic_attr_current_is_not_apparmor_presence() {
        let generic = path_readable("/proc/self/attr/current");
        let apparmor_attr = path_readable("/proc/self/attr/apparmor/current");
        let module = read_sysctl_flag("/sys/module/apparmor/parameters/enabled");
        let securityfs = path_is_dir("/sys/kernel/security/apparmor");
        // `/proc/self/attr/current` existe en SELinux y en kernels "kernel".
        // No es señal de AppArmor: solo módulo, securityfs o attr/apparmor.
        if generic && !apparmor_attr && !securityfs && module != Some(true) {
            assert!(!apparmor_present_live());
            assert!(!apparmor_present_from(module, securityfs, false));
            assert_ne!(detect_mac(), MacKind::AppArmor);
            assert_ne!(detect_mac(), MacKind::Both);
        }
    }

    #[cfg(unix)]
    #[test]
    fn live_mac_still_unsets_inherited_when_helper_is_755() {
        with_devel_env(|| {
            let dir = temp_slot("live-mac-755");
            write_mode(&dir, "chrome-sandbox", 0o755);
            std::env::set_var("CHROME_DEVEL_SANDBOX", "/inherited/chrome-sandbox");
            apply_devel_sandbox_env(&dir);
            assert!(
                std::env::var_os("CHROME_DEVEL_SANDBOX").is_none(),
                "live MAC {:?} must not keep inherited env for a 755 helper",
                detect_mac()
            );
            assert!(!should_export_devel_sandbox(false, detect_mac()));
            let _ = std::fs::remove_dir_all(&dir);
        });
    }
}
