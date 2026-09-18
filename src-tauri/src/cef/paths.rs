//! Rutas del runtime CEF (home del usuario, base bundleada, sidecar).

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use tauri::{AppHandle, Manager};

/// Clave de plataforma del índice oficial de CEF. Idioteque solo usa linux64.
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
        let parsed: BaseInfoFile =
            serde_json::from_str(BASE_JSON).expect("src-tauri/cef/base.json debe parsear");
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
        let home = match override_path(std::env::var_os("IDIOTEQUE_CEF_HOME").as_deref()) {
            Some(value) => value,
            None => default_user_home(
                &app.path()
                    .home_dir()
                    .map_err(|error| format!("No se pudo resolver el home: {error}"))?,
            ),
        };

        let bundled_base =
            match override_path(std::env::var_os("IDIOTEQUE_CEF_BASE_DIR").as_deref()) {
                Some(value) => value,
                None => {
                    bundled_base_from_resource_dir(&app.path().resource_dir().map_err(|error| {
                        format!("No se pudo resolver el directorio de recursos: {error}")
                    })?)
                }
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

/// Sidecar `cef-host` junto al ejecutable.
/// Override: `IDIOTEQUE_CEF_HOST_BIN`.
///
/// Se mira primero junto a `std::env::current_exe()`: dentro de una AppImage
/// es `$APPDIR/usr/bin/idioteque`, que es donde va el sidecar, mientras que
/// `tauri::process::current_binary` devuelve la ruta del `.AppImage` en sí
/// (pensada para reiniciar la app) y ahí no hay ningún `cef-host`. Ese
/// segundo candidato queda como respaldo.
pub fn host_binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    let env_override = override_path(std::env::var_os("IDIOTEQUE_CEF_HOST_BIN").as_deref());

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        candidates.push(exe);
    }
    if let Ok(exe) = tauri::process::current_binary(&app.env()) {
        candidates.push(exe);
    }

    resolve_host_binary(env_override, &candidates)
}

/// Override de ruta: `None` o cadena vacía → no hay override.
///
/// `IDIOTEQUE_CEF_*=` vacío (wrappers, AppImage, systemd) no debe interpretarse
/// como el directorio actual.
fn override_path(value: Option<&OsStr>) -> Option<PathBuf> {
    value.and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

/// `~/.idioteque/cef` a partir del home del usuario (contrato 3.2).
fn default_user_home(user_home: &Path) -> PathBuf {
    user_home.join(".idioteque").join("cef")
}

/// `<resource_dir>/cef/base`. En AppImage `resource_dir` es
/// `$APPDIR/usr/lib/idioteque` (contrato 3.1); no se lee `$APPDIR` a mano.
fn bundled_base_from_resource_dir(resource_dir: &Path) -> PathBuf {
    resource_dir.join("cef").join("base")
}

/// Sidecar junto a cada candidato a ejecutable.
///
/// AppImage: `current_exe` va primero porque dentro del squash es
/// `$APPDIR/usr/bin/idioteque`; `current_binary` apunta al `.AppImage`
/// y ahí no hay `cef-host`. No se usa `std::env::var("APPDIR")`.
fn resolve_host_binary(
    env_override: Option<PathBuf>,
    exe_candidates: &[PathBuf],
) -> Result<PathBuf, String> {
    if let Some(path) = env_override {
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "No se encontró el binario cef-host en `{}`",
            path.display()
        ));
    }

    if exe_candidates.is_empty() {
        return Err("No se pudo resolver el ejecutable".to_string());
    }

    let paths: Vec<PathBuf> = exe_candidates
        .iter()
        .filter_map(|exe| exe.parent().map(|dir| dir.join(host_binary_name())))
        .collect();
    if let Some(found) = paths.iter().find(|path| path.is_file()) {
        return Ok(found.clone());
    }
    let searched: Vec<String> = paths
        .iter()
        .map(|path| format!("`{}`", path.display()))
        .collect();
    Err(format!(
        "No se encontró el binario cef-host en {}",
        searched.join(" ni ")
    ))
}

fn host_binary_name() -> &'static str {
    "cef-host"
}

