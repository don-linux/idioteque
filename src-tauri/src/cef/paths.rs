//! Rutas del runtime CEF (home del usuario, base bundleada, sidecar).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use tauri::{AppHandle, Manager};

/// Clave de plataforma del índice oficial de CEF.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const PLATFORM: &str = "linux64";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const PLATFORM: &str = "linuxarm64";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub const PLATFORM: &str = "windows64";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const PLATFORM: &str = "macosx64";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const PLATFORM: &str = "macosarm64";
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
pub const PLATFORM: &str = "linux64";

const BASE_JSON: &str = include_str!("../../cef/base.json");

/// Metadatos del base bundleado (`src-tauri/cef/base.json`).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BaseInfo {
    pub cef_version: String,
    pub chromium_version: String,
    pub host_api_version: u32,
    pub api_version_min: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaseInfoFile {
    cef_version: String,
    chromium_version: String,
    host_api_version: u32,
    api_version_min: u32,
}

/// Info del base compilada en el binario.
pub fn base_info() -> &'static BaseInfo {
    static INFO: OnceLock<BaseInfo> = OnceLock::new();
    INFO.get_or_init(|| {
        let parsed: BaseInfoFile = serde_json::from_str(BASE_JSON)
            .expect("src-tauri/cef/base.json debe parsear");
        BaseInfo {
            cef_version: parsed.cef_version,
            chromium_version: parsed.chromium_version,
            host_api_version: parsed.host_api_version,
            api_version_min: parsed.api_version_min,
        }
    })
}

/// `~/.idioteque/cef` (o `IDIOTEQUE_CEF_HOME`) y el slot base bundleado.
#[derive(Debug, Clone)]
pub struct CefPaths {
    /// `~/.idioteque/cef` o `IDIOTEQUE_CEF_HOME`.
    pub home: PathBuf,
    /// `<resource_dir>/cef/base` o `IDIOTEQUE_CEF_BASE_DIR`.
    pub bundled_base: PathBuf,
}

impl CefPaths {
    #[allow(dead_code)]
    pub fn new(home: PathBuf, bundled_base: PathBuf) -> Self {
        Self { home, bundled_base }
    }

    pub fn from_app(app: &AppHandle) -> Result<Self, String> {
        let home = match std::env::var_os("IDIOTEQUE_CEF_HOME") {
            Some(value) => PathBuf::from(value),
            None => app
                .path()
                .home_dir()
                .map_err(|error| format!("No se pudo resolver el home: {error}"))?
                .join(".idioteque")
                .join("cef"),
        };

        let bundled_base = match std::env::var_os("IDIOTEQUE_CEF_BASE_DIR") {
            Some(value) => PathBuf::from(value),
            None => app
                .path()
                .resource_dir()
                .map_err(|error| {
                    format!("No se pudo resolver el directorio de recursos: {error}")
                })?
                .join("cef")
                .join("base"),
        };

        Ok(Self { home, bundled_base })
    }

    pub fn current(&self) -> PathBuf {
        self.home.join("current")
    }

    pub fn candidate(&self) -> PathBuf {
        self.home.join("candidate")
    }

    pub fn current_old(&self) -> PathBuf {
        self.home.join("current.old")
    }

    pub fn profile(&self) -> PathBuf {
        self.home.join("profile")
    }

    pub fn health_cache(&self, pid: u32) -> PathBuf {
        self.home.join(format!("health-cache-{pid}"))
    }

    pub fn denylist_file(&self) -> PathBuf {
        self.home.join("denylist.json")
    }

    pub fn state_file(&self) -> PathBuf {
        self.home.join("state.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.home.join("logs")
    }

    /// Crea `home`, `profile/` y `logs/`.
    pub fn ensure_dirs(&self) -> Result<(), String> {
        create_dir(&self.home)?;
        create_dir(&self.profile())?;
        create_dir(&self.logs_dir())?;
        Ok(())
    }
}

/// Sidecar `cef-host` junto al ejecutable (`.exe` en Windows).
/// Override: `IDIOTEQUE_CEF_HOST_BIN`.
pub fn host_binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("IDIOTEQUE_CEF_HOST_BIN") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "No se encontró el binario cef-host en `{}`",
            path.display()
        ));
    }

    let exe = tauri::process::current_binary(&app.env())
        .or_else(|_| std::env::current_exe())
        .map_err(|error| format!("No se pudo resolver el ejecutable: {error}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "Ruta del ejecutable inválida".to_string())?;
    let name = if cfg!(windows) {
        "cef-host.exe"
    } else {
        "cef-host"
    };
    let path = dir.join(name);
    if !path.is_file() {
        return Err(format!(
            "No se encontró el binario cef-host en `{}`",
            path.display()
        ));
    }
    Ok(path)
}

fn create_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn platform_is_a_known_index_key() {
        assert!(
            [
                "linux64",
                "linuxarm64",
                "windows64",
                "macosx64",
                "macosarm64"
            ]
            .contains(&PLATFORM)
        );
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(PLATFORM, "linux64");
    }

    #[test]
    fn base_info_matches_base_json() {
        let info = base_info();
        assert_eq!(
            info.cef_version,
            "152.0.6+g708dc14+chromium-152.0.7977.83"
        );
        assert_eq!(info.chromium_version, "152.0.7977.83");
        assert_eq!(info.host_api_version, 15200);
        assert_eq!(info.api_version_min, 13300);
    }

    #[test]
    fn new_and_ensure_dirs_layout() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("cef-home");
        let base = tmp.path().join("base");
        let paths = CefPaths::new(home.clone(), base.clone());

        assert_eq!(paths.current(), home.join("current"));
        assert_eq!(paths.candidate(), home.join("candidate"));
        assert_eq!(paths.current_old(), home.join("current.old"));
        assert_eq!(paths.profile(), home.join("profile"));
        assert_eq!(paths.health_cache(42), home.join("health-cache-42"));
        assert_eq!(paths.denylist_file(), home.join("denylist.json"));
        assert_eq!(paths.state_file(), home.join("state.json"));
        assert_eq!(paths.logs_dir(), home.join("logs"));
        assert_eq!(paths.bundled_base, base);

        paths.ensure_dirs().expect("ensure");
        assert!(home.is_dir());
        assert!(paths.profile().is_dir());
        assert!(paths.logs_dir().is_dir());
    }
}
