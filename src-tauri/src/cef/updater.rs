//! Ciclo del updater CEF: índice → descarga → extract → strip → health → promote.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::archive::{build_manifest, extract_runtime, parse_api_versions, parse_cef_version};
use super::denylist;
use super::download::{self, download_verified, MIN_FREE_BYTES};
use super::elf_strip::strip_libcef_in_place;
use super::health::{self, HealthOutcome, HealthRun, HEALTH_TIMEOUT};
use super::host::CefState;
use super::index::{download_url, fetch_index, parse_index, select_candidate, IndexFetch};
use super::manifest;
use super::paths::{self, CefPaths, PLATFORM};
use super::promote::{
    discard_candidate, mark_candidate_verified, promote_candidate, PromoteResult, Promoted,
};
use super::state::{self, UpdaterState};
use super::version::CefVersion;

const DEFAULT_INDEX_URL: &str = "https://cef-builds.spotifycdn.com/index.json";
const BASE_JSON: &str = include_str!("../../cef/base.json");
const STARTUP_DELAY_SECS: u64 = 30;
const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const LOOP_SLEEP_SECS: u64 = 10 * 60;

static CYCLE_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateEvent {
    Updated {
        chromium: String,
        cef: String,
    },
    #[serde(rename_all = "camelCase")]
    Incompatible {
        candidate_chromium: String,
        candidate_cef: String,
        current_chromium: String,
        current_cef: String,
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub struct UpdaterContext {
    pub paths: CefPaths,
    pub host_binary: PathBuf,
    pub host_api_version: u32,
    pub no_sandbox: bool,
    pub index_url: String,
    pub download_base_url: String,
    pub platform: String,
    /// Si es `true`, no se llama a `strip_libcef_in_place` (tests sintéticos).
    pub skip_strip: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleOutcome {
    NoNewer,
    NotModified,
    Updated(Promoted),
    Deferred { cef: String, chromium: String },
    Incompatible { cef: String, chromium: String, reason: String },
    Skipped(String),
}

struct CycleGuard;

impl Drop for CycleGuard {
    fn drop(&mut self) {
        CYCLE_RUNNING.store(false, Ordering::SeqCst);
    }
}

fn try_begin_cycle() -> Option<CycleGuard> {
    CYCLE_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .ok()
        .map(|_| CycleGuard)
}

pub fn run_cycle(
    ctx: &UpdaterContext,
    host_alive: &dyn Fn() -> bool,
    emit: &dyn Fn(UpdateEvent),
) -> CycleOutcome {
    let _ = ctx.paths.ensure_dirs();
    let mut state = state::load(&ctx.paths);
    let mut denylist = denylist::load(&ctx.paths, ctx.host_api_version);

    let mut early_promoted: Option<Promoted> = None;
    if state.pending_promotion.is_some() && !host_alive() {
        log_step(&ctx.paths, "promoción diferida pendiente y el host no está vivo");
        match promote_candidate(&ctx.paths, false) {
            Ok(PromoteResult::Promoted(promoted)) => {
                emit(UpdateEvent::Updated {
                    chromium: promoted.chromium_version.clone(),
                    cef: promoted.cef_version.clone(),
                });
                log_step(
                    &ctx.paths,
                    &format!(
                        "promovido Chromium {} (CEF {})",
                        promoted.chromium_version, promoted.cef_version
                    ),
                );
                early_promoted = Some(promoted);
                state.pending_promotion = None;
            }
            Ok(PromoteResult::Deferred(pending)) => {
                state.pending_promotion = Some(pending);
            }
            Ok(PromoteResult::NothingToPromote) => {
                state.pending_promotion = None;
            }
            Err(error) => {
                log_step(&ctx.paths, &format!("no se pudo promover lo pendiente: {error}"));
            }
        }
    }

    log_step(&ctx.paths, &format!("consultando índice {}", ctx.index_url));
    let fetched = match fetch_index(&ctx.index_url, state.index_etag.as_deref()) {
        Ok(fetched) => fetched,
        Err(error) => {
            return finish(
                &ctx.paths,
                &mut state,
                CycleOutcome::Skipped(error),
            );
        }
    };

    match fetched {
        IndexFetch::NotModified => {
            log_step(&ctx.paths, "índice no modificado (ETag)");
            let outcome = if let Some(promoted) = early_promoted {
                CycleOutcome::Updated(promoted)
            } else {
                CycleOutcome::NotModified
            };
            return finish(&ctx.paths, &mut state, outcome);
        }
        IndexFetch::Fetched { body, etag } => {
            state.index_etag = etag;
            match continue_with_index(ctx, host_alive, emit, &mut state, &mut denylist, &body)
            {
                CycleOutcome::NoNewer => {
                    if let Some(promoted) = early_promoted {
                        finish(&ctx.paths, &mut state, CycleOutcome::Updated(promoted))
                    } else {
                        finish(&ctx.paths, &mut state, CycleOutcome::NoNewer)
                    }
                }
                other => finish(&ctx.paths, &mut state, other),
            }
        }
    }
}

fn continue_with_index(
    ctx: &UpdaterContext,
    host_alive: &dyn Fn() -> bool,
    emit: &dyn Fn(UpdateEvent),
    _state: &mut UpdaterState,
    denylist: &mut denylist::Denylist,
    body: &str,
) -> CycleOutcome {
    let index = match parse_index(body) {
        Ok(index) => index,
        Err(error) => return CycleOutcome::Skipped(error),
    };

    let effective = match manifest::resolve_effective(&ctx.paths) {
        Ok(slot) => slot,
        Err(error) => return CycleOutcome::Skipped(error),
    };
    let current_cef = effective.manifest.cef_version.clone();
    let current_chromium = effective.manifest.chromium_version.clone();
    let current_ver = match CefVersion::parse(&current_cef) {
        Ok(parsed) => parsed,
        Err(error) => return CycleOutcome::Skipped(error),
    };

    let Some(candidate) = select_candidate(&index, &ctx.platform, &current_ver, &|cef| {
        denylist.contains(cef)
    }) else {
        log_step(&ctx.paths, "no hay una versión stable más nueva");
        return CycleOutcome::NoNewer;
    };

    let cand_cef = candidate.version.cef_version.clone();
    let cand_chromium = candidate.version.chromium_version.clone();
    log_step(
        &ctx.paths,
        &format!("candidato {} ({})", cand_chromium, candidate.file.name),
    );

    match download::available_space(&ctx.paths.home) {
        Ok(free) if free >= MIN_FREE_BYTES => {}
        Ok(free) => {
            return CycleOutcome::Skipped(format!(
                "espacio insuficiente: {free} < {MIN_FREE_BYTES}"
            ));
        }
        Err(error) => return CycleOutcome::Skipped(error),
    }

    if let Err(error) = discard_candidate(&ctx.paths) {
        return CycleOutcome::Skipped(error);
    }
    if let Err(error) = fs::create_dir_all(ctx.paths.candidate()) {
        return CycleOutcome::Skipped(format!(
            "No se pudo crear el candidate: {error}"
        ));
    }

    let tarball_in = ctx.paths.candidate().join("download.tar.bz2");
    let url = download_url(&ctx.download_base_url, &candidate.file.name);
    log_step(&ctx.paths, &format!("descargando {url}"));
    if let Err(error) = download_verified(
        &url,
        &tarball_in,
        candidate.file.size,
        &candidate.file.sha1,
    ) {
        let _ = discard_candidate(&ctx.paths);
        return CycleOutcome::Skipped(error.message());
    }

    let tarball_out = ctx.paths.home.join("download.tar.bz2");
    let _ = fs::remove_file(&tarball_out);
    if let Err(error) = fs::rename(&tarball_in, &tarball_out) {
        let _ = discard_candidate(&ctx.paths);
        return CycleOutcome::Skipped(format!("No se pudo apartar el tarball: {error}"));
    }

    log_step(&ctx.paths, "extrayendo runtime");
    let extract = extract_runtime(&tarball_out, &ctx.paths.candidate());
    let _ = fs::remove_file(&tarball_out);
    if let Err(error) = extract {
        let _ = discard_candidate(&ctx.paths);
        return CycleOutcome::Skipped(error);
    }

    let api_header = ctx
        .paths
        .candidate()
        .join("include")
        .join("cef_api_versions.h");
    let ver_header = ctx.paths.candidate().join("include").join("cef_version.h");
    let api_text = match fs::read_to_string(&api_header) {
        Ok(text) => text,
        Err(error) => {
            let _ = discard_candidate(&ctx.paths);
            return CycleOutcome::Skipped(format!(
                "No se pudo leer cef_api_versions.h: {error}"
            ));
        }
    };
    let ver_text = match fs::read_to_string(&ver_header) {
        Ok(text) => text,
        Err(error) => {
            let _ = discard_candidate(&ctx.paths);
            return CycleOutcome::Skipped(format!("No se pudo leer cef_version.h: {error}"));
        }
    };
    let (api_min, api_last) = match parse_api_versions(&api_text) {
        Ok(pair) => pair,
        Err(error) => {
            let _ = discard_candidate(&ctx.paths);
            return CycleOutcome::Skipped(error);
        }
    };
    let (header_cef, header_chromium) = match parse_cef_version(&ver_text) {
        Ok(pair) => pair,
        Err(_) => (cand_cef.clone(), cand_chromium.clone()),
    };

    if api_min > ctx.host_api_version {
        let reason = "api-version-min-above-host";
        log_step(&ctx.paths, reason);
        denylist.add(&header_cef, &header_chromium, reason);
        let _ = denylist::save(&ctx.paths, denylist);
        let _ = discard_candidate(&ctx.paths);
        emit(UpdateEvent::Incompatible {
            candidate_chromium: header_chromium.clone(),
            candidate_cef: header_cef.clone(),
            current_chromium: current_chromium.clone(),
            current_cef: current_cef.clone(),
            reason: reason.to_string(),
        });
        return CycleOutcome::Incompatible {
            cef: header_cef,
            chromium: header_chromium,
            reason: reason.to_string(),
        };
    }

    let mut stripped = false;
    if !ctx.skip_strip {
        let libcef = ctx.paths.candidate().join("libcef.so");
        log_step(&ctx.paths, "stripping libcef.so");
        match strip_libcef_in_place(&libcef) {
            Ok(report) => {
                stripped = true;
                log_step(
                    &ctx.paths,
                    &format!(
                        "strip {:?}: {} → {}",
                        report.method, report.before, report.after
                    ),
                );
            }
            Err(error) => {
                let _ = discard_candidate(&ctx.paths);
                return CycleOutcome::Skipped(format!("strip falló: {error}"));
            }
        }
    }

    let manifest = match build_manifest(
        &ctx.paths.candidate(),
        &header_cef,
        &header_chromium,
        &ctx.platform,
        api_min,
        api_last,
        &candidate.file,
        stripped,
    ) {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = discard_candidate(&ctx.paths);
            return CycleOutcome::Skipped(error);
        }
    };
    if let Err(error) = manifest::save(&ctx.paths.candidate(), &manifest) {
        let _ = discard_candidate(&ctx.paths);
        return CycleOutcome::Skipped(error);
    }
    if let Err(error) = manifest::validate(&ctx.paths.candidate(), &manifest) {
        let _ = discard_candidate(&ctx.paths);
        return CycleOutcome::Skipped(error);
    }

    let cache_dir = ctx.paths.health_cache(std::process::id());
    let log_file = ctx.paths.logs_dir().join("updater-health.log");
    log_step(&ctx.paths, "health check del candidate");
    let health = health::run_health_check(&HealthRun {
        binary: &ctx.host_binary,
        slot_dir: &ctx.paths.candidate(),
        cache_dir: &cache_dir,
        no_sandbox: ctx.no_sandbox,
        log_file: Some(&log_file),
        timeout: HEALTH_TIMEOUT,
    });

    match health {
        HealthOutcome::Failed(failure) => {
            let reason = failure.denylist_reason();
            log_step(&ctx.paths, &format!("health check falló: {reason}"));
            denylist.add(&header_cef, &header_chromium, &reason);
            let _ = denylist::save(&ctx.paths, denylist);
            let _ = discard_candidate(&ctx.paths);
            emit(UpdateEvent::Incompatible {
                candidate_chromium: header_chromium.clone(),
                candidate_cef: header_cef.clone(),
                current_chromium,
                current_cef,
                reason: reason.clone(),
            });
            CycleOutcome::Incompatible {
                cef: header_cef,
                chromium: header_chromium,
                reason,
            }
        }
        HealthOutcome::Passed { .. } => {
            log_step(&ctx.paths, "health check ok; verificando candidate");
            if let Err(error) = mark_candidate_verified(&ctx.paths) {
                let _ = discard_candidate(&ctx.paths);
                return CycleOutcome::Skipped(error);
            }
            match promote_candidate(&ctx.paths, host_alive()) {
                Ok(PromoteResult::Promoted(promoted)) => {
                    emit(UpdateEvent::Updated {
                        chromium: promoted.chromium_version.clone(),
                        cef: promoted.cef_version.clone(),
                    });
                    log_step(
                        &ctx.paths,
                        &format!(
                            "actualizado a Chromium {}",
                            promoted.chromium_version
                        ),
                    );
                    CycleOutcome::Updated(promoted)
                }
                Ok(PromoteResult::Deferred(pending)) => {
                    log_step(&ctx.paths, "promoción diferida: cef-host vivo");
                    CycleOutcome::Deferred {
                        cef: pending.cef_version,
                        chromium: pending.chromium_version,
                    }
                }
                Ok(PromoteResult::NothingToPromote) => {
                    CycleOutcome::Skipped("nada que promover".into())
                }
                Err(error) => CycleOutcome::Skipped(error),
            }
        }
    }
}

fn finish(paths: &CefPaths, state: &mut UpdaterState, outcome: CycleOutcome) -> CycleOutcome {
    state.last_check_at = Some(state::now_rfc3339());
    state.last_outcome = Some(outcome_label(&outcome));
    if let Err(error) = state::save(paths, state) {
        log_step(paths, &format!("no se pudo guardar state.json: {error}"));
    }
    log_step(paths, &format!("resultado: {}", outcome_label(&outcome)));
    outcome
}

fn outcome_label(outcome: &CycleOutcome) -> String {
    match outcome {
        CycleOutcome::NoNewer => "no-newer".into(),
        CycleOutcome::NotModified => "not-modified".into(),
        CycleOutcome::Updated(_) => "updated".into(),
        CycleOutcome::Deferred { .. } => "deferred".into(),
        CycleOutcome::Incompatible { reason, .. } => format!("incompatible:{reason}"),
        CycleOutcome::Skipped(reason) => format!("skipped:{reason}"),
    }
}

fn log_step(paths: &CefPaths, message: &str) {
    eprintln!("[cef-updater] {message}");
    let _ = fs::create_dir_all(paths.logs_dir());
    let path = paths.logs_dir().join("updater.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[cef-updater] {message}");
    }
}

pub fn start_scheduler(app: AppHandle) {
    thread::spawn(move || {
        let delay = env_u64("IDIOTEQUE_CEF_STARTUP_DELAY_SECS", STARTUP_DELAY_SECS);
        thread::sleep(Duration::from_secs(delay));
        loop {
            if let Ok(ctx) = context_from_app(&app) {
                if check_is_due(&ctx.paths) {
                    if let Some(_guard) = try_begin_cycle() {
                        run_app_cycle(&app, &ctx);
                    }
                }
            }
            thread::sleep(Duration::from_secs(LOOP_SLEEP_SECS));
        }
    });
}

#[tauri::command]
pub fn cef_check_updates(app: AppHandle) -> Result<(), String> {
    let Some(_guard) = try_begin_cycle() else {
        return Err("Ya hay un ciclo del updater CEF en curso".to_string());
    };
    // El guard no debe soltarse al terminar esta función: se mueve al hilo.
    thread::spawn(move || {
        let _guard = _guard;
        match context_from_app(&app) {
            Ok(ctx) => run_app_cycle(&app, &ctx),
            Err(error) => eprintln!("[cef-updater] no se pudo preparar el ciclo: {error}"),
        }
    });
    Ok(())
}

pub fn context_from_app(app: &AppHandle) -> Result<UpdaterContext, String> {
    let paths = CefPaths::from_app(app)?;
    paths.ensure_dirs()?;
    let host_binary = paths::host_binary_path(app)?;
    let host_api_version = paths::base_info().host_api_version;
    let no_sandbox = env_flag("IDIOTEQUE_CEF_NO_SANDBOX")
        || app
            .try_state::<CefState>()
            .map(|state| state.no_sandbox.load(Ordering::SeqCst))
            .unwrap_or(false);
    let (index_url, download_base_url) = resolve_urls();
    Ok(UpdaterContext {
        paths,
        host_binary,
        host_api_version,
        no_sandbox,
        index_url,
        download_base_url,
        platform: PLATFORM.to_string(),
        skip_strip: env_flag("IDIOTEQUE_CEF_SKIP_STRIP"),
    })
}

fn run_app_cycle(app: &AppHandle, ctx: &UpdaterContext) {
    let emit = |event: UpdateEvent| {
        if let Err(error) = app.emit("cef-update", &event) {
            eprintln!("[cef-updater] no se pudo emitir cef-update: {error}");
        }
    };
    let host_alive = || app_host_alive(app);
    let _ = run_cycle(ctx, &host_alive, &emit);
}

fn app_host_alive(app: &AppHandle) -> bool {
    app.try_state::<CefState>()
        .map(|state| state.host_alive())
        .unwrap_or(false)
}

fn check_is_due(paths: &CefPaths) -> bool {
    let state = state::load(paths);
    let interval = env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", CHECK_INTERVAL_SECS);
    is_stale(state.last_check_at.as_deref(), interval)
}

fn is_stale(last_check_at: Option<&str>, interval_secs: u64) -> bool {
    let Some(stamp) = last_check_at else {
        return true;
    };
    let Some(then) = parse_rfc3339_unix(stamp) else {
        return true;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    now.saturating_sub(then) >= interval_secs
}

fn parse_rfc3339_unix(stamp: &str) -> Option<u64> {
    let stamp = stamp.trim();
    let stamp = stamp.strip_suffix('Z').unwrap_or(stamp);
    let (date, time) = stamp.split_once('T')?;
    let mut date = date.split('-');
    let year: i32 = date.next()?.parse().ok()?;
    let month: u32 = date.next()?.parse().ok()?;
    let day: u32 = date.next()?.parse().ok()?;
    let time = time.split(['.', '+']).next()?;
    let mut time = time.split(':');
    let hour: u32 = time.next()?.parse().ok()?;
    let minute: u32 = time.next()?.parse().ok()?;
    let second: u32 = time.next()?.parse().ok()?;
    Some(unix_from_civil(year, month, day, hour, minute, second))
}

fn unix_from_civil(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u64 {
    let mut year = year as i64;
    let month = month as i64;
    let day = day as i64;
    if month <= 2 {
        year -= 1;
    }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u64;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    let days = era * 146_097 + doe as i64 - 719_468;
    days as u64 * 86_400 + u64::from(hour) * 3_600 + u64::from(minute) * 60 + u64::from(second)
}

fn resolve_urls() -> (String, String) {
    let (file_index, file_download) = urls_from_base_json();
    if let Ok(index) = std::env::var("IDIOTEQUE_CEF_INDEX_URL") {
        if !index.trim().is_empty() {
            let download = download_base_from_index(&index);
            return (index, download);
        }
    }
    let index = file_index.unwrap_or_else(|| DEFAULT_INDEX_URL.to_string());
    let download = file_download.unwrap_or_else(|| download_base_from_index(&index));
    (index, download)
}

fn urls_from_base_json() -> (Option<String>, Option<String>) {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        index_url: Option<String>,
        download_base_url: Option<String>,
    }
    match serde_json::from_str::<File>(BASE_JSON) {
        Ok(file) => (file.index_url, file.download_base_url),
        Err(_) => (None, None),
    }
}

fn download_base_from_index(index_url: &str) -> String {
    match index_url.rsplit_once('/') {
        Some((prefix, _)) => {
            if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            }
        }
        None => format!("{index_url}/"),
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_flag(name: &str) -> bool {
    matches!(std::env::var(name), Ok(value) if value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cef::archive::{
        sample_api_versions_h, sample_cef_version_h, write_synthetic_tarball, VERSION_HEADER_SNIPPET,
    };
    use crate::cef::download::sha1_file;
    use crate::cef::manifest::{SlotSource, REQUIRED_FILES_LINUX64};
    use crate::cef::paths::PLATFORM;
    use sha1::{Digest, Sha1};
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    const BUNDLED: &str = "152.0.6+g708dc14+chromium-152.0.7977.83";
    const NEWER: &str = "153.0.1+gabc+chromium-153.0.8000.10";
    const NEWER_CHROMIUM: &str = "153.0.8000.10";
    const ARCHIVE_NAME: &str = "cef_binary_153_linux64_minimal.tar.bz2";

    fn paths_in(tmp: &TempDir) -> CefPaths {
        CefPaths::new(tmp.path().join("home"), tmp.path().join("base"))
    }

    fn write_required_slot(dir: &Path, version: &str, source: SlotSource) {
        use crate::cef::manifest::{self as man, ManifestFile, SlotManifest};
        fs::create_dir_all(dir).unwrap();
        let payload = b"x";
        let mut files = Vec::new();
        for name in REQUIRED_FILES_LINUX64 {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, payload).unwrap();
            files.push(ManifestFile {
                path: (*name).to_string(),
                size: payload.len() as u64,
                sha256: "ab".repeat(32),
            });
        }
        let chromium = crate::cef::version::chromium_from(version).unwrap();
        let manifest = SlotManifest {
            schema: 1,
            cef_version: version.to_string(),
            chromium_version: chromium,
            platform: PLATFORM.to_string(),
            api_version_min: 13300,
            api_version_last: 15200,
            source,
            archive_name: "bundled.tar.bz2".into(),
            archive_sha1: "00".repeat(20),
            archive_size: 1,
            stripped: true,
            files,
            verified: false,
            verified_at: None,
            created_at: "2026-09-16T23:00:00Z".into(),
        };
        man::save(dir, &manifest).unwrap();
    }

    fn tar_path_for_required(name: &str) -> String {
        if name.starts_with("locales/") || name.ends_with(".pak") || name == "icudtl.dat" {
            format!("Resources/{name}")
        } else {
            format!("Release/{name}")
        }
    }

    fn build_runtime_tarball(dest: &Path, api_min: u32, api_last: u32, cef: &str, chromium: &str) {
        let api = sample_api_versions_h(api_min, api_last);
        let ver = sample_cef_version_h(cef, chromium);
        let mut owned: Vec<(String, Vec<u8>, u32)> = Vec::new();
        for name in REQUIRED_FILES_LINUX64 {
            owned.push((tar_path_for_required(name), b"blob".to_vec(), 0o644));
        }
        owned.push((
            "include/cef_api_versions.h".into(),
            api.into_bytes(),
            0o644,
        ));
        owned.push(("include/cef_version.h".into(), ver.into_bytes(), 0o644));
        owned.push(("LICENSE.txt".into(), b"license".to_vec(), 0o644));
        let refs: Vec<(&str, &[u8], u32)> = owned
            .iter()
            .map(|(p, b, m)| (p.as_str(), b.as_slice(), *m))
            .collect();
        write_synthetic_tarball(dest, "cef_binary_test_linux64_minimal", &refs).unwrap();
    }

    fn spawn_http(files: HashMap<String, Vec<u8>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = match stream.read(&mut buf) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let key = path.trim_start_matches('/').to_string();
                if let Some(body) = files.get(&key).or_else(|| files.get(&path)) {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                } else {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        });
        format!("http://{addr}")
    }

    fn index_json(name: &str, sha1: &str, size: u64, cef: &str, chromium: &str) -> String {
        format!(
            r#"{{
              "linux64": {{
                "versions": [
                  {{
                    "cef_version": "{cef}",
                    "chromium_version": "{chromium}",
                    "channel": "stable",
                    "files": [
                      {{"type":"minimal","name":"{name}","sha1":"{sha1}","size":{size}}}
                    ]
                  }}
                ]
              }}
            }}"#
        )
    }

    #[cfg(unix)]
    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    fn setup_ctx(
        tmp: &TempDir,
        index_body: String,
        tarball: Option<(&str, Vec<u8>)>,
        host: PathBuf,
        skip_strip: bool,
    ) -> UpdaterContext {
        let paths = paths_in(tmp);
        paths.ensure_dirs().unwrap();
        write_required_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled);

        let mut files = HashMap::new();
        files.insert("index.json".into(), index_body.into_bytes());
        if let Some((name, bytes)) = tarball {
            files.insert(name.to_string(), bytes);
        }
        let origin = spawn_http(files);
        UpdaterContext {
            paths,
            host_binary: host,
            host_api_version: 15200,
            no_sandbox: true,
            index_url: format!("{origin}/index.json"),
            download_base_url: format!("{origin}/"),
            platform: PLATFORM.to_string(),
            skip_strip,
        }
    }

    fn collect_events() -> (Arc<Mutex<Vec<UpdateEvent>>>, Box<dyn Fn(UpdateEvent)>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let slot = Arc::clone(&events);
        let emit = move |event: UpdateEvent| {
            slot.lock().unwrap().push(event);
        };
        (events, Box::new(emit))
    }

    #[test]
    fn update_event_json_matches_contract() {
        let updated = serde_json::to_value(UpdateEvent::Updated {
            chromium: "153.0.8000.10".into(),
            cef: "153.0.1+…".into(),
        })
        .unwrap();
        assert_eq!(updated["kind"], "updated");
        assert_eq!(updated["chromium"], "153.0.8000.10");
        assert_eq!(updated["cef"], "153.0.1+…");

        let bad = serde_json::to_value(UpdateEvent::Incompatible {
            candidate_chromium: "153.0.8000.10".into(),
            candidate_cef: "153.0.1+…".into(),
            current_chromium: "152.0.7977.83".into(),
            current_cef: "152.0.6+…".into(),
            reason: "health-exit-10".into(),
        })
        .unwrap();
        assert_eq!(bad["kind"], "incompatible");
        assert_eq!(bad["candidateChromium"], "153.0.8000.10");
        assert_eq!(bad["candidateCef"], "153.0.1+…");
        assert_eq!(bad["currentChromium"], "152.0.7977.83");
        assert_eq!(bad["reason"], "health-exit-10");
    }

    #[test]
    fn run_cycle_no_newer() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("x.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let host = tmp.path().join("no-host");
        let ctx = setup_ctx(&tmp, index, None, host, true);
        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        assert_eq!(outcome, CycleOutcome::NoNewer);
        assert!(events.lock().unwrap().is_empty());
        let saved = state::load(&ctx.paths);
        assert_eq!(saved.last_outcome.as_deref(), Some("no-newer"));
        assert!(saved.last_check_at.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_health_exit_10_is_incompatible() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(ARCHIVE_NAME, &sha, bytes.len() as u64, NEWER, NEWER_CHROMIUM);
        let host = write_script(
            tmp.path(),
            "bad-host",
            r#"#!/bin/sh
echo ran > "$(dirname "$0")/ran"
printf '%s\n' '{"event":"fatal","message":"api","code":10}'
exit 10
"#,
        );
        let ctx = setup_ctx(&tmp, index, Some((ARCHIVE_NAME, bytes)), host, true);
        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        match &outcome {
            CycleOutcome::Incompatible { cef, chromium, reason } => {
                assert_eq!(cef, NEWER);
                assert_eq!(chromium, NEWER_CHROMIUM);
                assert_eq!(reason, "health-exit-10");
            }
            other => panic!("expected Incompatible, got {other:?}"),
        }
        assert!(!ctx.paths.candidate().exists());
        let list = denylist::load(&ctx.paths, 15200);
        assert!(list.contains(NEWER));
        assert_eq!(list.entries[0].reason, "health-exit-10");
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UpdateEvent::Incompatible {
                candidate_cef,
                candidate_chromium,
                current_cef,
                current_chromium,
                reason,
            } => {
                assert_eq!(candidate_cef, NEWER);
                assert_eq!(candidate_chromium, NEWER_CHROMIUM);
                assert_eq!(current_cef, BUNDLED);
                assert_eq!(current_chromium, "152.0.7977.83");
                assert_eq!(reason, "health-exit-10");
            }
            other => panic!("bad event {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_api_min_above_host_does_not_spawn() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 99999, 99999, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(ARCHIVE_NAME, &sha, bytes.len() as u64, NEWER, NEWER_CHROMIUM);
        let host = write_script(
            tmp.path(),
            "should-not-run",
            r#"#!/bin/sh
echo ran > "$(dirname "$0")/ran"
exit 0
"#,
        );
        let ctx = setup_ctx(&tmp, index, Some((ARCHIVE_NAME, bytes)), host, true);
        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        match &outcome {
            CycleOutcome::Incompatible { reason, .. } => {
                assert_eq!(reason, "api-version-min-above-host");
            }
            other => panic!("expected Incompatible, got {other:?}"),
        }
        assert!(!tmp.path().join("ran").exists(), "cef-host no debía arrancar");
        assert!(!ctx.paths.candidate().exists());
        let list = denylist::load(&ctx.paths, 15200);
        assert!(list.contains(NEWER));
        assert_eq!(list.entries[0].reason, "api-version-min-above-host");
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_strip_failure_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(ARCHIVE_NAME, &sha, bytes.len() as u64, NEWER, NEWER_CHROMIUM);
        let host = write_script(tmp.path(), "unused-host", "#!/bin/sh\nexit 0\n");
        let ctx = setup_ctx(&tmp, index, Some((ARCHIVE_NAME, bytes)), host, false);
        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        match outcome {
            CycleOutcome::Skipped(reason) => assert!(reason.contains("strip"), "{reason}"),
            other => panic!("expected Skipped, got {other:?}"),
        }
        assert!(events.lock().unwrap().is_empty());
        assert!(!denylist::load(&ctx.paths, 15200).contains(NEWER));
        assert!(!ctx.paths.candidate().exists());
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_health_ok_promotes() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(ARCHIVE_NAME, &sha, bytes.len() as u64, NEWER, NEWER_CHROMIUM);
        let host = write_script(
            tmp.path(),
            "ok-host",
            r#"#!/bin/sh
printf '%s\n' '{"event":"health","ok":true,"cef":"153.0.1+gabc+chromium-153.0.8000.10","chromium":"153.0.8000.10","apiVersion":15200}'
exit 0
"#,
        );
        let ctx = setup_ctx(&tmp, index, Some((ARCHIVE_NAME, bytes)), host, true);
        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        match outcome {
            CycleOutcome::Updated(promoted) => {
                assert_eq!(promoted.cef_version, NEWER);
                assert_eq!(promoted.chromium_version, NEWER_CHROMIUM);
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        assert!(!ctx.paths.candidate().exists());
        assert_eq!(
            manifest::load(&ctx.paths.current()).unwrap().cef_version,
            NEWER
        );
        let captured = events.lock().unwrap().clone();
        match &captured[0] {
            UpdateEvent::Updated { chromium, cef } => {
                assert_eq!(chromium, NEWER_CHROMIUM);
                assert_eq!(cef, NEWER);
            }
            other => panic!("bad event {other:?}"),
        }
    }

    #[test]
    fn download_base_from_index_url() {
        assert_eq!(
            download_base_from_index("https://cef-builds.spotifycdn.com/index.json"),
            "https://cef-builds.spotifycdn.com/"
        );
    }

    #[test]
    fn sample_headers_parse() {
        let _ = VERSION_HEADER_SNIPPET;
        let (min, last) = parse_api_versions(&sample_api_versions_h(13300, 15200)).unwrap();
        assert_eq!((min, last), (13300, 15200));
    }

    #[test]
    fn sha1_hex_smoke() {
        assert_eq!(sha1_hex(b"").len(), 40);
    }
}
