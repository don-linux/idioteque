//! `manifest.json` de un slot CEF y resolución del motor efectivo (contrato 3.4 / 3.5).

use std::fs;
use std::path::{Component, Path, PathBuf};

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
    let bytes = fs::read(&path)
        .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("No se pudo parsear `{}`: {error}", path.display()))
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

    // Contrato 3.5: validez en runtime = parse + plataforma + archivos
    // obligatorios con el tamaño del manifest. El sha256 se calcula al extraer
    // (archive.rs) y no se rehashea aquí: hashear libcef (~268 MB) en cada
    // spawn tumbaría el arranque en deb/rpm/AppImage.
    for file in &manifest.files {
        let path = slot_file_path(slot_dir, &file.path)?;
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
    match current_kind(&current_dir) {
        CurrentKind::Directory => match load_valid(&current_dir) {
            Ok(installed) => {
                let bundled_ver = CefVersion::parse(&bundled.cef_version)?;
                match CefVersion::parse(&installed.cef_version) {
                    Ok(installed_ver) if installed_ver > bundled_ver => {
                        return Ok(EffectiveSlot {
                            dir: current_dir,
                            manifest: installed,
                            source: EffectiveSource::Installed,
                        });
                    }
                    _ => remove_stale(&current_dir),
                }
            }
            Err(_) => remove_stale(&current_dir),
        },
        CurrentKind::StaleEntry => remove_stale(&current_dir),
        CurrentKind::Absent => {}
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

/// Ruta listada en `files`: relativa, sin `..` ni absoluta (no escapar el slot).
fn slot_file_path(slot_dir: &Path, listed: &str) -> Result<PathBuf, String> {
    let rel = Path::new(listed);
    if listed.is_empty() || rel.is_absolute() {
        return Err(format!("Ruta de manifest inválida: `{listed}`"));
    }
    for component in rel.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(format!("Ruta de manifest fuera del slot: `{listed}`"));
            }
        }
    }
    Ok(slot_dir.join(rel))
}

enum CurrentKind {
    Directory,
    StaleEntry,
    Absent,
}

fn current_kind(path: &Path) -> CurrentKind {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => CurrentKind::Directory,
        Ok(_) => CurrentKind::StaleEntry,
        Err(_) => CurrentKind::Absent,
    }
}

/// Borra un `current` inválido sin seguir symlinks (un `current` → base
/// no debe `remove_dir_all` el runtime bundleado).
fn remove_stale(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    let _ = if meta.file_type().is_symlink() || meta.file_type().is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    };
}

