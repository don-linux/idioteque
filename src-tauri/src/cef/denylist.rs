//! `denylist.json` de versiones CEF incompatibles (contrato 3.6).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::paths::CefPaths;

const SCHEMA: u32 = 1;
/// Nombre legado de un único tmp compartido. Ya no se escribe; `load` no lo lee.
#[cfg(test)]
const DENYLIST_TMP_NAME: &str = ".denylist.json.idioteque.tmp";
static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// Contrato 3.6: solo estos motivos se persisten.
///
/// `health-spawn` no está en la lista del contrato, pero el updater ya lo usa
/// vía `HealthFailure::Spawn` — se conserva: quitarlo rompería el denylist
/// de un candidate que ni llega a arrancar.
///
/// Nunca: descarga corrupta, hash malo, falta de disco o fallo de strip.
pub fn is_persistable_reason(reason: &str) -> bool {
    match reason {
        "api-version-min-above-host"
        | "health-timeout"
        | "health-crashed"
        | "health-no-handshake"
        | "health-spawn" => true,
        other => other
            .strip_prefix("health-exit-")
            .is_some_and(|code| !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit())),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DenyEntry {
    pub cef_version: String,
    pub chromium_version: String,
    pub reason: String,
    pub at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Denylist {
    pub schema: u32,
    pub host_api_version: u32,
    #[serde(default)]
    pub entries: Vec<DenyEntry>,
}

impl Denylist {
    pub fn new(host_api_version: u32) -> Self {
        Self {
            schema: SCHEMA,
            host_api_version,
            entries: Vec::new(),
        }
    }

    pub fn contains(&self, cef_version: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.cef_version == cef_version)
    }

    /// Añade o reemplaza la entrada de `cef_version` y pone `at` a ahora.
    ///
    /// Devuelve `false` y no toca la lista si `reason` no es persistible
    /// (contrato 3.6: descarga corrupta / hash / disco / strip nunca denylistan).
    pub fn add(&mut self, cef_version: &str, chromium_version: &str, reason: &str) -> bool {
        if !is_persistable_reason(reason) {
            return false;
        }
        let at = now_rfc3339();
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|entry| entry.cef_version == cef_version)
        {
            existing.chromium_version = chromium_version.to_string();
            existing.reason = reason.to_string();
            existing.at = at;
            return true;
        }
        self.entries.push(DenyEntry {
            cef_version: cef_version.to_string(),
            chromium_version: chromium_version.to_string(),
            reason: reason.to_string(),
            at,
        });
        true
    }
}

/// Falta o está corrupto → lista vacía. Si `schema` o `hostApiVersion` no
/// coinciden, se descarta el contenido y se conserva la versión nueva del host
/// (contrato 3.6). No reescribe el disco.
///
/// Entradas con motivo transitorio (descarga/hash/disco/strip) se tiran aunque
/// el archivo sea válido: un download corrupto nunca queda denylistado.
pub fn load(paths: &CefPaths, host_api_version: u32) -> Denylist {
    let empty = Denylist::new(host_api_version);
    let bytes = match fs::read(paths.denylist_file()) {
        Ok(bytes) => bytes,
        Err(_) => return empty,
    };
    match serde_json::from_slice::<Denylist>(&bytes) {
        Ok(mut list) if list.schema == SCHEMA && list.host_api_version == host_api_version => {
            list.entries
                .retain(|entry| is_persistable_reason(&entry.reason));
            list
        }
        Ok(_) => empty,
        Err(_) => empty,
    }
}

pub fn save(paths: &CefPaths, list: &Denylist) -> Result<(), String> {
    let persistable = persistable_snapshot(list);
    let json = serde_json::to_string_pretty(&persistable)
        .map_err(|error| format!("No se pudo serializar la denylist: {error}"))?;
    atomic_write(&paths.denylist_file(), &json)
}

