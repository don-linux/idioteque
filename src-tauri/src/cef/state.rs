//! `state.json` del updater CEF y el comando `cef_runtime_info` (contratos 3.7 / 5).

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::denylist::{self, DenyEntry};
use super::manifest::{self, SlotInfo};
use super::paths::{CefPaths, PLATFORM};

const SCHEMA: u32 = 1;
const STATE_TMP_NAME: &str = ".state.json.idioteque.tmp";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingPromotion {
    pub cef_version: String,
    pub chromium_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterState {
    #[serde(default)]
    pub schema: u32,
    pub last_check_at: Option<String>,
    pub index_etag: Option<String>,
    pub pending_promotion: Option<PendingPromotion>,
    pub last_outcome: Option<String>,
}

pub fn load(paths: &CefPaths) -> UpdaterState {
    match fs::read(paths.state_file()) {
        Ok(bytes) => parse_state(&bytes),
        Err(_) => default_state(),
    }
}

pub fn save(paths: &CefPaths, state: &UpdaterState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state)
        .map_err(|error| format!("No se pudo serializar el estado del updater: {error}"))?;
    atomic_write(&paths.state_file(), &json, STATE_TMP_NAME)
}

pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
}

/// Borra cada directorio `health-cache-*` bajo `paths.home`.
pub fn remove_health_caches(paths: &CefPaths) {
    let Ok(entries) = fs::read_dir(&paths.home) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("health-cache-") {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CefRuntimeInfo {
    pub current: SlotInfo,
    pub base: SlotInfo,
    pub candidate: Option<SlotInfo>,
    pub denylist: Vec<DenyEntry>,
    pub last_check_at: Option<String>,
    pub pending_promotion: Option<PendingPromotion>,
    pub host_api_version: u32,
    pub platform: String,
    pub host_alive: bool,
}

/// Constructor puro de `CefRuntimeInfo` (el comando Tauri delega aquí).
pub fn runtime_info(
    paths: &CefPaths,
    host_api_version: u32,
    host_alive: bool,
) -> Result<CefRuntimeInfo, String> {
    let effective = manifest::resolve_effective(paths)?;
    let current = manifest::slot_info(&effective);
    let base = bundled_slot_info(paths)?;
    let candidate = match manifest::load(&paths.candidate()) {
        Ok(manifest) => Some(slot_info_from_manifest(&paths.candidate(), &manifest)),
        Err(_) => None,
    };
    let denylist = denylist::load(paths, host_api_version);
    let state = load(paths);

    Ok(CefRuntimeInfo {
        current,
        base,
        candidate,
        denylist: denylist.entries,
        last_check_at: state.last_check_at,
        pending_promotion: state.pending_promotion,
        host_api_version,
        platform: PLATFORM.to_string(),
        host_alive,
    })
}

#[tauri::command]
pub fn cef_runtime_info(
    app: tauri::AppHandle,
    state: tauri::State<crate::cef::host::CefState>,
) -> Result<CefRuntimeInfo, String> {
    let paths = CefPaths::from_app(&app)?;
    let host_api_version = super::paths::base_info().host_api_version;
    runtime_info(&paths, host_api_version, state.host_alive())
}

fn bundled_slot_info(paths: &CefPaths) -> Result<SlotInfo, String> {
    let manifest = manifest::load(&paths.bundled_base).map_err(|error| {
        format!(
            "No se encontró el runtime CEF bundleado en `{}`: {error}",
            paths.bundled_base.display()
        )
    })?;
    Ok(slot_info_from_manifest(&paths.bundled_base, &manifest))
}

fn slot_info_from_manifest(dir: &Path, manifest: &manifest::SlotManifest) -> SlotInfo {
    SlotInfo {
        cef_version: manifest.cef_version.clone(),
        chromium_version: manifest.chromium_version.clone(),
        source: match manifest.source {
            manifest::SlotSource::Bundled => "bundled".to_string(),
            manifest::SlotSource::Downloaded => "downloaded".to_string(),
        },
        path: dir.to_string_lossy().into_owned(),
        verified: manifest.verified,
    }
}

fn default_state() -> UpdaterState {
    UpdaterState {
        schema: SCHEMA,
        ..UpdaterState::default()
    }
}

fn parse_state(bytes: &[u8]) -> UpdaterState {
    match serde_json::from_slice::<UpdaterState>(bytes) {
        Ok(mut state) => {
            if state.schema == 0 {
                state.schema = SCHEMA;
            }
            state
        }
        Err(_) => default_state(),
    }
}

fn atomic_write(path: &Path, contents: &str, tmp_name: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Ruta de estado inválida".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;
    let temporary = parent.join(tmp_name);
    fs::write(&temporary, contents)
        .map_err(|error| format!("No se pudo escribir el estado del updater: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo guardar el estado del updater: {error}")
    })
}

/// Civil date from Unix seconds, UTC. Howard Hinnant's `civil_from_days`.
fn format_unix_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cef::manifest::{
        save as save_manifest, ManifestFile, SlotManifest, SlotSource, REQUIRED_FILES_LINUX64,
    };
    use crate::cef::paths::PLATFORM;
    use crate::cef::version::chromium_from;
    use tempfile::TempDir;

    const BUNDLED: &str = "152.0.6+g708dc14+chromium-152.0.7977.83";
    const NEWER: &str = "153.0.1+gabc+chromium-153.0.8000.10";

    fn paths_in(tmp: &TempDir) -> CefPaths {
        CefPaths::new(tmp.path().join("home"), tmp.path().join("base"))
    }

    fn sample_manifest(version: &str, source: SlotSource) -> SlotManifest {
        let chromium = chromium_from(version).unwrap_or_else(|| "0.0.0.0".into());
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

    fn write_required(dir: &Path, size: u64) -> Vec<ManifestFile> {
        let payload = vec![b'x'; size as usize];
        let mut files = Vec::new();
        for name in REQUIRED_FILES_LINUX64 {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, &payload).expect("write");
            files.push(ManifestFile {
                path: (*name).to_string(),
                size,
                sha256: "ab".repeat(32),
            });
        }
        files
    }

    fn write_slot(dir: &Path, version: &str, source: SlotSource) -> SlotManifest {
        fs::create_dir_all(dir).expect("slot");
        let mut manifest = sample_manifest(version, source);
        manifest.files = write_required(dir, 1);
        save_manifest(dir, &manifest).expect("save");
        manifest
    }

    #[test]
    fn now_rfc3339_is_utc_zulu() {
        let stamp = now_rfc3339();
        assert!(stamp.ends_with('Z'), "{stamp}");
        assert_eq!(stamp.len(), "2026-09-16T23:00:00Z".len());
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn missing_file_is_default() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let state = load(&paths);
        assert_eq!(state.schema, 1);
        assert_eq!(state.last_check_at, None);
        assert_eq!(state.index_etag, None);
        assert_eq!(state.pending_promotion, None);
        assert_eq!(state.last_outcome, None);
    }

    #[test]
    fn corrupt_file_is_default() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(paths.state_file(), b"{not json").unwrap();
        let state = load(&paths);
        assert_eq!(state, default_state());
    }

    #[test]
    fn round_trip_camel_case() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let state = UpdaterState {
            schema: 1,
            last_check_at: Some("2026-09-16T23:00:00Z".into()),
            index_etag: Some("\"72725877b82897a3019cd6aabf3b23a1\"".into()),
            pending_promotion: Some(PendingPromotion {
                cef_version: NEWER.into(),
                chromium_version: "153.0.8000.10".into(),
            }),
            last_outcome: Some("no-newer".into()),
        };
        save(&paths, &state).expect("save");

        let disk = fs::read_to_string(paths.state_file()).unwrap();
        assert!(disk.contains("\"lastCheckAt\""));
        assert!(disk.contains("\"indexEtag\""));
        assert!(disk.contains("\"pendingPromotion\""));
        assert!(disk.contains("\"cefVersion\""));
        assert!(!paths.home.join(STATE_TMP_NAME).exists());

        assert_eq!(load(&paths), state);
    }

    #[test]
    fn remove_health_caches_only_drops_matching_dirs() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        paths.ensure_dirs().unwrap();

        let cache_a = paths.home.join("health-cache-1");
        let cache_b = paths.home.join("health-cache-123");
        let other_dir = paths.home.join("not-a-cache");
        let cache_lookalike = paths.home.join("health-cache");
        let cache_suffix = paths.home.join("health-caches");
        let other_file = paths.home.join("health-cache-note.txt");
        fs::create_dir_all(&cache_a).unwrap();
        fs::create_dir_all(&cache_b).unwrap();
        fs::create_dir_all(&other_dir).unwrap();
        fs::create_dir_all(&cache_lookalike).unwrap();
        fs::create_dir_all(&cache_suffix).unwrap();
        fs::write(cache_a.join("x"), b"1").unwrap();
        fs::write(&other_file, b"keep").unwrap();
        fs::write(paths.home.join("state.json"), b"{}").unwrap();

        remove_health_caches(&paths);

        assert!(!cache_a.exists());
        assert!(!cache_b.exists());
        assert!(other_dir.is_dir());
        assert!(cache_lookalike.is_dir());
        assert!(cache_suffix.is_dir());
        assert!(other_file.is_file());
        assert!(paths.profile().is_dir());
        assert!(paths.state_file().is_file());
    }

    #[test]
    fn runtime_info_bundled_only() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled);

        let info = runtime_info(&paths, 15200, false).expect("info");
        assert_eq!(info.current.cef_version, BUNDLED);
        assert_eq!(info.current.source, "bundled");
        assert_eq!(info.base.cef_version, BUNDLED);
        assert_eq!(info.base.source, "bundled");
        assert_eq!(info.base.path, paths.bundled_base.to_string_lossy());
        assert_eq!(info.candidate, None);
        assert!(info.denylist.is_empty());
        assert_eq!(info.last_check_at, None);
        assert_eq!(info.pending_promotion, None);
        assert_eq!(info.host_api_version, 15200);
        assert_eq!(info.platform, PLATFORM);
        assert!(!info.host_alive);
    }

    #[test]
    fn runtime_info_installed_keeps_bundled_as_base() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        paths.ensure_dirs().unwrap();
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled);
        write_slot(&paths.current(), NEWER, SlotSource::Downloaded);

        let mut candidate = write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded);
        candidate.verified = true;
        candidate.verified_at = Some("2026-09-16T23:00:00Z".into());
        save_manifest(&paths.candidate(), &candidate).unwrap();

        let mut denylist = denylist::Denylist::new(15200);
        denylist.add(NEWER, "153.0.8000.10", "health-exit-10");
        denylist::save(&paths, &denylist).unwrap();

        let state = UpdaterState {
            schema: 1,
            last_check_at: Some("2026-09-16T23:00:00Z".into()),
            index_etag: None,
            pending_promotion: Some(PendingPromotion {
                cef_version: NEWER.into(),
                chromium_version: "153.0.8000.10".into(),
            }),
            last_outcome: Some("pending".into()),
        };
        save(&paths, &state).unwrap();

        let info = runtime_info(&paths, 15200, true).expect("info");
        assert_eq!(info.current.source, "installed");
        assert_eq!(info.current.cef_version, NEWER);
        assert_eq!(info.current.path, paths.current().to_string_lossy());
        assert_eq!(info.base.source, "bundled");
        assert_eq!(info.base.cef_version, BUNDLED);
        assert_eq!(info.base.path, paths.bundled_base.to_string_lossy());
        let candidate_info = info.candidate.expect("candidate");
        assert_eq!(candidate_info.cef_version, NEWER);
        assert_eq!(candidate_info.source, "downloaded");
        assert!(candidate_info.verified);
        assert_eq!(info.denylist.len(), 1);
        assert_eq!(info.last_check_at.as_deref(), Some("2026-09-16T23:00:00Z"));
        assert_eq!(
            info.pending_promotion
                .as_ref()
                .map(|p| p.cef_version.as_str()),
            Some(NEWER)
        );
        assert!(info.host_alive);
    }
}
