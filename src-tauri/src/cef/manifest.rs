//! `manifest.json` de un slot CEF y resolución del motor efectivo (contrato 3.4 / 3.5).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::paths::{CefPaths, PLATFORM};
use super::version::CefVersion;

const MANIFEST_NAME: &str = "manifest.json";
const MANIFEST_TMP_NAME: &str = ".manifest.json.idioteque.tmp";

/// Archivos obligatorios en linux64 (falta alguno → slot inválido).
pub const REQUIRED_FILES_LINUX64: &[&str] = &[
    "libcef.so",
    "icudtl.dat",
    "v8_context_snapshot.bin",
    "resources.pak",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "locales/en-US.pak",
    "libEGL.so",
    "libGLESv2.so",
    "libvk_swiftshader.so",
    "libvulkan.so.1",
    "vk_swiftshader_icd.json",
    "chrome-sandbox",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotSource {
    Bundled,
    Downloaded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotManifest {
    pub schema: u32,
    pub cef_version: String,
    pub chromium_version: String,
    pub platform: String,
    pub api_version_min: u32,
    pub api_version_last: u32,
    pub source: SlotSource,
    pub archive_name: String,
    pub archive_sha1: String,
    pub archive_size: u64,
    pub stripped: bool,
    pub files: Vec<ManifestFile>,
    pub verified: bool,
    pub verified_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveSource {
    Bundled,
    Installed,
}

#[derive(Debug, Clone)]
pub struct EffectiveSlot {
    pub dir: PathBuf,
    pub manifest: SlotManifest,
    pub source: EffectiveSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
    pub cef_version: String,
    pub chromium_version: String,
    pub source: String,
    pub path: String,
    pub verified: bool,
}

pub fn load(slot_dir: &Path) -> Result<SlotManifest, String> {
    let path = slot_dir.join(MANIFEST_NAME);
    let bytes = fs::read(&path).map_err(|error| {
        format!("No se pudo leer `{}`: {error}", path.display())
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!("No se pudo parsear `{}`: {error}", path.display())
    })
}

pub fn save(slot_dir: &Path, manifest: &SlotManifest) -> Result<(), String> {
    fs::create_dir_all(slot_dir)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", slot_dir.display()))?;

    let json = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("No se pudo serializar el manifest: {error}"))?;

    let temporary = slot_dir.join(MANIFEST_TMP_NAME);
    let dest = slot_dir.join(MANIFEST_NAME);

    fs::write(&temporary, json)
        .map_err(|error| format!("No se pudo escribir el manifest: {error}"))?;

    fs::rename(&temporary, &dest).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo guardar el manifest: {error}")
    })
}

pub fn validate(slot_dir: &Path, manifest: &SlotManifest) -> Result<(), String> {
    if manifest.platform != PLATFORM {
        return Err(format!(
            "El slot `{}` es de `{}`, esta app usa `{PLATFORM}`",
            slot_dir.display(),
            manifest.platform
        ));
    }

    for name in required_files() {
        let path = slot_dir.join(name);
        if !path.is_file() {
            return Err(format!(
                "Falta el archivo obligatorio `{}` en `{}`",
                name,
                slot_dir.display()
            ));
        }
    }

    for file in &manifest.files {
        let path = slot_dir.join(&file.path);
        let meta = fs::metadata(&path).map_err(|error| {
            format!(
                "No se encontró `{}` listado en el manifest: {error}",
                path.display()
            )
        })?;
        if meta.len() != file.size {
            return Err(format!(
                "Tamaño incorrecto de `{}`: {} ≠ {}",
                path.display(),
                meta.len(),
                file.size
            ));
        }
    }

    Ok(())
}

/// Motor efectivo: instalado solo si es válido y `cefVersion` estrictamente mayor que el base.
pub fn resolve_effective(paths: &CefPaths) -> Result<EffectiveSlot, String> {
    let bundled = match load_valid(&paths.bundled_base) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Err(format!(
                "No se encontró el runtime CEF bundleado en `{}`: {error}",
                paths.bundled_base.display()
            ));
        }
    };

    let current_dir = paths.current();
    match load_valid(&current_dir) {
        Ok(installed) => {
            let bundled_ver = CefVersion::parse(&bundled.cef_version)?;
            let installed_ver = CefVersion::parse(&installed.cef_version)?;
            if installed_ver > bundled_ver {
                return Ok(EffectiveSlot {
                    dir: current_dir,
                    manifest: installed,
                    source: EffectiveSource::Installed,
                });
            }
            let _ = fs::remove_dir_all(&current_dir);
        }
        Err(_) => {
            if current_dir.exists() {
                let _ = fs::remove_dir_all(&current_dir);
            }
        }
    }

    Ok(EffectiveSlot {
        dir: paths.bundled_base.clone(),
        manifest: bundled,
        source: EffectiveSource::Bundled,
    })
}