fn create_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use tempfile::TempDir;

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, bytes).expect("write");
    }

    #[test]
    fn platform_is_a_known_index_key() {
        assert_eq!(PLATFORM, "linux64");
    }

    #[test]
    fn base_info_matches_base_json() {
        let info = base_info();
        assert_eq!(info.cef_version, "152.0.6+g708dc14+chromium-152.0.7977.83");
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
        assert_eq!(paths.health_cache(0), home.join("health-cache-0"));
        assert_eq!(paths.denylist_file(), home.join("denylist.json"));
        assert_eq!(paths.state_file(), home.join("state.json"));
        assert_eq!(paths.logs_dir(), home.join("logs"));
        assert_eq!(paths.bundled_base, base);

        paths.ensure_dirs().expect("ensure");
        assert!(home.is_dir());
        assert!(paths.profile().is_dir());
        assert!(paths.logs_dir().is_dir());
        assert!(!paths.current().exists());
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
    }

    #[test]
    fn override_path_treats_empty_as_unset() {
        assert_eq!(override_path(None), None);
        assert_eq!(override_path(Some(OsStr::new(""))), None);
        assert_eq!(
            override_path(Some(OsStr::new("/custom/cef"))),
            Some(PathBuf::from("/custom/cef"))
        );
        // Espacios son un path válido; no se recortan.
        assert_eq!(
            override_path(Some(OsStr::new(" /tmp/cef "))),
            Some(PathBuf::from(" /tmp/cef "))
        );
    }

    #[test]
    fn env_overrides_win_over_default_home_and_resource_dir() {
        let user_home = Path::new("/home/fernando");
        let resource = Path::new("/usr/lib/idioteque");

        let home = override_path(Some(OsStr::new("/mnt/cef-home")))
            .unwrap_or_else(|| default_user_home(user_home));
        let base = override_path(Some(OsStr::new("/opt/cef-base")))
            .unwrap_or_else(|| bundled_base_from_resource_dir(resource));

        assert_eq!(home, PathBuf::from("/mnt/cef-home"));
        assert_eq!(base, PathBuf::from("/opt/cef-base"));
        assert_ne!(home, default_user_home(user_home));
        assert_ne!(base, bundled_base_from_resource_dir(resource));
    }

    #[test]
    fn empty_env_overrides_fall_back_to_defaults() {
        let user_home = Path::new("/home/fernando");
        let resource = Path::new("/usr/lib/idioteque");

        let home =
            override_path(Some(OsStr::new(""))).unwrap_or_else(|| default_user_home(user_home));
        let base = override_path(Some(OsStr::new("")))
            .unwrap_or_else(|| bundled_base_from_resource_dir(resource));

        assert_eq!(home, PathBuf::from("/home/fernando/.idioteque/cef"));
        assert_eq!(base, PathBuf::from("/usr/lib/idioteque/cef/base"));
    }

    #[test]
    fn default_home_and_base_match_contract_linux_layouts() {
        assert_eq!(
            default_user_home(Path::new("/home/fernando")),
            PathBuf::from("/home/fernando/.idioteque/cef")
        );
        // deb / rpm
        assert_eq!(
            bundled_base_from_resource_dir(Path::new("/usr/lib/idioteque")),
            PathBuf::from("/usr/lib/idioteque/cef/base")
        );
        // AppImage: resource_dir = $APPDIR/usr/lib/idioteque (contrato 3.1).
        let appdir = Path::new("/tmp/.mount_idioteXXXX/usr/lib/idioteque");
        assert_eq!(
            bundled_base_from_resource_dir(appdir),
            PathBuf::from("/tmp/.mount_idioteXXXX/usr/lib/idioteque/cef/base")
        );
        // $APPDIR crudo no es resource_dir: no se concatena cef/base a APPDIR.
        let raw_appdir = Path::new("/tmp/.mount_idioteXXXX");
        assert_ne!(
            bundled_base_from_resource_dir(raw_appdir),
            PathBuf::from("/tmp/.mount_idioteXXXX/usr/lib/idioteque/cef/base")
        );
    }

    #[test]
    fn host_bin_override_requires_a_real_file() {
        let tmp = TempDir::new().expect("tmp");
        let missing = tmp.path().join("no-such-host");
        let error = resolve_host_binary(Some(missing.clone()), &[]).unwrap_err();
        assert!(
            error.contains(missing.to_string_lossy().as_ref()),
            "{error}"
        );

        let as_dir = tmp.path().join("host-dir");
        fs::create_dir_all(&as_dir).unwrap();
        let error = resolve_host_binary(Some(as_dir.clone()), &[]).unwrap_err();
        assert!(error.contains(as_dir.to_string_lossy().as_ref()), "{error}");

        let host = tmp.path().join("custom-cef-host");
        write_file(&host, b"#!/bin/true");
        let found = resolve_host_binary(Some(host.clone()), &[]).unwrap();
        assert_eq!(found, host);
    }

    #[test]
    fn empty_host_bin_override_falls_through_to_candidates() {
        let tmp = TempDir::new().expect("tmp");
        let exe = tmp.path().join("usr/bin/idioteque");
        let sidecar = tmp.path().join("usr/bin").join(host_binary_name());
        write_file(&exe, b"ade");
        write_file(&sidecar, b"host");

        let override_empty = override_path(Some(OsStr::new("")));
        assert!(override_empty.is_none());
        let found = resolve_host_binary(override_empty, &[exe]).unwrap();
        assert_eq!(found, sidecar);
    }

    #[test]
    fn host_bin_override_does_not_fall_back_when_missing() {
        let tmp = TempDir::new().expect("tmp");
        let exe = tmp.path().join("usr/bin/idioteque");
        let sidecar = tmp.path().join("usr/bin").join(host_binary_name());
        write_file(&exe, b"ade");
        write_file(&sidecar, b"host");

        let missing = tmp.path().join("missing-override");
        let error = resolve_host_binary(Some(missing), &[exe]).unwrap_err();
        assert!(error.contains("missing-override"), "{error}");
        assert!(!error.contains("ni"), "{error}");
    }

    #[test]
    fn appimage_prefers_current_exe_under_appdir_over_appimage_file() {
        // Contrato 3.1: current_exe = $APPDIR/usr/bin/idioteque.
        // No se lee la env APPDIR; el layout es el que produce el squash.
        let tmp = TempDir::new().expect("tmp");
        let appdir = tmp.path().join("squash");
        let current_exe = appdir.join("usr/bin/idioteque");
        let sidecar = appdir.join("usr/bin").join(host_binary_name());
        let appimage = tmp.path().join("Idioteque-x86_64.AppImage");
        write_file(&current_exe, b"ade");
        write_file(&sidecar, b"host");
        write_file(&appimage, b"outer");

        let found = resolve_host_binary(None, &[current_exe, appimage.clone()]).unwrap();
        assert_eq!(found, sidecar);

        // Si solo existiera el .AppImage, no hay sidecar junto a él.
        let error = resolve_host_binary(None, &[appimage.clone()]).unwrap_err();
        assert!(
            error.contains(&format!(
                "`{}`",
                appimage
                    .parent()
                    .unwrap()
                    .join(host_binary_name())
                    .display()
            )),
            "{error}"
        );
    }

    #[test]
    fn appimage_falls_back_to_current_binary_dir_when_squash_has_no_sidecar() {
        let tmp = TempDir::new().expect("tmp");
        let appdir = tmp.path().join("squash");
        let current_exe = appdir.join("usr/bin/idioteque");
        let appimage = tmp.path().join("Idioteque-x86_64.AppImage");
        let fallback = tmp.path().join(host_binary_name());
        write_file(&current_exe, b"ade");
        write_file(&appimage, b"outer");
        write_file(&fallback, b"host");

        let found = resolve_host_binary(None, &[current_exe, appimage]).unwrap();
        assert_eq!(found, fallback);
    }

    #[test]
    fn missing_sidecar_lists_every_candidate() {
        let tmp = TempDir::new().expect("tmp");
        let first = tmp.path().join("a/idioteque");
        let second = tmp.path().join("b/idioteque");
        write_file(&first, b"ade");
        write_file(&second, b"ade");

        let error = resolve_host_binary(None, &[first, second]).unwrap_err();
        assert!(
            error.contains("a/cef-host") || error.contains("a\\cef-host"),
            "{error}"
        );
        assert!(
            error.contains("b/cef-host") || error.contains("b\\cef-host"),
            "{error}"
        );
        assert!(error.contains(" ni "), "{error}");
    }

    #[test]
    fn no_exe_candidates_is_err() {
        let error = resolve_host_binary(None, &[]).unwrap_err();
        assert!(
            error.contains("No se pudo resolver el ejecutable"),
            "{error}"
        );
    }

    #[test]
    fn host_binary_name_is_platform_sidecar() {
        assert_eq!(host_binary_name(), "cef-host");
    }

    #[test]
    fn ensure_dirs_fails_when_home_is_a_file() {
        let tmp = TempDir::new().expect("tmp");
        let home = tmp.path().join("not-a-dir");
        write_file(&home, b"nope");
        let paths = CefPaths::new(home, tmp.path().join("base"));
        let error = paths.ensure_dirs().unwrap_err();
        assert!(error.contains("No se pudo crear"), "{error}");
    }

    #[test]
    fn ensure_dirs_fails_when_parent_is_not_writable() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let tmp = TempDir::new().expect("tmp");
            let locked = tmp.path().join("locked");
            fs::create_dir_all(&locked).unwrap();
            let mut perms = fs::metadata(&locked).unwrap().permissions();
            perms.set_mode(0o555);
            fs::set_permissions(&locked, perms).unwrap();
            let paths = CefPaths::new(locked.join("cef"), tmp.path().join("base"));
            let result = paths.ensure_dirs();
            let mut restore = fs::metadata(&locked).unwrap().permissions();
            restore.set_mode(0o755);
            let _ = fs::set_permissions(&locked, restore);
            if result.is_ok() {
                // root / CAP_DAC_OVERRIDE: 0555 no bloquea (lab, no producto).
                return;
            }
            assert!(result.unwrap_err().contains("No se pudo crear"));
        }
    }

    #[test]
    fn relative_override_is_kept_as_is() {
        let relative = OsString::from("./cef-home");
        assert_eq!(
            override_path(Some(relative.as_os_str())),
            Some(PathBuf::from("./cef-home"))
        );
    }
}
