//! `state.json` del updater CEF y el comando `cef_runtime_info` (contratos 3.7 / 5).

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::denylist::{self, DenyEntry};
use super::manifest::{self, SlotInfo};
use super::paths::{CefPaths, PLATFORM};

const SCHEMA: u32 = 1;
const STATE_TMP_NAME: &str = ".state.json.idioteque.tmp";
static STATE_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

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
    // Un crash previo puede dejar el nombre fijo como directorio o symlink;
    // no se reutiliza: cada save usa un tmp único (pid + seq).
    remove_path_best_effort(&parent.join(tmp_name));
    let temporary = parent.join(format!(
        "{tmp_name}.{}.{}",
        std::process::id(),
        STATE_TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temporary, contents).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo escribir el estado del updater: {error}")
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo guardar el estado del updater: {error}")
    })
}

fn remove_path_best_effort(path: &Path) {
    let Ok(meta) = path.symlink_metadata() else {
        return;
    };
    if meta.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
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

    fn pending(cef: &str, chromium: &str) -> PendingPromotion {
        PendingPromotion {
            cef_version: cef.into(),
            chromium_version: chromium.into(),
        }
    }

    fn deferred_state() -> UpdaterState {
        UpdaterState {
            schema: 1,
            last_check_at: Some("2026-09-16T23:00:00Z".into()),
            index_etag: Some("\"72725877b82897a3019cd6aabf3b23a1\"".into()),
            pending_promotion: Some(pending(NEWER, "153.0.8000.10")),
            last_outcome: Some("deferred".into()),
        }
    }

    #[test]
    fn corrupt_file_is_default() {
        let cases: &[(&str, &[u8])] = &[
            ("empty", b""),
            ("whitespace", b" \n\t"),
            ("not-json", b"{not json"),
            ("truncated", b"{\"schema\":1,"),
            ("null", b"null"),
            ("array", b"[]"),
            ("bool", b"true"),
            ("schema-string", br#"{"schema":"1"}"#),
            ("schema-float", br#"{"schema":1.5}"#),
            (
                "pending-string",
                br#"{"schema":1,"pendingPromotion":"yes"}"#,
            ),
            (
                "pending-empty-obj",
                br#"{"schema":1,"pendingPromotion":{}}"#,
            ),
            ("pending-array", br#"{"schema":1,"pendingPromotion":[]}"#),
            (
                "pending-missing-chromium",
                br#"{"schema":1,"pendingPromotion":{"cefVersion":"153.0.1"}}"#,
            ),
            ("last-check-number", br#"{"schema":1,"lastCheckAt":1}"#),
            ("trailing-junk", br#"{"schema":1}{}"#),
            ("utf8-bom", b"\xef\xbb\xbf{\"schema\":1}"),
            ("invalid-utf8", &[0xff, 0xfe, 0x00, 0x7b]),
            ("json-comment", br#"{/*x*/"schema":1}"#),
        ];
        for (name, bytes) in cases {
            let tmp = TempDir::new().unwrap();
            let paths = paths_in(&tmp);
            fs::create_dir_all(&paths.home).unwrap();
            fs::write(paths.state_file(), bytes).unwrap();
            assert_eq!(load(&paths), default_state(), "corrupt case {name}");
        }
    }

    #[test]
    fn state_json_directory_is_default() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(paths.state_file()).unwrap();
        assert_eq!(load(&paths), default_state());
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
    fn pending_promotion_survives_save_of_other_fields() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let original = deferred_state();
        save(&paths, &original).expect("save");

        let mut loaded = load(&paths);
        assert_eq!(loaded.pending_promotion, original.pending_promotion);
        loaded.last_check_at = Some("2026-09-17T08:00:00Z".into());
        loaded.last_outcome = Some("no-newer".into());
        loaded.index_etag = Some("\"etag-after-cycle\"".into());
        save(&paths, &loaded).expect("save after finish()");

        let again = load(&paths);
        assert_eq!(
            again.pending_promotion, original.pending_promotion,
            "finish()/save no debe borrar pendingPromotion si el caller lo dejó"
        );
        assert_eq!(again.last_check_at.as_deref(), Some("2026-09-17T08:00:00Z"));
        assert_eq!(again.last_outcome.as_deref(), Some("no-newer"));
        assert_eq!(again.index_etag.as_deref(), Some("\"etag-after-cycle\""));

        let disk = fs::read_to_string(paths.state_file()).unwrap();
        assert!(disk.contains("\"pendingPromotion\""));
        assert!(disk.contains("\"cefVersion\""));
        assert!(disk.contains(NEWER));
        assert!(disk.contains("153.0.8000.10"));
    }

    #[test]
    fn explicit_clear_writes_null_pending_promotion() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        save(&paths, &deferred_state()).expect("save");

        let mut loaded = load(&paths);
        loaded.pending_promotion = None;
        loaded.last_outcome = Some("updated".into());
        save(&paths, &loaded).expect("clear");

        let again = load(&paths);
        assert_eq!(again.pending_promotion, None);
        assert_eq!(again.last_outcome.as_deref(), Some("updated"));
        let disk = fs::read_to_string(paths.state_file()).unwrap();
        assert!(disk.contains("\"pendingPromotion\": null"));
    }

    #[test]
    fn extra_fields_and_schema_zero_keep_pending() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(
            paths.state_file(),
            r#"{
  "schema": 0,
  "lastCheckAt": "2026-09-16T23:00:00Z",
  "pendingPromotion": {
    "cefVersion": "153.0.1+gabc+chromium-153.0.8000.10",
    "chromiumVersion": "153.0.8000.10",
    "ignored": true
  },
  "futureField": 1
}"#,
        )
        .unwrap();

        let state = load(&paths);
        assert_eq!(state.schema, 1);
        assert_eq!(
            state.pending_promotion,
            Some(pending(NEWER, "153.0.8000.10"))
        );
        assert_eq!(state.last_check_at.as_deref(), Some("2026-09-16T23:00:00Z"));
    }

    #[test]
    fn crlf_and_null_pending_are_valid() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(
            paths.state_file(),
            b"{\r\n  \"schema\": 1,\r\n  \"pendingPromotion\": null,\r\n  \"lastOutcome\": \"no-newer\"\r\n}\r\n",
        )
        .unwrap();
        let state = load(&paths);
        assert_eq!(state.schema, 1);
        assert_eq!(state.pending_promotion, None);
        assert_eq!(state.last_outcome.as_deref(), Some("no-newer"));
    }

    #[test]
    fn snake_case_pending_key_is_not_pending() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(
            paths.state_file(),
            r#"{"schema":1,"pending_promotion":{"cefVersion":"x","chromiumVersion":"y"}}"#,
        )
        .unwrap();
        let state = load(&paths);
        assert_eq!(state.schema, 1);
        assert_eq!(
            state.pending_promotion, None,
            "el contrato 3.7 es camelCase; snake_case no cuenta como pending"
        );
    }

    #[test]
    fn leftover_state_tmp_directory_does_not_block_save() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        let stale = paths.home.join(STATE_TMP_NAME);
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("stuck"), b"x").unwrap();

        let state = deferred_state();
        save(&paths, &state).expect("un .tmp huérfano (dir) no debe impedir save");
        assert_eq!(load(&paths).pending_promotion, state.pending_promotion);
        assert_eq!(load(&paths), state);
        assert!(
            !stale.exists(),
            "save debe limpiar el tmp fijo huérfano antes de persistir"
        );
    }

    #[test]
    fn leftover_state_tmp_is_ignored_on_load() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let state = deferred_state();
        save(&paths, &state).expect("save");
        fs::write(
            paths.home.join(STATE_TMP_NAME),
            br#"{"schema":1,"pendingPromotion":null,"lastOutcome":"torn"}"#,
        )
        .unwrap();
        assert_eq!(load(&paths), state);
    }

    #[test]
    fn concurrent_saves_do_not_tear_pending_promotion() {
        let tmp = TempDir::new().unwrap();
        let paths = std::sync::Arc::new(paths_in(&tmp));
        let expected = pending(NEWER, "153.0.8000.10");
        let mut threads = Vec::new();
        for i in 0..8 {
            let paths = std::sync::Arc::clone(&paths);
            let expected = expected.clone();
            threads.push(std::thread::spawn(move || {
                for j in 0..8 {
                    let state = UpdaterState {
                        schema: 1,
                        last_check_at: Some(format!("2026-09-16T23:{:02}:{:02}Z", i, j)),
                        index_etag: Some(format!("\"etag-{i}-{j}\"")),
                        pending_promotion: Some(expected.clone()),
                        last_outcome: Some("deferred".into()),
                    };
                    save(&paths, &state).expect("save");
                }
            }));
        }
        for thread in threads {
            thread.join().expect("thread");
        }
        let disk = fs::read_to_string(paths.state_file()).expect("state.json");
        let parsed: UpdaterState =
            serde_json::from_str(&disk).expect("state.json no debe quedar a medias");
        assert_eq!(parsed.pending_promotion.as_ref(), Some(&expected));
        assert_eq!(load(&paths).pending_promotion.as_ref(), Some(&expected));
    }

    #[test]
    fn remove_health_caches_only_drops_matching_dirs() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        paths.ensure_dirs().unwrap();

        let cache_a = paths.home.join("health-cache-1");
        let cache_b = paths.home.join("health-cache-123");
        let cache_zero = paths.home.join("health-cache-0");
        let nested = cache_a.join("nested").join("deep");
        let other_dir = paths.home.join("not-a-cache");
        let cache_lookalike = paths.home.join("health-cache");
        let cache_suffix = paths.home.join("health-caches");
        let prefix_inside = paths.home.join("xx-health-cache-1");
        let other_file = paths.home.join("health-cache-note.txt");
        let cache_as_file = paths.home.join("health-cache-1.file");
        let cache_file_exact = paths.home.join("health-cache-999");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("x"), b"1").unwrap();
        fs::create_dir_all(&cache_b).unwrap();
        fs::create_dir_all(&cache_zero).unwrap();
        fs::create_dir_all(&other_dir).unwrap();
        fs::create_dir_all(&cache_lookalike).unwrap();
        fs::create_dir_all(&cache_suffix).unwrap();
        fs::create_dir_all(&prefix_inside).unwrap();
        fs::write(&other_file, b"keep").unwrap();
        fs::write(&cache_as_file, b"not-a-dir").unwrap();
        fs::write(&cache_file_exact, b"leftover-file").unwrap();
        fs::write(paths.home.join("state.json"), b"{}").unwrap();
        fs::write(paths.home.join("denylist.json"), b"{}").unwrap();

        let current_cache = paths.current().join("health-cache-1");
        fs::create_dir_all(&current_cache).unwrap();
        fs::write(current_cache.join("slot"), b"keep").unwrap();
        let logs_cache = paths.logs_dir().join("health-cache-1");
        fs::create_dir_all(&logs_cache).unwrap();
        fs::create_dir_all(paths.candidate()).unwrap();
        fs::create_dir_all(paths.current_old()).unwrap();

        remove_health_caches(&paths);

        assert!(!cache_a.exists());
        assert!(!cache_b.exists());
        assert!(!cache_zero.exists());
        assert!(other_dir.is_dir());
        assert!(cache_lookalike.is_dir());
        assert!(cache_suffix.is_dir());
        assert!(prefix_inside.is_dir());
        assert!(other_file.is_file());
        assert!(cache_as_file.is_file());
        assert!(
            cache_file_exact.is_file(),
            "solo se borran directorios health-cache-*, no ficheros"
        );
        assert!(paths.profile().is_dir());
        assert!(paths.state_file().is_file());
        assert!(paths.home.join("denylist.json").is_file());
        assert!(
            current_cache.join("slot").is_file(),
            "no recursar dentro de current/"
        );
        assert!(logs_cache.is_dir(), "no recursar dentro de logs/");
        assert!(paths.candidate().is_dir());
        assert!(paths.current_old().is_dir());
    }

    #[test]
    fn remove_health_caches_missing_home_is_a_noop() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        remove_health_caches(&paths);
        assert!(!paths.home.exists());
    }

    #[test]
    fn remove_health_caches_home_file_is_a_noop() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::write(&paths.home, b"not-a-dir").unwrap();
        remove_health_caches(&paths);
        assert!(paths.home.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn remove_health_caches_does_not_follow_symlink_into_profile() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        paths.ensure_dirs().unwrap();
        let marker = paths.profile().join("keep-me");
        fs::write(&marker, b"safe").unwrap();
        let link = paths.home.join("health-cache-symlink");
        std::os::unix::fs::symlink(&paths.profile(), &link).unwrap();

        remove_health_caches(&paths);

        assert!(
            marker.is_file(),
            "no debe borrar profile a través de un symlink health-cache-*"
        );
        assert!(
            !link.exists(),
            "el leftover health-cache-* (symlink) sí se limpia"
        );
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

    #[test]
    fn runtime_info_corrupt_state_does_not_drop_slots() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(paths.state_file(), b"{not json").unwrap();

        let info = runtime_info(&paths, 15200, false).expect("info");
        assert_eq!(info.current.cef_version, BUNDLED);
        assert_eq!(info.pending_promotion, None);
        assert_eq!(info.last_check_at, None);
    }
}