pub fn slot_info(slot: &EffectiveSlot) -> SlotInfo {
    SlotInfo {
        cef_version: slot.manifest.cef_version.clone(),
        chromium_version: slot.manifest.chromium_version.clone(),
        source: match slot.source {
            EffectiveSource::Bundled => "bundled".to_string(),
            EffectiveSource::Installed => "installed".to_string(),
        },
        path: slot.dir.to_string_lossy().into_owned(),
        verified: slot.manifest.verified,
    }
}

fn load_valid(slot_dir: &Path) -> Result<SlotManifest, String> {
    let manifest = load(slot_dir)?;
    validate(slot_dir, &manifest)?;
    Ok(manifest)
}

fn required_files() -> &'static [&'static str] {
    match PLATFORM {
        "linux64" | "linuxarm64" => REQUIRED_FILES_LINUX64,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const BUNDLED: &str = "152.0.6+g708dc14+chromium-152.0.7977.83";
    const NEWER: &str = "153.0.1+gabc+chromium-153.0.8000.10";
    const OLDER: &str = "151.0.1+gold+chromium-151.0.1.1";

    fn sample_manifest(version: &str, source: SlotSource) -> SlotManifest {
        let chromium = super::super::version::chromium_from(version)
            .unwrap_or_else(|| "0.0.0.0".into());
        SlotManifest {
            schema: 1,
            cef_version: version.to_string(),
            chromium_version: chromium,
            platform: PLATFORM.to_string(),
            api_version_min: 13300,
            api_version_last: 15200,
            source,
            archive_name: "cef_binary_linux64_minimal.tar.bz2".into(),
            archive_sha1: "9711b86c105fb590da576fe5a829802f1a79d520".into(),
            archive_size: 321503907,
            stripped: true,
            files: Vec::new(),
            verified: false,
            verified_at: None,
            created_at: "2026-09-16T23:00:00Z".into(),
        }
    }

    fn write_required(dir: &Path, listed: bool, size: u64) -> Vec<ManifestFile> {
        let payload = vec![b'x'; size as usize];
        let mut files = Vec::new();
        for name in REQUIRED_FILES_LINUX64 {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, &payload).expect("write");
            if listed {
                files.push(ManifestFile {
                    path: (*name).to_string(),
                    size,
                    sha256: "ab".repeat(32),
                });
            }
        }
        files
    }

    fn write_slot(dir: &Path, version: &str, source: SlotSource, listed: bool) -> SlotManifest {
        fs::create_dir_all(dir).expect("slot");
        let mut manifest = sample_manifest(version, source);
        manifest.files = write_required(dir, listed, 1);
        save(dir, &manifest).expect("save");
        manifest
    }

    fn paths_in(tmp: &TempDir) -> CefPaths {
        CefPaths::new(tmp.path().join("home"), tmp.path().join("base"))
    }

    #[test]
    fn load_save_round_trip_camel_case() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let saved = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded, saved);
        let disk = fs::read_to_string(dir.join("manifest.json")).unwrap();
        assert!(disk.contains("\"cefVersion\""));
        assert!(disk.contains("\"source\": \"bundled\""));
        assert!(!dir.join(MANIFEST_TMP_NAME).exists());
    }

    #[test]
    fn bundled_only() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert_eq!(slot.dir, paths.bundled_base);
        let info = slot_info(&slot);
        assert_eq!(info.source, "bundled");
        assert_eq!(info.cef_version, BUNDLED);
        assert!(!paths.current().exists());
    }

    #[test]
    fn installed_newer_wins() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Installed);
        assert_eq!(slot.dir, paths.current());
        assert_eq!(slot.manifest.cef_version, NEWER);
        assert!(paths.current().exists());
        assert_eq!(slot_info(&slot).source, "installed");
    }

    #[test]
    fn installed_older_falls_back_and_deletes_current() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        write_slot(&paths.current(), OLDER, SlotSource::Downloaded, true);

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert_eq!(slot.dir, paths.bundled_base);
        assert!(!paths.current().exists());
    }

    #[test]
    fn installed_corrupt_missing_file_falls_back() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        fs::remove_file(paths.current().join("libcef.so")).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn installed_corrupt_wrong_size_falls_back() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        let mut manifest = write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        fs::write(paths.current().join("libcef.so"), b"too-big").unwrap();
        manifest.files.iter_mut().find(|f| f.path == "libcef.so").unwrap().size = 1;
        save(&paths.current(), &manifest).unwrap();

        assert!(validate(&paths.current(), &manifest).is_err());
        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn bundled_missing_is_err() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let error = resolve_effective(&paths).unwrap_err();
        assert!(error.contains("No se encontró el runtime CEF bundleado"));
    }

    #[test]
    fn validate_rejects_other_platform() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        manifest.platform = "windows64".into();
        assert!(validate(&dir, &manifest).is_err());
    }
}