fn persistable_snapshot(list: &Denylist) -> Denylist {
    Denylist {
        schema: SCHEMA,
        host_api_version: list.host_api_version,
        entries: list
            .entries
            .iter()
            .filter(|entry| is_persistable_reason(&entry.reason))
            .cloned()
            .collect(),
    }
}

fn denylist_tmp_path(parent: &Path) -> PathBuf {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".denylist.json.{}.{seq}.idioteque.tmp",
        std::process::id()
    ))
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Ruta de denylist inválida".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;
    let temporary = denylist_tmp_path(parent);
    match write_tmp_then_rename(&temporary, path, contents) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn write_tmp_then_rename(temporary: &Path, dest: &Path, contents: &str) -> Result<(), String> {
    {
        let mut file = fs::File::create(temporary)
            .map_err(|error| format!("No se pudo escribir la denylist: {error}"))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("No se pudo escribir la denylist: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("No se pudo escribir la denylist: {error}"))?;
    }
    fs::rename(temporary, dest).map_err(|error| format!("No se pudo guardar la denylist: {error}"))
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
}

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
    use tempfile::TempDir;

    fn paths_in(tmp: &TempDir) -> CefPaths {
        CefPaths::new(tmp.path().join("home"), tmp.path().join("base"))
    }

    #[test]
    fn missing_file_is_empty_with_requested_host_version() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let list = load(&paths, 15200);
        assert_eq!(list, Denylist::new(15200));
        assert!(list.entries.is_empty());
        assert!(!list.contains("anything"));
    }

    #[test]
    fn corrupt_file_is_empty() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(paths.denylist_file(), b"{not json").unwrap();
        let list = load(&paths, 15200);
        assert_eq!(list.host_api_version, 15200);
        assert!(list.entries.is_empty());
    }

    #[test]
    fn round_trip_camel_case() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.add(
            "153.0.1+gabc+chromium-153.0.8000.10",
            "153.0.8000.10",
            "health-exit-10",
        );
        save(&paths, &list).expect("save");

        let disk = fs::read_to_string(paths.denylist_file()).unwrap();
        assert!(disk.contains("\"hostApiVersion\""));
        assert!(disk.contains("\"cefVersion\""));
        assert!(disk.contains("\"chromiumVersion\""));
        assert!(!paths.home.join(DENYLIST_TMP_NAME).exists());

        let loaded = load(&paths, 15200);
        assert_eq!(loaded.schema, 1);
        assert_eq!(loaded.host_api_version, 15200);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].cef_version, list.entries[0].cef_version);
        assert_eq!(loaded.entries[0].reason, "health-exit-10");
        assert!(loaded.entries[0].at.ends_with('Z'));
        assert!(loaded.contains("153.0.1+gabc+chromium-153.0.8000.10"));
    }

    #[test]
    fn host_version_mismatch_resets_entries() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.add(
            "153.0.1+gabc+chromium-153.0.8000.10",
            "153.0.8000.10",
            "health-timeout",
        );
        save(&paths, &list).unwrap();

        let loaded = load(&paths, 15300);
        assert_eq!(loaded.host_api_version, 15300);
        assert!(loaded.entries.is_empty());
        assert!(!loaded.contains("153.0.1+gabc+chromium-153.0.8000.10"));
    }

    #[test]
    fn add_replaces_existing_entry_for_same_version() {
        let mut list = Denylist::new(15200);
        list.add("v1", "c1", "health-timeout");
        list.add("v1", "c2", "health-crashed");
        list.add("v2", "c3", "health-no-handshake");

        assert_eq!(list.entries.len(), 2);
        assert!(list.contains("v1"));
        assert!(list.contains("v2"));
        assert_eq!(list.entries[0].chromium_version, "c2");
        assert_eq!(list.entries[0].reason, "health-crashed");
        assert!(!list.entries[0].at.is_empty());
        assert!(list.entries[0].at.ends_with('Z'));
    }

    fn write_raw(paths: &CefPaths, json: &str) {
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(paths.denylist_file(), json).unwrap();
    }

    fn leftover_tmps(paths: &CefPaths) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&paths.home) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.ends_with(".idioteque.tmp").then(|| name.into_owned())
            })
            .collect()
    }

    #[test]
    fn persistable_reasons_match_contract() {
        for reason in [
            "api-version-min-above-host",
            "health-exit-0",
            "health-exit-10",
            "health-exit-16",
            "health-timeout",
            "health-crashed",
            "health-no-handshake",
            "health-spawn",
        ] {
            assert!(is_persistable_reason(reason), "{reason}");
        }
        for reason in [
            "download-corrupt",
            "sha1-mismatch",
            "size-mismatch",
            "disk-full",
            "strip-failure",
            "extract-failed",
            "Hash SHA1 incorrecto: deadbeef",
            "Tamaño incorrecto: 1 != 2",
            "espacio insuficiente",
            "strip falló: x",
            "network",
            "",
            "health-exit-",
            "health-exit-10x",
            "health-exit--1",
            "HEALTH-TIMEOUT",
            "health-timeout ",
        ] {
            assert!(!is_persistable_reason(reason), "{reason}");
        }
    }

    #[test]
    fn add_never_denylists_corrupt_download_or_pipeline_failures() {
        let mut list = Denylist::new(15200);
        for reason in [
            "download-corrupt",
            "sha1-mismatch",
            "size-mismatch",
            "disk-full",
            "strip-failure",
            "extract-failed",
            "Hash SHA1 incorrecto: abc",
            "espacio insuficiente",
        ] {
            assert!(
                !list.add(
                    "153.0.1+gabc+chromium-153.0.8000.10",
                    "153.0.8000.10",
                    reason
                ),
                "{reason}"
            );
        }
        assert!(list.entries.is_empty());
        assert!(!list.contains("153.0.1+gabc+chromium-153.0.8000.10"));
    }

    #[test]
    fn add_does_not_replace_valid_entry_with_transient_reason() {
        let mut list = Denylist::new(15200);
        assert!(list.add("v1", "c1", "health-timeout"));
        let at = list.entries[0].at.clone();
        assert!(!list.add("v1", "c2", "download-corrupt"));
        assert_eq!(list.entries.len(), 1);
        assert_eq!(list.entries[0].reason, "health-timeout");
        assert_eq!(list.entries[0].chromium_version, "c1");
        assert_eq!(list.entries[0].at, at);
    }

    #[test]
    fn load_drops_corrupt_download_entries_even_if_file_is_valid() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_raw(
            &paths,
            r#"{
              "schema": 1,
              "hostApiVersion": 15200,
              "entries": [
                {
                  "cefVersion": "keep",
                  "chromiumVersion": "c",
                  "reason": "health-exit-10",
                  "at": "2026-01-01T00:00:00Z"
                },
                {
                  "cefVersion": "drop-sha1",
                  "chromiumVersion": "c",
                  "reason": "sha1-mismatch",
                  "at": "2026-01-01T00:00:00Z"
                },
                {
                  "cefVersion": "drop-download",
                  "chromiumVersion": "c",
                  "reason": "download-corrupt",
                  "at": "2026-01-01T00:00:00Z"
                },
                {
                  "cefVersion": "drop-strip",
                  "chromiumVersion": "c",
                  "reason": "strip-failure",
                  "at": "2026-01-01T00:00:00Z"
                }
              ]
            }"#,
        );

        let loaded = load(&paths, 15200);
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.contains("keep"));
        assert!(!loaded.contains("drop-sha1"));
        assert!(!loaded.contains("drop-download"));
        assert!(!loaded.contains("drop-strip"));
    }

    #[test]
    fn save_strips_hand_built_transient_entries_from_disk() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.entries.push(DenyEntry {
            cef_version: "bad".into(),
            chromium_version: "c".into(),
            reason: "sha1-mismatch".into(),
            at: "2026-01-01T00:00:00Z".into(),
        });
        assert!(list.add("good", "c", "health-crashed"));
        save(&paths, &list).unwrap();

        let disk = fs::read_to_string(paths.denylist_file()).unwrap();
        assert!(!disk.contains("sha1-mismatch"));
        assert!(!disk.contains("\"bad\""));
        let loaded = load(&paths, 15200);
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.contains("good"));
    }

    #[test]
    fn host_version_mismatch_does_not_rewrite_disk() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.add("v1", "c", "health-timeout");
        save(&paths, &list).unwrap();
        let before = fs::read(paths.denylist_file()).unwrap();

        let loaded = load(&paths, 15300);
        assert!(loaded.entries.is_empty());
        assert_eq!(loaded.host_api_version, 15300);
        assert_eq!(fs::read(paths.denylist_file()).unwrap(), before);

        let still = load(&paths, 15200);
        assert!(still.contains("v1"));
    }

    #[test]
    fn saving_reset_list_overwrites_old_host_entries() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.add("v1", "c", "health-timeout");
        save(&paths, &list).unwrap();

        let reset = load(&paths, 15300);
        save(&paths, &reset).unwrap();

        assert!(!load(&paths, 15200).contains("v1"));
        assert!(load(&paths, 15300).entries.is_empty());
        let disk = fs::read_to_string(paths.denylist_file()).unwrap();
        assert!(disk.contains("\"hostApiVersion\": 15300"));
    }

    #[test]
    fn schema_mismatch_resets_even_if_host_matches() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_raw(
            &paths,
            r#"{
              "schema": 2,
              "hostApiVersion": 15200,
              "entries": [{
                "cefVersion": "stale",
                "chromiumVersion": "c",
                "reason": "health-timeout",
                "at": "2026-01-01T00:00:00Z"
              }]
            }"#,
        );
        let loaded = load(&paths, 15200);
        assert_eq!(loaded, Denylist::new(15200));
        assert!(!loaded.contains("stale"));
    }

    #[test]
    fn snake_case_host_api_version_is_corrupt_reset() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_raw(
            &paths,
            r#"{
              "schema": 1,
              "host_api_version": 15200,
              "entries": [{
                "cefVersion": "stale",
                "chromiumVersion": "c",
                "reason": "health-timeout",
                "at": "2026-01-01T00:00:00Z"
              }]
            }"#,
        );
        let loaded = load(&paths, 15200);
        assert!(loaded.entries.is_empty());
        assert_eq!(loaded.host_api_version, 15200);
    }

    #[test]
    fn host_api_version_string_is_corrupt_reset() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_raw(
            &paths,
            r#"{
              "schema": 1,
              "hostApiVersion": "15200",
              "entries": []
            }"#,
        );
        let loaded = load(&paths, 15200);
        assert_eq!(loaded, Denylist::new(15200));
    }

    #[test]
    fn extra_unknown_fields_are_ignored_when_host_and_schema_match() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_raw(
            &paths,
            r#"{
              "schema": 1,
              "hostApiVersion": 15200,
              "comment": "forward-compat",
              "entries": [{
                "cefVersion": "keep",
                "chromiumVersion": "c",
                "reason": "api-version-min-above-host",
                "at": "2026-01-01T00:00:00Z",
                "extra": true
              }]
            }"#,
        );
        let loaded = load(&paths, 15200);
        assert!(loaded.contains("keep"));
        assert_eq!(loaded.entries[0].reason, "api-version-min-above-host");
    }

    #[test]
    fn trailing_junk_after_json_is_corrupt_reset() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.add("v1", "c", "health-timeout");
        save(&paths, &list).unwrap();
        let mut disk = fs::read_to_string(paths.denylist_file()).unwrap();
        disk.push_str("\nTRAILING");
        fs::write(paths.denylist_file(), disk).unwrap();

        let loaded = load(&paths, 15200);
        assert!(loaded.entries.is_empty());
    }

    #[test]
    fn leftover_tmp_is_never_loaded_as_denylist() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.home).unwrap();
        fs::write(
            paths.home.join(DENYLIST_TMP_NAME),
            r#"{
              "schema": 1,
              "hostApiVersion": 15200,
              "entries": [{
                "cefVersion": "from-tmp",
                "chromiumVersion": "c",
                "reason": "health-timeout",
                "at": "2026-01-01T00:00:00Z"
              }]
            }"#,
        )
        .unwrap();

        let loaded = load(&paths, 15200);
        assert!(loaded.entries.is_empty());
        assert!(!loaded.contains("from-tmp"));
    }

    #[test]
    fn leftover_tmp_does_not_hide_or_corrupt_existing_dest() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        let mut list = Denylist::new(15200);
        list.add("on-disk", "c", "health-exit-11");
        save(&paths, &list).unwrap();
        fs::write(paths.home.join(DENYLIST_TMP_NAME), "{partial-crash").unwrap();

        let loaded = load(&paths, 15200);
        assert!(loaded.contains("on-disk"));
        assert_eq!(loaded.entries[0].reason, "health-exit-11");

        let mut next = Denylist::new(15200);
        next.add("on-disk", "c", "health-crashed");
        save(&paths, &next).unwrap();
        let reloaded = load(&paths, 15200);
        assert_eq!(reloaded.entries[0].reason, "health-crashed");
        let parsed: Denylist =
            serde_json::from_str(&fs::read_to_string(paths.denylist_file()).unwrap()).unwrap();
        assert_eq!(parsed.entries.len(), 1);
    }

    #[test]
    fn save_replaces_truncated_dest_with_complete_json() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_raw(&paths, "{partial");
        let mut list = Denylist::new(15200);
        list.add("v1", "c", "health-no-handshake");
        save(&paths, &list).unwrap();

        let disk = fs::read_to_string(paths.denylist_file()).unwrap();
        let parsed: Denylist =
            serde_json::from_str(&disk).expect("complete JSON after atomic save");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].cef_version, "v1");
        assert!(!paths.home.join(DENYLIST_TMP_NAME).exists());
        assert!(leftover_tmps(&paths).is_empty());
    }

    #[test]
    fn denylist_tmp_names_are_unique() {
        let parent = Path::new("/tmp/cef-denylist-home");
        let a = denylist_tmp_path(parent);
        let b = denylist_tmp_path(parent);
        assert_ne!(a, b);
        for path in [&a, &b] {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with(".denylist.json."));
            assert!(name.ends_with(".idioteque.tmp"));
        }
    }

    #[test]
    fn save_cleans_tmp_when_rename_fails() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(paths.denylist_file()).unwrap();
        let mut list = Denylist::new(15200);
        list.add("v1", "c", "health-exit-10");
        assert!(save(&paths, &list).is_err());
        assert!(leftover_tmps(&paths).is_empty());
    }

    #[test]
    fn concurrent_saves_never_leave_torn_json() {
        let tmp = TempDir::new().unwrap();
        let paths = std::sync::Arc::new(paths_in(&tmp));
        std::thread::scope(|scope| {
            for i in 0..12 {
                let paths = paths.clone();
                scope.spawn(move || {
                    for round in 0..8 {
                        let mut list = Denylist::new(15200);
                        assert!(list.add(&format!("v{i}-{round}"), "c", "health-timeout"));
                        save(&paths, &list).expect("atomic save");
                    }
                });
            }
        });

        let disk = fs::read_to_string(paths.denylist_file()).unwrap();
        let parsed: Denylist =
            serde_json::from_str(&disk).expect("atomic save must never tear JSON");
        assert_eq!(parsed.schema, 1);
        assert_eq!(parsed.host_api_version, 15200);
        assert_eq!(parsed.entries.len(), 1);
        assert!(parsed.entries[0].cef_version.starts_with('v'));
        assert_eq!(parsed.entries[0].reason, "health-timeout");
        assert!(leftover_tmps(&paths).is_empty());
    }
}
