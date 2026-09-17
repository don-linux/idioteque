//! `CHROME_DEVEL_SANDBOX` solo si el helper es setuid-root (contrato 4.1).

use std::path::Path;

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

/// Si el helper del slot no es usable, quita el env (aunque viniera heredado).
pub fn apply_devel_sandbox_env(slot: &Path) {
    let sandbox = slot.join("chrome-sandbox");
    if helper_usable(&sandbox) {
        if std::env::var_os("CHROME_DEVEL_SANDBOX").is_none() {
            std::env::set_var("CHROME_DEVEL_SANDBOX", &sandbox);
        }
    } else {
        std::env::remove_var("CHROME_DEVEL_SANDBOX");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn helper_rejects_plain_755_file() {
        let dir = std::env::temp_dir().join(format!(
            "idq-sandbox-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("chrome-sandbox");
        std::fs::write(&path, b"x").expect("write");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        assert!(!helper_usable(&path));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
