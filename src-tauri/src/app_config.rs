use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const CONFIG_VERSION: u32 = 1;
const MAX_RECENTS: usize = 24;
const MIN_FONT_SIZE: u8 = 10;
const MAX_FONT_SIZE: u8 = 24;
const CONFIG_DIR_NAME: &str = ".idioteque";
const CONFIG_FILE_NAME: &str = "config.json";
const CONFIG_TMP_NAME: &str = ".config.json.idioteque.tmp";
const DEFAULT_ACTION_ORDER: [&str; 4] = ["home", "folder", "settings", "terminal"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredRecent {
    path: String,
    opened_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTerminal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font_family: Option<String>,
    #[serde(default = "default_font_size")]
    font_size: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredFooter {
    #[serde(default = "default_action_order")]
    action_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredConfig {
    version: u32,
    #[serde(default)]
    recents: Vec<StoredRecent>,
    #[serde(default)]
    terminal: StoredTerminal,
    #[serde(default)]
    footer: StoredFooter,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFolder {
    pub path: String,
    pub opened_at: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettings {
    pub font_family: Option<String>,
    pub font_size: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettingsUpdate {
    pub font_family: Option<String>,
    pub font_size: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FooterSettings {
    pub action_order: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FooterSettingsUpdate {
    pub action_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub recents: Vec<RecentFolder>,
    pub terminal: TerminalSettings,
    pub footer: FooterSettings,
}

fn default_font_size() -> u8 {
    13
}

fn default_terminal() -> StoredTerminal {
    StoredTerminal {
        font_family: None,
        font_size: default_font_size(),
    }
}

impl Default for StoredTerminal {
    fn default() -> Self {
        default_terminal()
    }
}

fn default_action_order() -> Vec<String> {
    DEFAULT_ACTION_ORDER
        .iter()
        .map(|id| (*id).to_string())
        .collect()
}

fn default_footer() -> StoredFooter {
    StoredFooter {
        action_order: default_action_order(),
    }
}

impl Default for StoredFooter {
    fn default() -> Self {
        default_footer()
    }
}

fn default_config() -> StoredConfig {
    StoredConfig {
        version: CONFIG_VERSION,
        recents: Vec::new(),
        terminal: default_terminal(),
        footer: default_footer(),
    }
}

fn clamp_font_size(size: u8) -> u8 {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

fn normalize_font_family(family: Option<String>) -> Option<String> {
    family.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn apply_terminal(
    mut config: StoredConfig,
    font_family: Option<String>,
    font_size: u8,
) -> StoredConfig {
    config.version = CONFIG_VERSION;
    config.terminal = StoredTerminal {
        font_family: normalize_font_family(font_family),
        font_size: clamp_font_size(font_size),
    };
    config
}

fn is_known_action(id: &str) -> bool {
    DEFAULT_ACTION_ORDER.contains(&id)
}

fn normalize_action_order(saved: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();

    for id in saved {
        if !is_known_action(&id) || !seen.insert(id.clone()) {
            continue;
        }
        ordered.push(id);
    }

    for id in DEFAULT_ACTION_ORDER {
        if seen.contains(id) {
            continue;
        }
        ordered.push(id.to_string());
    }

    ordered
}

fn apply_footer(mut config: StoredConfig, action_order: Vec<String>) -> StoredConfig {
    config.version = CONFIG_VERSION;
    config.footer = StoredFooter {
        action_order: normalize_action_order(action_order),
    };
    config
}

fn config_file_in(home: &Path) -> PathBuf {
    home.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME)
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("No se pudo resolver el home: {error}"))?;
    Ok(config_file_in(&home))
}

fn parse_config(bytes: &[u8]) -> StoredConfig {
    match serde_json::from_slice::<StoredConfig>(bytes) {
        Ok(config) if config.version == CONFIG_VERSION => config,
        _ => default_config(),
    }
}

fn load_from_path(path: &Path) -> StoredConfig {
    match fs::read(path) {
        Ok(bytes) => parse_config(&bytes),
        Err(_) => default_config(),
    }
}

fn save_to_path(path: &Path, config: &StoredConfig) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Ruta de configuración inválida".to_string())?;

    fs::create_dir_all(parent)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;

    let json = serde_json::to_string_pretty(config)
        .map_err(|error| format!("No se pudo serializar la configuración: {error}"))?;

    let temporary = parent.join(CONFIG_TMP_NAME);

    fs::write(&temporary, json)
        .map_err(|error| format!("No se pudo escribir la configuración: {error}"))?;

    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("No se pudo guardar la configuración: {error}")
    })
}

fn record_recent(mut config: StoredConfig, path: String, opened_at: String) -> StoredConfig {
    let mut recents: Vec<StoredRecent> = config
        .recents
        .into_iter()
        .filter(|recent| recent.path != path)
        .collect();

    recents.insert(0, StoredRecent { path, opened_at });
    recents.truncate(MAX_RECENTS);

    config.version = CONFIG_VERSION;
    config.recents = recents;
    config
}

fn remove_recent(mut config: StoredConfig, path: &str) -> StoredConfig {
    config.version = CONFIG_VERSION;
    config.recents = config
        .recents
        .into_iter()
        .filter(|recent| !same_recent_path(&recent.path, path))
        .collect();
    config
}

fn same_recent_path(stored: &str, requested: &str) -> bool {
    if path_keys_match(stored, requested) {
        return true;
    }

    match resolve_folder(requested) {
        Ok(canonical) => path_keys_match(stored, &canonical),
        Err(_) => false,
    }
}

fn path_keys_match(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }

    strip_windows_extended_prefix(trim_trailing_seps(left))
        == strip_windows_extended_prefix(trim_trailing_seps(right))
}

fn trim_trailing_seps(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        path
    } else {
        trimmed
    }
}

/// `\\?\C:\Users\…` → `C:\Users\…`. Leave `\\?\UNC\…` alone.
fn strip_windows_extended_prefix(path: &str) -> &str {
    match path.strip_prefix(r#"\\?\"#) {
        Some(rest) if !rest.starts_with("UNC\\") && !rest.starts_with("UNC/") => rest,
        _ => path,
    }
}

fn portable_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    strip_windows_extended_prefix(&raw).to_string()
}

fn annotate(config: StoredConfig) -> AppConfig {
    AppConfig {
        version: config.version,
        recents: config
            .recents
            .into_iter()
            .map(|recent| RecentFolder {
                exists: Path::new(&recent.path).is_dir(),
                path: recent.path,
                opened_at: recent.opened_at,
            })
            .collect(),
        terminal: TerminalSettings {
            font_family: normalize_font_family(config.terminal.font_family),
            font_size: clamp_font_size(config.terminal.font_size),
        },
        footer: FooterSettings {
            action_order: normalize_action_order(config.footer.action_order),
        },
    }
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
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

fn resolve_folder(path: &str) -> Result<String, String> {
    let canonical =
        fs::canonicalize(path).map_err(|error| format!("No se pudo abrir `{path}`: {error}"))?;

    if !canonical.is_dir() {
        return Err(format!("`{path}` no es una carpeta"));
    }

    Ok(portable_path(&canonical))
}

fn mutate_config(
    app: &AppHandle,
    mutate: impl FnOnce(StoredConfig) -> StoredConfig,
) -> Result<AppConfig, String> {
    let path = config_path(app)?;
    let next = mutate(load_from_path(&path));
    save_to_path(&path, &next)?;
    Ok(annotate(next))
}

#[tauri::command]
pub fn load_app_config(app: AppHandle) -> Result<AppConfig, String> {
    let path = config_path(&app)?;
    Ok(annotate(load_from_path(&path)))
}

#[tauri::command]
pub fn record_recent_folder(app: AppHandle, path: String) -> Result<AppConfig, String> {
    let canonical = resolve_folder(&path)?;
    let opened_at = now_rfc3339();

    mutate_config(&app, |config| record_recent(config, canonical, opened_at))
}

#[tauri::command]
pub fn remove_recent_folder(app: AppHandle, path: String) -> Result<AppConfig, String> {
    mutate_config(&app, |config| remove_recent(config, &path))
}

#[tauri::command]
pub fn update_terminal_settings(
    app: AppHandle,
    terminal: TerminalSettingsUpdate,
) -> Result<AppConfig, String> {
    mutate_config(&app, |config| {
        apply_terminal(config, terminal.font_family, terminal.font_size)
    })
}

#[tauri::command]
pub fn update_footer_settings(
    app: AppHandle,
    footer: FooterSettingsUpdate,
) -> Result<AppConfig, String> {
    mutate_config(&app, |config| apply_footer(config, footer.action_order))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQ: AtomicU64 = AtomicU64::new(0);

    struct Home {
        root: PathBuf,
    }

    impl Home {
        fn new() -> Self {
            let seq = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("idioteque-home-{nanos}-{seq}"));
            fs::create_dir_all(&root).expect("create fake home");
            Self { root }
        }

        fn config_path(&self) -> PathBuf {
            config_file_in(&self.root)
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn recent(path: &str, opened_at: &str) -> StoredRecent {
        StoredRecent {
            path: path.to_string(),
            opened_at: opened_at.to_string(),
        }
    }

    fn config_with(recents: Vec<StoredRecent>) -> StoredConfig {
        StoredConfig {
            recents,
            ..default_config()
        }
    }

    fn nerd_terminal() -> StoredTerminal {
        StoredTerminal {
            font_family: Some("JetBrainsMono Nerd Font".into()),
            font_size: 16,
        }
    }

    #[test]
    fn format_unix_utc_known_instants() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(86_400), "1970-01-02T00:00:00Z");
        assert_eq!(format_unix_utc(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(format_unix_utc(1_704_067_200), "2024-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format_unix_utc(1_755_476_700), "2025-08-18T00:25:00Z");
    }

    #[test]
    fn parse_config_recovers_from_missing_or_broken() {
        assert_eq!(parse_config(b""), default_config());
        assert_eq!(parse_config(b"{not json"), default_config());
        assert_eq!(
            parse_config(br#"{"version":99,"recents":[]}"#),
            default_config()
        );
    }

    #[test]
    fn parse_config_unknown_version_drops_valid_recents() {
        assert_eq!(
            parse_config(
                br#"{
                  "version": 99,
                  "recents": [
                    { "path": "/tmp/docs", "openedAt": "2026-08-18T20:25:00Z" }
                  ]
                }"#,
            ),
            default_config()
        );
    }

    #[test]
    fn parse_config_missing_recents_is_empty() {
        let parsed = parse_config(br#"{"version":1}"#);
        assert_eq!(parsed.version, 1);
        assert!(parsed.recents.is_empty());
        assert_eq!(parsed.terminal, default_terminal());
        assert_eq!(parsed.footer, default_footer());
    }

    #[test]
    fn parse_config_reads_versioned_json() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [
                { "path": "/tmp/docs", "openedAt": "2026-08-18T20:25:00Z" }
              ]
            }"#,
        );

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.recents.len(), 1);
        assert_eq!(parsed.recents[0].path, "/tmp/docs");
        assert_eq!(parsed.recents[0].opened_at, "2026-08-18T20:25:00Z");
        assert_eq!(parsed.terminal, default_terminal());
    }

    #[test]
    fn parse_config_reads_terminal_settings() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "terminal": { "fontFamily": "JetBrainsMono Nerd Font", "fontSize": 16 }
            }"#,
        );

        assert_eq!(
            parsed.terminal.font_family.as_deref(),
            Some("JetBrainsMono Nerd Font")
        );
        assert_eq!(parsed.terminal.font_size, 16);
    }

    #[test]
    fn record_recent_dedupes_and_moves_to_front() {
        let config = config_with(vec![
            recent("/a", "2026-01-01T00:00:00Z"),
            recent("/b", "2026-01-02T00:00:00Z"),
        ]);

        let next = record_recent(config, "/b".into(), "2026-01-03T00:00:00Z".into());
        assert_eq!(
            next.recents,
            vec![
                recent("/b", "2026-01-03T00:00:00Z"),
                recent("/a", "2026-01-01T00:00:00Z"),
            ]
        );
    }

    #[test]
    fn record_recent_caps_at_max() {
        let recents = (0..MAX_RECENTS)
            .map(|index| recent(&format!("/{index}"), "2026-01-01T00:00:00Z"))
            .collect();
        let config = config_with(recents);

        let next = record_recent(config, "/new".into(), "2026-01-02T00:00:00Z".into());
        assert_eq!(MAX_RECENTS, 24);
        assert_eq!(next.recents.len(), 24);
        assert_eq!(next.recents[0].path, "/new");
        assert!(next.recents.iter().all(|recent| recent.path != "/23"));
    }

    #[test]
    fn record_recent_rerecords_at_cap_without_dropping_other() {
        let recents = (0..24)
            .map(|index| recent(&format!("/{index}"), "2026-01-01T00:00:00Z"))
            .collect();
        let config = config_with(recents);

        let next = record_recent(config, "/5".into(), "2026-01-02T00:00:00Z".into());
        assert_eq!(next.recents.len(), 24);
        assert_eq!(next.recents[0].path, "/5");
        assert_eq!(next.recents[0].opened_at, "2026-01-02T00:00:00Z");
        assert_eq!(
            next.recents
                .iter()
                .filter(|recent| recent.path == "/5")
                .count(),
            1
        );
        assert!(next.recents.iter().any(|recent| recent.path == "/23"));
    }

    #[test]
    fn remove_recent_is_idempotent() {
        let config = config_with(vec![recent("/a", "2026-01-01T00:00:00Z")]);

        let next = remove_recent(config, "/a");
        assert!(next.recents.is_empty());
        assert!(remove_recent(next, "/a").recents.is_empty());
    }

    #[test]
    fn remove_recent_keeps_other_entries() {
        let config = config_with(vec![
            recent("/a", "2026-01-01T00:00:00Z"),
            recent("/b", "2026-01-02T00:00:00Z"),
        ]);

        assert_eq!(
            remove_recent(config, "/a").recents,
            vec![recent("/b", "2026-01-02T00:00:00Z")]
        );
    }

    #[test]
    fn remove_recent_trims_slash_on_missing_folder() {
        let config = config_with(vec![
            recent("/gone", "2026-01-01T00:00:00Z"),
            recent("/keep", "2026-01-02T00:00:00Z"),
        ]);

        assert_eq!(
            remove_recent(config, "/gone/").recents,
            vec![recent("/keep", "2026-01-02T00:00:00Z")]
        );
    }

    #[test]
    fn remove_recent_matches_existing_folder_with_trailing_slash() {
        let home = Home::new();
        let dir = home.root.join("docs");
        fs::create_dir_all(&dir).expect("docs");
        let canonical = resolve_folder(&dir.to_string_lossy()).expect("resolve");

        let config = config_with(vec![
            recent(&canonical, "2026-01-01T00:00:00Z"),
            recent("/keep", "2026-01-02T00:00:00Z"),
        ]);

        let with_slash = format!("{canonical}/");
        assert_eq!(
            remove_recent(config, &with_slash).recents,
            vec![recent("/keep", "2026-01-02T00:00:00Z")]
        );
    }

    #[test]
    fn load_and_save_roundtrip_creates_dotfolder() {
        let home = Home::new();
        let path = home.config_path();
        assert!(!path.exists());

        let loaded = load_from_path(&path);
        assert_eq!(loaded, default_config());
        assert!(!path.exists());

        let recorded = record_recent(loaded, "/tmp/docs".into(), "2026-08-18T20:25:00Z".into());
        save_to_path(&path, &recorded).expect("save");

        assert!(path.exists());
        assert_eq!(
            path.parent().and_then(|parent| parent.file_name()),
            Some(std::ffi::OsStr::new(".idioteque"))
        );

        let reread = load_from_path(&path);
        assert_eq!(reread, recorded);

        let disk = fs::read_to_string(&path).expect("read disk");
        assert!(disk.contains("\"openedAt\""));
        assert!(disk.contains("/tmp/docs"));
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }

    #[test]
    fn save_to_path_overwrites_existing_config() {
        let home = Home::new();
        let path = home.config_path();
        let first = record_recent(default_config(), "/a".into(), "2026-01-01T00:00:00Z".into());
        save_to_path(&path, &first).expect("save first");

        let second = record_recent(default_config(), "/b".into(), "2026-01-02T00:00:00Z".into());
        save_to_path(&path, &second).expect("save second");

        assert_eq!(load_from_path(&path), second);
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }

    #[test]
    fn annotate_marks_missing_folders_without_dropping_them() {
        let home = Home::new();
        let existing = home.root.join("alive");
        fs::create_dir_all(&existing).expect("create alive");

        let config = config_with(vec![
            recent(&existing.to_string_lossy(), "2026-01-01T00:00:00Z"),
            recent(
                "/definitely/missing/idioteque-folder",
                "2026-01-02T00:00:00Z",
            ),
        ]);

        let annotated = annotate(config);
        assert_eq!(annotated.recents.len(), 2);
        assert!(annotated.recents[0].exists);
        assert!(!annotated.recents[1].exists);
    }

    #[test]
    fn annotate_file_is_not_existing_folder() {
        let home = Home::new();
        let file = home.root.join("note.md");
        fs::write(&file, "x").expect("write file");

        let config = config_with(vec![recent(
            &file.to_string_lossy(),
            "2026-01-01T00:00:00Z",
        )]);

        let annotated = annotate(config);
        assert_eq!(annotated.recents.len(), 1);
        assert!(!annotated.recents[0].exists);
    }

    #[test]
    fn resolve_folder_rejects_files() {
        let home = Home::new();
        let file = home.root.join("note.md");
        fs::write(&file, "x").expect("write file");

        let error = resolve_folder(&file.to_string_lossy()).unwrap_err();
        assert!(error.contains("no es una carpeta"));
    }

    #[test]
    fn resolve_folder_ok_canonicalizes() {
        let home = Home::new();
        let foo = home.root.join("foo");
        fs::create_dir_all(&foo).expect("foo");

        let via_slash = resolve_folder(&format!("{}/", foo.display())).expect("slash");
        let via_dotdot =
            resolve_folder(&foo.join("..").join("foo").to_string_lossy()).expect("dotdot");

        assert_eq!(via_slash, via_dotdot);
        assert!(!via_slash.ends_with('/'));
        assert!(Path::new(&via_slash).is_dir());
    }

    #[test]
    fn resolve_folder_missing_path_errors() {
        let error = resolve_folder("/definitely/missing/idioteque-folder").unwrap_err();
        assert!(error.contains("No se pudo abrir"));
    }

    #[test]
    fn strip_windows_extended_prefix_drive_but_not_unc() {
        assert_eq!(
            strip_windows_extended_prefix(r#"\\?\C:\Users\x"#),
            r#"C:\Users\x"#
        );
        assert_eq!(
            strip_windows_extended_prefix(r#"\\?\UNC\server\share"#),
            r#"\\?\UNC\server\share"#
        );
        assert_eq!(strip_windows_extended_prefix("/home/x"), "/home/x");
    }

    #[test]
    fn record_recent_preserves_terminal_settings() {
        let mut config = config_with(vec![recent("/a", "2026-01-01T00:00:00Z")]);
        config.terminal = nerd_terminal();

        let next = record_recent(config, "/b".into(), "2026-01-02T00:00:00Z".into());
        assert_eq!(next.terminal, nerd_terminal());
        assert_eq!(next.recents[0].path, "/b");
    }

    #[test]
    fn remove_recent_preserves_terminal_settings() {
        let mut config = config_with(vec![
            recent("/a", "2026-01-01T00:00:00Z"),
            recent("/b", "2026-01-02T00:00:00Z"),
        ]);
        config.terminal = nerd_terminal();

        let next = remove_recent(config, "/a");
        assert_eq!(next.terminal, nerd_terminal());
        assert_eq!(next.recents, vec![recent("/b", "2026-01-02T00:00:00Z")]);
    }

    #[test]
    fn apply_terminal_clamps_size_and_trims_family() {
        let next = apply_terminal(default_config(), Some("  Hack Nerd Font  ".into()), 3);
        assert_eq!(next.terminal.font_family.as_deref(), Some("Hack Nerd Font"));
        assert_eq!(next.terminal.font_size, MIN_FONT_SIZE);

        let wide = apply_terminal(default_config(), Some("".into()), 99);
        assert_eq!(wide.terminal.font_family, None);
        assert_eq!(wide.terminal.font_size, MAX_FONT_SIZE);
    }

    #[test]
    fn annotate_clamps_terminal_font_size() {
        let mut config = default_config();
        config.terminal.font_family = Some("  ".into());
        config.terminal.font_size = 4;

        let annotated = annotate(config);
        assert_eq!(annotated.terminal.font_family, None);
        assert_eq!(annotated.terminal.font_size, MIN_FONT_SIZE);
    }

    fn custom_footer() -> StoredFooter {
        StoredFooter {
            action_order: vec![
                "terminal".into(),
                "home".into(),
                "settings".into(),
                "folder".into(),
            ],
        }
    }

    #[test]
    fn parse_config_missing_footer_uses_default() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "terminal": { "fontSize": 13 }
            }"#,
        );
        assert_eq!(parsed.footer, default_footer());
    }

    #[test]
    fn parse_config_reads_footer_order() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "footer": { "actionOrder": ["terminal", "home", "settings", "folder"] }
            }"#,
        );
        assert_eq!(parsed.footer, custom_footer());
    }

    #[test]
    fn annotate_normalizes_invalid_footer_order() {
        let mut config = default_config();
        config.footer.action_order = vec![
            "ghost".into(),
            "home".into(),
            "home".into(),
            "terminal".into(),
        ];

        let annotated = annotate(config);
        assert_eq!(
            annotated.footer.action_order,
            vec![
                "home".to_string(),
                "terminal".to_string(),
                "folder".to_string(),
                "settings".to_string()
            ]
        );
    }

    #[test]
    fn apply_footer_normalizes_and_keeps_known_order() {
        let next = apply_footer(
            default_config(),
            vec![
                "terminal".into(),
                "ghost".into(),
                "home".into(),
                "home".into(),
            ],
        );
        assert_eq!(
            next.footer.action_order,
            vec![
                "terminal".to_string(),
                "home".to_string(),
                "folder".to_string(),
                "settings".to_string()
            ]
        );
    }

    #[test]
    fn apply_terminal_preserves_footer() {
        let mut config = default_config();
        config.footer = custom_footer();

        let next = apply_terminal(config, Some("Hack".into()), 16);
        assert_eq!(next.footer, custom_footer());
        assert_eq!(next.terminal.font_family.as_deref(), Some("Hack"));
    }

    #[test]
    fn record_recent_preserves_footer() {
        let mut config = config_with(vec![recent("/a", "2026-01-01T00:00:00Z")]);
        config.footer = custom_footer();

        let next = record_recent(config, "/b".into(), "2026-01-02T00:00:00Z".into());
        assert_eq!(next.footer, custom_footer());
        assert_eq!(next.recents[0].path, "/b");
    }

    #[test]
    fn apply_footer_preserves_terminal() {
        let mut config = default_config();
        config.terminal = nerd_terminal();

        let next = apply_footer(
            config,
            vec![
                "settings".into(),
                "terminal".into(),
                "home".into(),
                "folder".into(),
            ],
        );
        assert_eq!(next.terminal, nerd_terminal());
        assert_eq!(next.footer.action_order[0], "settings");
    }

    #[test]
    fn save_and_load_roundtrip_footer_settings() {
        let home = Home::new();
        let path = home.config_path();
        let stored = apply_footer(
            default_config(),
            vec![
                "terminal".into(),
                "home".into(),
                "settings".into(),
                "folder".into(),
            ],
        );
        save_to_path(&path, &stored).expect("save");

        let disk = fs::read_to_string(&path).expect("read disk");
        assert!(disk.contains("\"actionOrder\""));
        assert!(disk.contains("terminal"));

        let reread = load_from_path(&path);
        assert_eq!(reread.footer, stored.footer);
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }

    #[test]
    fn save_and_load_roundtrip_terminal_settings() {
        let home = Home::new();
        let path = home.config_path();
        let stored = apply_terminal(default_config(), Some("JetBrainsMono Nerd Font".into()), 16);
        save_to_path(&path, &stored).expect("save");

        let disk = fs::read_to_string(&path).expect("read disk");
        assert!(disk.contains("JetBrainsMono Nerd Font"));
        assert!(disk.contains("\"fontSize\": 16"));

        let reread = load_from_path(&path);
        assert_eq!(reread.terminal, stored.terminal);
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }
}
