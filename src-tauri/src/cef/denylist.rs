//! `denylist.json` de versiones CEF incompatibles (contrato 3.6).

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::paths::CefPaths;

const SCHEMA: u32 = 1;
const DENYLIST_TMP_NAME: &str = ".denylist.json.idioteque.tmp";

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
    pub fn add(&mut self, cef_version: &str, chromium_version: &str, reason: &str) {
        let at = now_rfc3339();
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|entry| entry.cef_version == cef_version)
        {
            existing.chromium_version = chromium_version.to_string();
            existing.reason = reason.to_string();
            existing.at = at;
            return;
        }
        self.entries.push(DenyEntry {
            cef_version: cef_version.to_string(),
            chromium_version: chromium_version.to_string(),
            reason: reason.to_string(),
            at,
        });
    }
}

/// Falta o está corrupto → lista vacía. Si `hostApiVersion` no coincide, se descarta
/// el contenido y se conserva la versión nueva del host (contrato 3.6).
pub fn load(paths: &CefPaths, host_api_version: u32) -> Denylist {
    let empty = Denylist::new(host_api_version);
    let bytes = match fs::read(paths.denylist_file()) {
        Ok(bytes) => bytes,
        Err(_) => return empty,
    };
    match serde_json::from_slice::<Denylist>(&bytes) {
        Ok(list) if list.host_api_version == host_api_version => list,
        Ok(_) => empty,
        Err(_) => empty,
    }
}

pub fn save(paths: &CefPaths, list: &Denylist) -> Result<(), String> {
    let json = serde_json::to_string_pretty(list)
        .map_err(|error| format!("No se pudo serializar la denylist: {error}"))?;
    atomic_write(&paths.denylist_file(), &json, DENYLIST_TMP_NAME)
}

fn atomic_write(path: &Path, contents: &str, tmp_name: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Ruta de denylist inválida".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;
    let temporary = parent.join(tmp_name);
    fs::write(&temporary, contents)
        .map_err(|error| format!("No se pudo escribir la denylist: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo guardar la denylist: {error}")
    })
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
        list.add("153.0.1+gabc+chromium-153.0.8000.10", "153.0.8000.10", "health-timeout");
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
}