fn required_files() -> &'static [&'static str] {
    match PLATFORM {
        "linux64" => REQUIRED_FILES_LINUX64,
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
        let chromium =
            super::super::version::chromium_from(version).unwrap_or_else(|| "0.0.0.0".into());
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
    fn installed_same_version_falls_back_and_deletes_current() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, true);

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
        manifest
            .files
            .iter_mut()
            .find(|f| f.path == "libcef.so")
            .unwrap()
            .size = 1;
        save(&paths.current(), &manifest).unwrap();

        assert!(validate(&paths.current(), &manifest).is_err());
        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn validate_rejects_missing_required_file_even_if_files_list_is_empty() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, false);
        fs::remove_file(dir.join("libcef.so")).unwrap();
        assert!(manifest.files.is_empty());
        let error = validate(&dir, &manifest).unwrap_err();
        assert!(error.contains("libcef.so"), "{error}");
    }

    #[test]
    fn installed_newer_with_empty_files_list_still_requires_mandatory_files() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        write_slot(&paths.current(), NEWER, SlotSource::Downloaded, false);
        fs::remove_file(paths.current().join("chrome-sandbox")).unwrap();

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

    #[test]
    fn validate_ignores_sha256_mismatch_when_size_matches() {
        // Contrato 3.5: el runtime no rehashea. sha256 se fija en extract.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        for file in &mut manifest.files {
            file.sha256 = "00".repeat(32);
        }
        assert!(validate(&dir, &manifest).is_ok());
        manifest.files[0].sha256 = "not-a-hex-digest".into();
        assert!(validate(&dir, &manifest).is_ok());
        manifest.files[0].sha256.clear();
        assert!(validate(&dir, &manifest).is_ok());
    }

    #[test]
    fn validate_rejects_listed_file_wrong_size_even_if_required_ok() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        fs::write(dir.join("LICENSE.txt"), b"abcd").unwrap();
        manifest.files.push(ManifestFile {
            path: "LICENSE.txt".into(),
            size: 1,
            sha256: "ab".repeat(32),
        });
        let error = validate(&dir, &manifest).unwrap_err();
        assert!(error.contains("Tamaño incorrecto"), "{error}");
        assert!(error.contains("LICENSE.txt"), "{error}");
    }

    #[test]
    fn validate_rejects_missing_listed_extra_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        manifest.files.push(ManifestFile {
            path: "LICENSE.txt".into(),
            size: 4,
            sha256: "ab".repeat(32),
        });
        let error = validate(&dir, &manifest).unwrap_err();
        assert!(error.contains("LICENSE.txt"), "{error}");
    }

    #[test]
    fn validate_rejects_path_escape_in_files_list() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        fs::write(tmp.path().join("outside.bin"), b"xx").unwrap();
        for listed in ["../outside.bin", "/etc/passwd", "", "foo/../../outside.bin"] {
            manifest.files.push(ManifestFile {
                path: listed.into(),
                size: 2,
                sha256: "ab".repeat(32),
            });
            let error = validate(&dir, &manifest).unwrap_err();
            assert!(
                error.contains("inválida") || error.contains("fuera del slot"),
                "{listed}: {error}"
            );
            manifest.files.pop();
        }
    }

    #[test]
    fn validate_accepts_nested_relative_listed_path() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        let nested = dir.join("locales/es.pak");
        fs::write(&nested, b"z").unwrap();
        manifest.files.push(ManifestFile {
            path: "locales/es.pak".into(),
            size: 1,
            sha256: "ab".repeat(32),
        });
        assert!(validate(&dir, &manifest).is_ok());
    }

    #[test]
    fn validate_rejects_required_file_that_is_a_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        fs::remove_file(dir.join("libcef.so")).unwrap();
        fs::create_dir(dir.join("libcef.so")).unwrap();
        let error = validate(&dir, &manifest).unwrap_err();
        assert!(error.contains("libcef.so"), "{error}");
    }

    #[test]
    fn validate_duplicate_listed_paths_second_size_wins_as_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut manifest = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        manifest.files.push(ManifestFile {
            path: "libcef.so".into(),
            size: 99,
            sha256: "ab".repeat(32),
        });
        assert!(validate(&dir, &manifest).is_err());
    }

    #[test]
    fn stale_current_wrong_platform_is_deleted() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        let mut manifest = write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        manifest.platform = "windows64".into();
        save(&paths.current(), &manifest).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn stale_current_unparsable_version_falls_back_and_deletes() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        let mut manifest = write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        manifest.cef_version = "not-a-version".into();
        save(&paths.current(), &manifest).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn stale_current_corrupt_json_falls_back_and_deletes() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        fs::create_dir_all(paths.current()).unwrap();
        fs::write(paths.current().join("manifest.json"), b"{not json").unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn stale_current_empty_dir_is_deleted() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        fs::create_dir_all(paths.current()).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn stale_current_file_is_removed_not_followed() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(paths.current(), b"i-am-a-file").unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
        assert!(paths.bundled_base.join("libcef.so").is_file());
    }

    #[test]
    #[cfg(unix)]
    fn stale_current_symlink_to_bundled_does_not_delete_base() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        fs::create_dir_all(&paths.home).unwrap();
        std::os::unix::fs::symlink(&paths.bundled_base, paths.current()).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
        assert!(paths.bundled_base.join("manifest.json").is_file());
        assert!(paths.bundled_base.join("libcef.so").is_file());
    }

    #[test]
    fn candidate_and_current_old_are_ignored() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        write_slot(&paths.current_old(), NEWER, SlotSource::Downloaded, true);

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
        assert!(paths.candidate().exists());
        assert!(paths.current_old().exists());
    }

    #[test]
    fn leftover_manifest_tmp_does_not_count_as_manifest() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        fs::create_dir_all(paths.current()).unwrap();
        let good = sample_manifest(NEWER, SlotSource::Downloaded);
        fs::write(
            paths.current().join(MANIFEST_TMP_NAME),
            serde_json::to_string(&good).unwrap(),
        )
        .unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }

    #[test]
    fn load_tolerates_unknown_fields() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
        value["totallyUnknown"] = serde_json::json!(true);
        fs::write(dir.join("manifest.json"), value.to_string()).unwrap();
        assert_eq!(load(&dir).unwrap().cef_version, BUNDLED);
    }

    #[test]
    fn load_rejects_partial_object() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.json"), br#"{"schema":1}"#).unwrap();
        assert!(load(&dir).is_err());
    }

    #[test]
    fn installed_unverified_newer_still_wins() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        let mut manifest = write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        manifest.verified = false;
        save(&paths.current(), &manifest).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Installed);
        assert!(!slot.manifest.verified);
        let info = slot_info(&slot);
        assert!(!info.verified);
        assert_eq!(info.path, paths.current().to_string_lossy());
    }

    #[test]
    fn save_overwrites_and_leaves_no_tmp() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("slot");
        let mut first = write_slot(&dir, BUNDLED, SlotSource::Bundled, true);
        first.verified = true;
        first.verified_at = Some("2026-09-17T00:00:00Z".into());
        save(&dir, &first).unwrap();
        let loaded = load(&dir).unwrap();
        assert!(loaded.verified);
        assert_eq!(loaded.verified_at.as_deref(), Some("2026-09-17T00:00:00Z"));
        assert!(!dir.join(MANIFEST_TMP_NAME).exists());
    }

    #[test]
    fn installed_newer_sha_mismatch_still_used_when_sizes_match() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        let mut manifest = write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        for file in &mut manifest.files {
            file.sha256 = "ff".repeat(32);
        }
        save(&paths.current(), &manifest).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Installed);
        assert!(paths.current().exists());
    }

    #[test]
    fn installed_newer_with_escaped_listed_path_is_stale() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, true);
        let mut manifest = write_slot(&paths.current(), NEWER, SlotSource::Downloaded, true);
        manifest.files.push(ManifestFile {
            path: "../secret".into(),
            size: 1,
            sha256: "ab".repeat(32),
        });
        save(&paths.current(), &manifest).unwrap();

        let slot = resolve_effective(&paths).unwrap();
        assert_eq!(slot.source, EffectiveSource::Bundled);
        assert!(!paths.current().exists());
    }
}
