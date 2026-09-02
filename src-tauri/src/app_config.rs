use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const CONFIG_VERSION: u32 = 1;
const MAX_RECENTS: usize = 24;
const MAX_WORKSPACE_VIEWS: usize = 48;
const MIN_FONT_SIZE: u8 = 10;
const MAX_FONT_SIZE: u8 = 24;
const CONFIG_DIR_NAME: &str = ".idioteque";
const CONFIG_FILE_NAME: &str = "config.json";
const CONFIG_TMP_NAME: &str = ".config.json.idioteque.tmp";
const DEFAULT_THEME: &str = "tokyo-night";
const KNOWN_THEMES: [&str; 8] = [
    "tokyo-night",
    "dracula",
    "nord",
    "gruvbox-dark",
    "catppuccin-mocha",
    "one-half-dark",
    "solarized-dark",
    "campbell",
];
const DEFAULT_UI_THEME: &str = "idioteque-dark";
const KNOWN_UI_THEMES: [&str; 11] = [
    "idioteque-dark",
    "idioteque-night",
    "idioteque-light",
    "platzi",
    "tokyo-night",
    "catppuccin-mocha",
    "nord",
    "gruvbox-dark",
    "everforest-dark",
    "one-dark",
    "solarized-dark",
];

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
    #[serde(default = "default_theme")]
    theme: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAppearance {
    #[serde(default = "default_ui_theme")]
    theme: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredWorkspaceView {
    path: String,
    visible_folders: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredConfig {
    version: u32,
    #[serde(default)]
    recents: Vec<StoredRecent>,
    #[serde(default)]
    terminal: StoredTerminal,
    #[serde(default)]
    appearance: StoredAppearance,
    #[serde(default, rename = "workspaceViews")]
    workspace_views: Vec<StoredWorkspaceView>,
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
    pub theme: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettingsUpdate {
    pub font_family: Option<String>,
    pub font_size: u8,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettingsUpdate {
    pub theme: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub path: String,
    pub visible_folders: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewUpdate {
    pub path: String,
    pub visible_folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub recents: Vec<RecentFolder>,
    pub terminal: TerminalSettings,
    pub appearance: AppearanceSettings,
    pub workspace_views: Vec<WorkspaceView>,
}

fn default_font_size() -> u8 {
    13
}

fn default_theme() -> String {
    DEFAULT_THEME.to_string()
}

fn default_terminal() -> StoredTerminal {
    StoredTerminal {
        font_family: None,
        font_size: default_font_size(),
        theme: default_theme(),
    }
}

impl Default for StoredTerminal {
    fn default() -> Self {
        default_terminal()
    }
}

fn default_ui_theme() -> String {
    DEFAULT_UI_THEME.to_string()
}

fn default_appearance() -> StoredAppearance {
    StoredAppearance {
        theme: default_ui_theme(),
    }
}

impl Default for StoredAppearance {
    fn default() -> Self {
        default_appearance()
    }
}

fn default_config() -> StoredConfig {
    StoredConfig {
        version: CONFIG_VERSION,
        recents: Vec::new(),
        terminal: default_terminal(),
        appearance: default_appearance(),
        workspace_views: Vec::new(),
    }
}

fn is_immediate_folder_name(name: &str) -> bool {
    let path = Path::new(name);
    let mut count = 0;

    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return false;
        }
        count += 1;
        if count > 1 {
            return false;
        }
    }

    count == 1
}

fn normalize_visible_folders(folders: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for folder in folders {
        let name = folder.trim();
        if !is_immediate_folder_name(name) {
            continue;
        }
        if seen.insert(name.to_string()) {
            out.push(name.to_string());
        }
    }

    out
}

fn apply_workspace_view(
    mut config: StoredConfig,
    path: String,
    visible_folders: Vec<String>,
) -> StoredConfig {
    let visible_folders = normalize_visible_folders(visible_folders);
    let mut views: Vec<StoredWorkspaceView> = config
        .workspace_views
        .into_iter()
        .filter(|view| !path_keys_match(&view.path, &path))
        .collect();

    views.insert(
        0,
        StoredWorkspaceView {
            path,
            visible_folders,
        },
    );
    views.truncate(MAX_WORKSPACE_VIEWS);

    config.version = CONFIG_VERSION;
    config.workspace_views = views;
    config
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

fn is_known_theme(id: &str) -> bool {
    KNOWN_THEMES.contains(&id)
}

fn normalize_theme(theme: String) -> String {
    let trimmed = theme.trim();
    if is_known_theme(trimmed) {
        trimmed.to_string()
    } else {
        default_theme()
    }
}

fn is_known_ui_theme(id: &str) -> bool {
    KNOWN_UI_THEMES.contains(&id)
}

fn normalize_ui_theme(theme: String) -> String {
    let trimmed = theme.trim();
    if is_known_ui_theme(trimmed) {
        trimmed.to_string()
    } else {
        default_ui_theme()
    }
}

fn apply_appearance(mut config: StoredConfig, theme: String) -> StoredConfig {
    config.version = CONFIG_VERSION;
    config.appearance = StoredAppearance {
        theme: normalize_ui_theme(theme),
    };
    config
}

fn apply_terminal(
    mut config: StoredConfig,
    font_family: Option<String>,
    font_size: u8,
    theme: String,
) -> StoredConfig {
    config.version = CONFIG_VERSION;
    config.terminal = StoredTerminal {
        font_family: normalize_font_family(font_family),
        font_size: clamp_font_size(font_size),
        theme: normalize_theme(theme),
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
            theme: normalize_theme(config.terminal.theme),
        },
        appearance: AppearanceSettings {
            theme: normalize_ui_theme(config.appearance.theme),
        },
        workspace_views: config
            .workspace_views
            .into_iter()
            .map(|view| WorkspaceView {
                path: view.path,
                visible_folders: normalize_visible_folders(view.visible_folders),
            })
            .collect(),
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
        apply_terminal(
            config,
            terminal.font_family,
            terminal.font_size,
            terminal.theme,
        )
    })
}

#[tauri::command]
pub fn update_appearance_settings(
    app: AppHandle,
    appearance: AppearanceSettingsUpdate,
) -> Result<AppConfig, String> {
    mutate_config(&app, |config| apply_appearance(config, appearance.theme))
}

#[tauri::command]
pub fn update_workspace_view(
    app: AppHandle,
    view: WorkspaceViewUpdate,
) -> Result<AppConfig, String> {
    let canonical = resolve_folder(&view.path)?;
    mutate_config(&app, |config| {
        apply_workspace_view(config, canonical, view.visible_folders)
    })
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
            theme: "dracula".into(),
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
        assert_eq!(parsed.appearance, default_appearance());
        assert!(parsed.workspace_views.is_empty());
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
        assert_eq!(parsed.terminal.theme, "tokyo-night");
    }

    #[test]
    fn parse_config_reads_terminal_theme() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "terminal": { "fontSize": 13, "theme": "dracula" }
            }"#,
        );
        assert_eq!(parsed.terminal.theme, "dracula");
    }

    #[test]
    fn annotate_unknown_theme_falls_back_to_tokyo_night() {
        let mut config = default_config();
        config.terminal.theme = "  not-a-theme  ".into();

        let annotated = annotate(config);
        assert_eq!(annotated.terminal.theme, "tokyo-night");
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
        let next = apply_terminal(
            default_config(),
            Some("  Hack Nerd Font  ".into()),
            3,
            "tokyo-night".into(),
        );
        assert_eq!(next.terminal.font_family.as_deref(), Some("Hack Nerd Font"));
        assert_eq!(next.terminal.font_size, MIN_FONT_SIZE);

        let wide = apply_terminal(default_config(), Some("".into()), 99, "tokyo-night".into());
        assert_eq!(wide.terminal.font_family, None);
        assert_eq!(wide.terminal.font_size, MAX_FONT_SIZE);
    }

    #[test]
    fn apply_terminal_keeps_theme_when_changing_font() {
        let next = apply_terminal(
            default_config(),
            Some("Hack".into()),
            16,
            "nord".into(),
        );
        assert_eq!(next.terminal.font_family.as_deref(), Some("Hack"));
        assert_eq!(next.terminal.font_size, 16);
        assert_eq!(next.terminal.theme, "nord");
    }

    #[test]
    fn apply_terminal_unknown_theme_falls_back() {
        let next = apply_terminal(default_config(), None, 13, "  ghost  ".into());
        assert_eq!(next.terminal.theme, "tokyo-night");
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

    #[test]
    fn parse_config_ignores_leftover_footer_key() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "footer": { "actionOrder": ["terminal", "home", "settings", "folder"] }
            }"#,
        );
        assert_eq!(parsed, default_config());
    }

    #[test]
    fn save_and_load_roundtrip_terminal_settings() {
        let home = Home::new();
        let path = home.config_path();
        let stored = apply_terminal(
            default_config(),
            Some("JetBrainsMono Nerd Font".into()),
            16,
            "tokyo-night".into(),
        );
        save_to_path(&path, &stored).expect("save");

        let disk = fs::read_to_string(&path).expect("read disk");
        assert!(disk.contains("JetBrainsMono Nerd Font"));
        assert!(disk.contains("\"fontSize\": 16"));
        assert!(disk.contains("tokyo-night"));

        let reread = load_from_path(&path);
        assert_eq!(reread.terminal, stored.terminal);
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }

    fn custom_appearance() -> StoredAppearance {
        StoredAppearance {
            theme: "idioteque-night".into(),
        }
    }

    #[test]
    fn parse_config_missing_appearance_uses_default() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "terminal": { "fontSize": 13 }
            }"#,
        );
        assert_eq!(parsed.appearance, default_appearance());
        assert_eq!(parsed.appearance.theme, "idioteque-dark");
    }

    #[test]
    fn parse_config_reads_appearance_theme() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "appearance": { "theme": "idioteque-light" }
            }"#,
        );
        assert_eq!(parsed.appearance.theme, "idioteque-light");
    }

    #[test]
    fn annotate_unknown_ui_theme_falls_back_to_idioteque_dark() {
        let mut config = default_config();
        config.appearance.theme = "  not-a-theme  ".into();

        let annotated = annotate(config);
        assert_eq!(annotated.appearance.theme, "idioteque-dark");
    }

    #[test]
    fn apply_appearance_normalizes_and_keeps_known_theme() {
        let next = apply_appearance(default_config(), "  idioteque-night  ".into());
        assert_eq!(next.appearance.theme, "idioteque-night");

        let platzi = apply_appearance(default_config(), "platzi".into());
        assert_eq!(platzi.appearance.theme, "platzi");

        let tokyo_night = apply_appearance(default_config(), "tokyo-night".into());
        assert_eq!(tokyo_night.appearance.theme, "tokyo-night");

        let unknown = apply_appearance(default_config(), "ghost".into());
        assert_eq!(unknown.appearance.theme, "idioteque-dark");
    }

    #[test]
    fn apply_appearance_preserves_terminal() {
        let mut config = default_config();
        config.terminal = nerd_terminal();

        let next = apply_appearance(config, "idioteque-light".into());
        assert_eq!(next.terminal, nerd_terminal());
        assert_eq!(next.appearance.theme, "idioteque-light");
    }

    #[test]
    fn apply_terminal_preserves_appearance() {
        let mut config = default_config();
        config.appearance = custom_appearance();

        let next = apply_terminal(config, Some("Hack".into()), 16, "tokyo-night".into());
        assert_eq!(next.appearance, custom_appearance());
        assert_eq!(next.terminal.font_family.as_deref(), Some("Hack"));
    }

    #[test]
    fn record_recent_preserves_appearance() {
        let mut config = config_with(vec![recent("/a", "2026-01-01T00:00:00Z")]);
        config.appearance = custom_appearance();

        let next = record_recent(config, "/b".into(), "2026-01-02T00:00:00Z".into());
        assert_eq!(next.appearance, custom_appearance());
        assert_eq!(next.recents[0].path, "/b");
    }

    #[test]
    fn save_and_load_roundtrip_appearance_settings() {
        let home = Home::new();
        let path = home.config_path();
        let stored = apply_appearance(default_config(), "idioteque-light".into());
        save_to_path(&path, &stored).expect("save");

        let disk = fs::read_to_string(&path).expect("read disk");
        assert!(disk.contains("\"appearance\""));
        assert!(disk.contains("idioteque-light"));

        let reread = load_from_path(&path);
        assert_eq!(reread.appearance, stored.appearance);
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }

    fn view(path: &str, folders: &[&str]) -> StoredWorkspaceView {
        StoredWorkspaceView {
            path: path.to_string(),
            visible_folders: folders.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    #[test]
    fn parse_config_missing_workspace_views_is_empty() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": []
            }"#,
        );
        assert!(parsed.workspace_views.is_empty());
    }

    #[test]
    fn parse_config_reads_workspace_views() {
        let parsed = parse_config(
            br#"{
              "version": 1,
              "recents": [],
              "workspaceViews": [
                { "path": "/tmp/idioteque", "visibleFolders": [".cursor", "src"] }
              ]
            }"#,
        );
        assert_eq!(
            parsed.workspace_views,
            vec![view("/tmp/idioteque", &[".cursor", "src"])]
        );
    }

    #[test]
    fn apply_workspace_view_replaces_and_moves_to_front() {
        let mut config = default_config();
        config.workspace_views = vec![
            view("/a", &["src"]),
            view("/b", &["docs"]),
        ];

        let next = apply_workspace_view(config, "/b".into(), vec!["src".into(), "docs".into()]);
        assert_eq!(
            next.workspace_views,
            vec![view("/b", &["src", "docs"]), view("/a", &["src"])]
        );
    }

    #[test]
    fn apply_workspace_view_matches_trailing_slash() {
        let mut config = default_config();
        config.workspace_views = vec![view("/gone", &["src"])];

        let next = apply_workspace_view(config, "/gone/".into(), vec!["docs".into()]);
        assert_eq!(next.workspace_views, vec![view("/gone/", &["docs"])]);
    }

    #[test]
    fn apply_workspace_view_caps_at_max() {
        let mut config = default_config();
        config.workspace_views = (0..MAX_WORKSPACE_VIEWS)
            .map(|index| view(&format!("/{index}"), &["src"]))
            .collect();

        let next = apply_workspace_view(config, "/new".into(), vec!["docs".into()]);
        assert_eq!(MAX_WORKSPACE_VIEWS, 48);
        assert_eq!(next.workspace_views.len(), 48);
        assert_eq!(next.workspace_views[0].path, "/new");
        assert!(next
            .workspace_views
            .iter()
            .all(|view| view.path != "/47"));
    }

    #[test]
    fn apply_workspace_view_drops_invalid_and_duplicate_names() {
        let next = apply_workspace_view(
            default_config(),
            "/proj".into(),
            vec![
                "  src  ".into(),
                "src".into(),
                "docs/nested".into(),
                "..".into(),
                "".into(),
                "docs".into(),
            ],
        );
        assert_eq!(next.workspace_views, vec![view("/proj", &["src", "docs"])]);
    }

    #[test]
    fn apply_workspace_view_empty_folders_is_explicit() {
        let next = apply_workspace_view(default_config(), "/proj".into(), vec![]);
        assert_eq!(next.workspace_views, vec![view("/proj", &[])]);
    }

    #[test]
    fn apply_workspace_view_preserves_recents_and_appearance() {
        let mut config = config_with(vec![recent("/a", "2026-01-01T00:00:00Z")]);
        config.appearance = custom_appearance();
        config.terminal = nerd_terminal();

        let next = apply_workspace_view(config, "/proj".into(), vec!["src".into()]);
        assert_eq!(next.recents, vec![recent("/a", "2026-01-01T00:00:00Z")]);
        assert_eq!(next.appearance, custom_appearance());
        assert_eq!(next.terminal, nerd_terminal());
    }

    #[test]
    fn record_recent_preserves_workspace_views() {
        let mut config = default_config();
        config.workspace_views = vec![view("/proj", &["src"])];

        let next = record_recent(config, "/b".into(), "2026-01-02T00:00:00Z".into());
        assert_eq!(next.workspace_views, vec![view("/proj", &["src"])]);
    }

    #[test]
    fn remove_recent_keeps_workspace_views() {
        let mut config = config_with(vec![recent("/proj", "2026-01-01T00:00:00Z")]);
        config.workspace_views = vec![view("/proj", &["src"])];

        let next = remove_recent(config, "/proj");
        assert!(next.recents.is_empty());
        assert_eq!(next.workspace_views, vec![view("/proj", &["src"])]);
    }

    #[test]
    fn save_and_load_roundtrip_workspace_views() {
        let home = Home::new();
        let path = home.config_path();
        let stored = apply_workspace_view(
            default_config(),
            "/tmp/idioteque".into(),
            vec![".cursor".into(), "src".into()],
        );
        save_to_path(&path, &stored).expect("save");

        let disk = fs::read_to_string(&path).expect("read disk");
        assert!(disk.contains("\"workspaceViews\""));
        assert!(disk.contains("visibleFolders"));
        assert!(disk.contains(".cursor"));

        let reread = load_from_path(&path);
        assert_eq!(reread.workspace_views, stored.workspace_views);
        assert!(!path.with_file_name(CONFIG_TMP_NAME).exists());
    }

    #[test]
    fn annotate_exposes_workspace_views() {
        let mut config = default_config();
        config.workspace_views = vec![view("/proj", &["  src  ", "src", "docs"])];

        let annotated = annotate(config);
        assert_eq!(annotated.workspace_views.len(), 1);
        assert_eq!(annotated.workspace_views[0].path, "/proj");
        assert_eq!(
            annotated.workspace_views[0].visible_folders,
            vec!["src", "docs"]
        );
    }
}
