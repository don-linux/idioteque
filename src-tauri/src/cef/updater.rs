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
    Deferred {
        cef: String,
        chromium: String,
    },
    Incompatible {
        cef: String,
        chromium: String,
        reason: String,
    },
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
        log_step(
            &ctx.paths,
            "promoción diferida pendiente y el host no está vivo",
        );
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
                log_step(
                    &ctx.paths,
                    &format!("no se pudo promover lo pendiente: {error}"),
                );
            }
        }
    }

    log_step(&ctx.paths, &format!("consultando índice {}", ctx.index_url));
    let fetched = match fetch_index(&ctx.index_url, state.index_etag.as_deref()) {
        Ok(fetched) => fetched,
        Err(error) => {
            return finish(&ctx.paths, &mut state, CycleOutcome::Skipped(error));
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
            match continue_with_index(ctx, host_alive, emit, &mut state, &mut denylist, &body) {
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
    state: &mut UpdaterState,
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

    // Host vivo + el mismo candidate ya verificado: no llamar a
    // `discard_candidate` (borraría el slot pendiente) ni re-bajar.
    // El motor efectivo sigue siendo el actual/bundle; sin esto cada
    // ciclo (deb/rpm/AppImage) repetiría health del mismo tarball.
    if let Some(pending) = &state.pending_promotion {
        if pending.cef_version == candidate.version.cef_version && host_alive() {
            log_step(
                &ctx.paths,
                "candidato ya pendiente de promoción; se omite la descarga",
            );
            return CycleOutcome::Deferred {
                cef: pending.cef_version.clone(),
                chromium: pending.chromium_version.clone(),
            };
        }
    }

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
        return CycleOutcome::Skipped(format!("No se pudo crear el candidate: {error}"));
    }

    let tarball_in = ctx.paths.candidate().join("download.tar.bz2");
    let url = download_url(&ctx.download_base_url, &candidate.file.name);
    log_step(&ctx.paths, &format!("descargando {url}"));
    if let Err(error) =
        download_verified(&url, &tarball_in, candidate.file.size, &candidate.file.sha1)
    {
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
            return CycleOutcome::Skipped(format!("No se pudo leer cef_api_versions.h: {error}"));
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
                    state.pending_promotion = None;
                    emit(UpdateEvent::Updated {
                        chromium: promoted.chromium_version.clone(),
                        cef: promoted.cef_version.clone(),
                    });
                    log_step(
                        &ctx.paths,
                        &format!("actualizado a Chromium {}", promoted.chromium_version),
                    );
                    CycleOutcome::Updated(promoted)
                }
                Ok(PromoteResult::Deferred(pending)) => {
                    log_step(&ctx.paths, "promoción diferida: cef-host vivo");
                    state.pending_promotion = Some(pending.clone());
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
    let no_sandbox = super::sandbox::wants_no_sandbox(&paths.bundled_base)
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
        let index = index.trim();
        if !index.is_empty() {
            let download = download_base_from_index(index);
            return (index.to_string(), download);
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
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn env_flag(name: &str) -> bool {
    matches!(std::env::var(name), Ok(value) if value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cef::archive::{
        sample_api_versions_h, sample_cef_version_h, write_synthetic_tarball,
        VERSION_HEADER_SNIPPET,
    };
    use crate::cef::download::sha1_file;
    use crate::cef::manifest::{SlotSource, REQUIRED_FILES_LINUX64};
    use crate::cef::paths::PLATFORM;
    use sha1::{Digest, Sha1};
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Barrier, Mutex};
    use tempfile::TempDir;

    /// Serializa tests que mutan env o `CYCLE_RUNNING` (cargo test es paralelo).
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    static CYCLE_LOCK: Mutex<()> = Mutex::new(());

    fn lock(mutex: &'static Mutex<()>) -> std::sync::MutexGuard<'static, ()> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn set_env(key: &str, value: &str) {
        // Edition 2021: `set_var` sigue siendo seguro de nombrar; el proceso
        // de test se serializa con `ENV_LOCK`.
        std::env::set_var(key, value);
    }

    fn remove_env(key: &str) {
        std::env::remove_var(key);
    }

    fn with_env<T>(key: &'static str, value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = lock(&ENV_LOCK);
        let previous = std::env::var(key).ok();
        match value {
            Some(value) => set_env(key, value),
            None => remove_env(key),
        }
        let result = f();
        match previous {
            Some(value) => set_env(key, &value),
            None => remove_env(key),
        }
        result
    }

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
        owned.push(("include/cef_api_versions.h".into(), api.into_bytes(), 0o644));
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
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
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

    fn seed_pending(paths: &CefPaths, etag: Option<&str>) {
        let mut saved = state::load(paths);
        saved.pending_promotion = Some(state::PendingPromotion {
            cef_version: NEWER.into(),
            chromium_version: NEWER_CHROMIUM.into(),
        });
        saved.index_etag = etag.map(str::to_string);
        state::save(paths, &saved).unwrap();
    }

    fn assert_pending_kept(paths: &CefPaths) {
        let saved = state::load(paths);
        let pending = saved
            .pending_promotion
            .as_ref()
            .expect("pendingPromotion debía sobrevivir al persistir state.json (c2a16b5)");
        assert_eq!(pending.cef_version, NEWER);
        assert_eq!(pending.chromium_version, NEWER_CHROMIUM);
        let disk = fs::read_to_string(paths.state_file()).unwrap();
        assert!(
            disk.contains("\"pendingPromotion\""),
            "state.json sin pendingPromotion: {disk}"
        );
        assert!(disk.contains("\"cefVersion\""), "{disk}");
    }

    fn http_header(req: &str, name: &str) -> Option<String> {
        req.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    }

    fn read_http_head(stream: &mut impl Read) -> String {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
            if buf.windows(4).any(|window| window == b"\r\n\r\n") || buf.len() > 16 * 1024 {
                break;
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn write_http(stream: &mut impl Write, status: &str, extra: &str, body: &[u8]) {
        let header = format!(
            "{status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
    }

    /// Índice con ETag. 304 si `If-None-Match` coincide. El tarball 404 para
    /// pillar un ciclo que descargue cuando no debía.
    fn spawn_etag_index(index_body: Vec<u8>, etag: &str, hits: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let etag = etag.to_string();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let req = read_http_head(&mut stream);
                let path = req
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let key = path.trim_start_matches('/').to_string();
                let inm = http_header(&req, "If-None-Match").unwrap_or_default();
                hits.lock().unwrap().push(format!("{key}|inm={inm}"));
                if key == "index.json" {
                    if !inm.is_empty() && inm == etag {
                        write_http(
                            &mut stream,
                            "HTTP/1.1 304 Not Modified",
                            &format!("ETag: {etag}\r\n"),
                            b"",
                        );
                    } else {
                        write_http(
                            &mut stream,
                            "HTTP/1.1 200 OK",
                            &format!("ETag: {etag}\r\nContent-Type: application/json\r\n"),
                            &index_body,
                        );
                    }
                } else {
                    write_http(&mut stream, "HTTP/1.1 404 Not Found", "", b"");
                }
            }
        });
        format!("http://{addr}")
    }

    fn ctx_at_origin(tmp: &TempDir, origin: &str, host: PathBuf) -> UpdaterContext {
        let paths = paths_in(tmp);
        paths.ensure_dirs().unwrap();
        write_required_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled);
        UpdaterContext {
            paths,
            host_binary: host,
            host_api_version: 15200,
            no_sandbox: true,
            index_url: format!("{origin}/index.json"),
            download_base_url: format!("{origin}/"),
            platform: PLATFORM.to_string(),
            skip_strip: true,
        }
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
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
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
            CycleOutcome::Incompatible {
                cef,
                chromium,
                reason,
            } => {
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
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
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
        assert!(
            !tmp.path().join("ran").exists(),
            "cef-host no debía arrancar"
        );
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
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
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
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
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

    #[cfg(unix)]
    #[test]
    fn run_cycle_health_ok_defers_when_host_alive() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
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
        let outcome = run_cycle(&ctx, &|| true, emit.as_ref());
        match outcome {
            CycleOutcome::Deferred { cef, chromium } => {
                assert_eq!(cef, NEWER);
                assert_eq!(chromium, NEWER_CHROMIUM);
            }
            other => panic!("expected Deferred, got {other:?}"),
        }
        assert!(ctx.paths.candidate().exists());
        assert!(!ctx.paths.current().exists());
        assert!(manifest::load(&ctx.paths.candidate()).unwrap().verified);
        let pending = state::load(&ctx.paths).pending_promotion.expect("pending");
        assert_eq!(pending.cef_version, NEWER);
        assert_eq!(
            state::load(&ctx.paths).last_outcome.as_deref(),
            Some("deferred")
        );
        assert_pending_kept(&ctx.paths);
        assert!(events.lock().unwrap().is_empty());
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

    #[test]
    fn try_begin_cycle_serializes_and_releases() {
        let _lock = lock(&CYCLE_LOCK);
        let first = try_begin_cycle().expect("primer ciclo");
        assert!(
            try_begin_cycle().is_none(),
            "scheduler y cef_check_updates no pueden solaparse"
        );
        drop(first);
        assert!(try_begin_cycle().is_some());
    }

    #[test]
    fn try_begin_cycle_releases_after_panic() {
        let _lock = lock(&CYCLE_LOCK);
        let exploded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = try_begin_cycle().expect("ciclo");
            panic!("el ciclo del updater reventó");
        }));
        assert!(exploded.is_err());
        assert!(
            try_begin_cycle().is_some(),
            "CycleGuard debe soltar CYCLE_RUNNING aunque el hilo pánico"
        );
    }

    #[test]
    fn try_begin_cycle_single_winner_under_contention() {
        let _lock = lock(&CYCLE_LOCK);
        let barrier = Arc::new(Barrier::new(8));
        let wins = Arc::new(AtomicUsize::new(0));
        thread::scope(|scope| {
            for _ in 0..8 {
                let barrier = Arc::clone(&barrier);
                let wins = Arc::clone(&wins);
                scope.spawn(move || {
                    barrier.wait();
                    if let Some(_guard) = try_begin_cycle() {
                        wins.fetch_add(1, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(15));
                    }
                });
            }
        });
        assert_eq!(wins.load(Ordering::SeqCst), 1);
        assert!(try_begin_cycle().is_some());
    }

    #[test]
    fn run_cycle_etag_304_is_not_modified_and_does_not_download() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("must-not-fetch.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let etag = "\"linux-mirror-1\"";
        let hits = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_etag_index(index.into_bytes(), etag, Arc::clone(&hits));
        let ctx = ctx_at_origin(&tmp, &origin, tmp.path().join("no-host"));

        let (events, emit) = collect_events();
        let first = run_cycle(&ctx, &|| false, emit.as_ref());
        assert_eq!(first, CycleOutcome::NoNewer);
        assert_eq!(state::load(&ctx.paths).index_etag.as_deref(), Some(etag));

        let second = run_cycle(&ctx, &|| false, emit.as_ref());
        assert_eq!(second, CycleOutcome::NotModified);
        assert_eq!(
            state::load(&ctx.paths).last_outcome.as_deref(),
            Some("not-modified")
        );
        assert!(state::load(&ctx.paths).last_check_at.is_some());
        assert!(events.lock().unwrap().is_empty());

        let hits = hits.lock().unwrap().clone();
        assert!(
            hits.iter()
                .any(|hit| hit.starts_with("index.json|inm=") && hit.contains(etag)),
            "segundo GET debía mandar If-None-Match: {hits:?}"
        );
        assert!(
            hits.iter().all(|hit| !hit.starts_with("must-not-fetch")),
            "304 no debe tocar el tarball (deb/rpm/AppImage usan el mismo índice): {hits:?}"
        );
    }

    #[test]
    fn run_cycle_etag_304_does_not_clear_pending_promotion() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("x.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let etag = "\"etag-pending\"";
        let hits = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_etag_index(index.into_bytes(), etag, hits);
        let ctx = ctx_at_origin(&tmp, &origin, tmp.path().join("no-host"));
        seed_pending(&ctx.paths, Some(etag));

        let (events, emit) = collect_events();
        // Host vivo: no promover lo pendiente; el 304 solo refresca lastCheckAt.
        let outcome = run_cycle(&ctx, &|| true, emit.as_ref());
        assert_eq!(outcome, CycleOutcome::NotModified);
        assert_eq!(
            state::load(&ctx.paths).last_outcome.as_deref(),
            Some("not-modified")
        );
        assert_pending_kept(&ctx.paths);
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn run_cycle_no_newer_and_index_error_keep_pending_promotion() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("x.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let ctx = setup_ctx(&tmp, index, None, tmp.path().join("no-host"), true);
        seed_pending(&ctx.paths, None);

        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| true, emit.as_ref());
        assert_eq!(outcome, CycleOutcome::NoNewer);
        assert_pending_kept(&ctx.paths);

        let mut dead = ctx.clone();
        dead.index_url = "http://127.0.0.1:1/index.json".into();
        let skipped = run_cycle(&dead, &|| true, emit.as_ref());
        match skipped {
            CycleOutcome::Skipped(reason) => {
                assert!(
                    reason.contains("índice")
                        || reason.contains("index")
                        || reason.contains("conectar")
                        || reason.contains("Connection")
                        || reason.contains("os error")
                        || reason.contains("error"),
                    "{reason}"
                );
            }
            other => panic!("expected Skipped, got {other:?}"),
        }
        assert_pending_kept(&dead.paths);
        assert!(
            state::load(&dead.paths)
                .last_outcome
                .as_deref()
                .unwrap_or("")
                .starts_with("skipped:"),
            "{:?}",
            state::load(&dead.paths).last_outcome
        );
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn run_cycle_empty_etag_is_treated_as_absent() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("x.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let etag = "\"real-etag\"";
        let hits = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_etag_index(index.into_bytes(), etag, Arc::clone(&hits));
        let ctx = ctx_at_origin(&tmp, &origin, tmp.path().join("no-host"));
        let mut saved = state::load(&ctx.paths);
        saved.index_etag = Some(String::new());
        state::save(&ctx.paths, &saved).unwrap();

        let (_events, emit) = collect_events();
        assert_eq!(
            run_cycle(&ctx, &|| false, emit.as_ref()),
            CycleOutcome::NoNewer
        );
        assert_eq!(state::load(&ctx.paths).index_etag.as_deref(), Some(etag));
        let hits = hits.lock().unwrap().clone();
        assert!(
            hits.iter().any(|hit| hit == "index.json|inm="),
            "etag vacío no debe mandar If-None-Match: {hits:?}"
        );
    }

    #[test]
    fn run_cycle_304_after_early_promote_reports_updated() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("x.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let etag = "\"after-promote\"";
        let hits = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_etag_index(index.into_bytes(), etag, hits);
        let ctx = ctx_at_origin(&tmp, &origin, tmp.path().join("no-host"));
        write_required_slot(&ctx.paths.candidate(), NEWER, SlotSource::Downloaded);
        mark_candidate_verified(&ctx.paths).unwrap();
        seed_pending(&ctx.paths, Some(etag));

        let (events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        match outcome {
            CycleOutcome::Updated(promoted) => {
                assert_eq!(promoted.cef_version, NEWER);
                assert_eq!(promoted.chromium_version, NEWER_CHROMIUM);
            }
            other => panic!("expected Updated after early promote + 304, got {other:?}"),
        }
        assert!(!ctx.paths.candidate().exists());
        assert_eq!(
            manifest::load(&ctx.paths.current()).unwrap().cef_version,
            NEWER
        );
        assert!(state::load(&ctx.paths).pending_promotion.is_none());
        assert_eq!(
            state::load(&ctx.paths).last_outcome.as_deref(),
            Some("updated")
        );
        let captured = events.lock().unwrap().clone();
        match captured.as_slice() {
            [UpdateEvent::Updated { cef, chromium }] => {
                assert_eq!(cef, NEWER);
                assert_eq!(chromium, NEWER_CHROMIUM);
            }
            other => panic!("expected single updated event, got {other:?}"),
        }
    }

    #[test]
    fn run_cycle_stale_pending_without_candidate_clears_when_host_dead() {
        let tmp = TempDir::new().unwrap();
        let index = index_json("x.tar.bz2", "aa", 1, BUNDLED, "152.0.7977.83");
        let ctx = setup_ctx(&tmp, index, None, tmp.path().join("no-host"), true);
        seed_pending(&ctx.paths, None);
        assert!(!ctx.paths.candidate().exists());

        let (_events, emit) = collect_events();
        let outcome = run_cycle(&ctx, &|| false, emit.as_ref());
        assert_eq!(outcome, CycleOutcome::NoNewer);
        assert!(
            state::load(&ctx.paths).pending_promotion.is_none(),
            "NothingToPromote debe limpiar un pending huérfano"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_deferred_second_cycle_does_not_redownload() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
        let host = write_script(
            tmp.path(),
            "ok-host-defer",
            r#"#!/bin/sh
echo x >> "$(dirname "$0")/health-runs"
printf '%s\n' '{"event":"health","ok":true,"cef":"153.0.1+gabc+chromium-153.0.8000.10","chromium":"153.0.8000.10","apiVersion":15200}'
exit 0
"#,
        );
        let mut files = HashMap::new();
        files.insert("index.json".into(), index.into_bytes());
        files.insert(ARCHIVE_NAME.to_string(), bytes);
        let origin = spawn_http(files);
        let mut ctx = ctx_at_origin(&tmp, &origin, host);
        ctx.download_base_url = format!("{origin}/");

        let (events, emit) = collect_events();
        match run_cycle(&ctx, &|| true, emit.as_ref()) {
            CycleOutcome::Deferred { cef, .. } => assert_eq!(cef, NEWER),
            other => panic!("expected Deferred, got {other:?}"),
        }
        assert_pending_kept(&ctx.paths);
        let etag = state::load(&ctx.paths).index_etag;
        assert!(
            etag.is_none(),
            "spawn_http no envía ETag; el persist no debe inventar uno"
        );
        let runs = tmp.path().join("health-runs");
        assert_eq!(fs::read_to_string(&runs).unwrap().matches('x').count(), 1);

        // Mismo índice + host vivo: no re-bajar ni discard del candidate verificado.
        // c2a16b5: finish() no puede reescribir pendingPromotion a null.
        let again = run_cycle(&ctx, &|| true, emit.as_ref());
        match again {
            CycleOutcome::Deferred { cef, chromium } => {
                assert_eq!(cef, NEWER);
                assert_eq!(chromium, NEWER_CHROMIUM);
            }
            other => panic!("expected Deferred (no re-download), got {other:?}"),
        }
        assert_eq!(
            state::load(&ctx.paths).last_outcome.as_deref(),
            Some("deferred")
        );
        assert_pending_kept(&ctx.paths);
        assert!(ctx.paths.candidate().exists());
        assert!(manifest::load(&ctx.paths.candidate()).unwrap().verified);
        assert_eq!(
            fs::read_to_string(&runs).unwrap().matches('x').count(),
            1,
            "segundo ciclo no debe repetir el health check"
        );
        assert!(events.lock().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_health_exits_12_through_16_are_incompatible() {
        let archive = TempDir::new().unwrap();
        let tarball_path = archive.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();

        // 12 watchdog health, 13 version mismatch, 14 bad slot, 15 sandbox
        // (SUID o userns; AppImage suele ir sin helper), 16 no X11 (sesión
        // nativa, XWayland o AppImage sin DISPLAY). No son códigos de Ubuntu.
        for code in [12, 13, 14, 15, 16] {
            let tmp = TempDir::new().unwrap();
            let host = write_script(
                tmp.path(),
                &format!("host-{code}"),
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' '{{\"event\":\"fatal\",\"message\":\"e\",\"code\":{code}}}'\nexit {code}\n"
                ),
            );
            let index = index_json(
                ARCHIVE_NAME,
                &sha,
                bytes.len() as u64,
                NEWER,
                NEWER_CHROMIUM,
            );
            let ctx = setup_ctx(&tmp, index, Some((ARCHIVE_NAME, bytes.clone())), host, true);
            let (events, emit) = collect_events();
            let reason = format!("health-exit-{code}");
            match run_cycle(&ctx, &|| false, emit.as_ref()) {
                CycleOutcome::Incompatible {
                    cef,
                    chromium,
                    reason: got,
                } => {
                    assert_eq!(cef, NEWER);
                    assert_eq!(chromium, NEWER_CHROMIUM);
                    assert_eq!(got, reason);
                }
                other => panic!("exit {code}: expected Incompatible, got {other:?}"),
            }
            assert!(!ctx.paths.candidate().exists(), "exit {code}");
            let list = denylist::load(&ctx.paths, 15200);
            assert!(list.contains(NEWER), "exit {code}");
            assert_eq!(list.entries[0].reason, reason, "exit {code}");
            assert_eq!(
                state::load(&ctx.paths).last_outcome.as_deref(),
                Some(format!("incompatible:{reason}").as_str()),
                "exit {code}"
            );
            let captured = events.lock().unwrap().clone();
            match captured.as_slice() {
                [UpdateEvent::Incompatible {
                    reason: event_reason,
                    ..
                }] => assert_eq!(event_reason, &reason),
                other => panic!("exit {code}: bad event {other:?}"),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_cycle_health_ok_then_exit_16_is_still_incompatible() {
        let tmp = TempDir::new().unwrap();
        let tarball_path = tmp.path().join(ARCHIVE_NAME);
        build_runtime_tarball(&tarball_path, 13300, 15200, NEWER, NEWER_CHROMIUM);
        let bytes = fs::read(&tarball_path).unwrap();
        let sha = sha1_file(&tarball_path).unwrap();
        let index = index_json(
            ARCHIVE_NAME,
            &sha,
            bytes.len() as u64,
            NEWER,
            NEWER_CHROMIUM,
        );
        let host = write_script(
            tmp.path(),
            "health-then-no-x11",
            r#"#!/bin/sh
printf '%s\n' '{"event":"health","ok":true,"cef":"153.0.1+gabc+chromium-153.0.8000.10","chromium":"153.0.8000.10","apiVersion":15200}'
exit 16
"#,
        );
        let ctx = setup_ctx(&tmp, index, Some((ARCHIVE_NAME, bytes)), host, true);
        let (events, emit) = collect_events();
        match run_cycle(&ctx, &|| false, emit.as_ref()) {
            CycleOutcome::Incompatible { reason, .. } => {
                assert_eq!(reason, "health-exit-16");
            }
            other => panic!("expected Incompatible, got {other:?}"),
        }
        assert!(!ctx.paths.candidate().exists());
        assert!(denylist::load(&ctx.paths, 15200).contains(NEWER));
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[test]
    fn is_stale_none_garbage_future_and_interval_zero() {
        assert!(is_stale(None, CHECK_INTERVAL_SECS));
        assert!(is_stale(Some(""), CHECK_INTERVAL_SECS));
        assert!(is_stale(Some("not-a-date"), CHECK_INTERVAL_SECS));
        assert!(is_stale(Some("2026-09-16 23:00:00Z"), CHECK_INTERVAL_SECS));
        assert!(is_stale(Some("2026-09-16t23:00:00Z"), CHECK_INTERVAL_SECS));

        let now = state::now_rfc3339();
        assert!(!is_stale(Some(&now), CHECK_INTERVAL_SECS));
        assert!(is_stale(Some(&now), 0), "intervalo 0 ⇒ siempre due");
        assert!(is_stale(Some("2020-01-01T00:00:00Z"), 86_400));
        assert!(!is_stale(Some("2099-01-01T00:00:00Z"), 86_400));
        assert!(is_stale(Some("2099-01-01T00:00:00Z"), 0));
    }

    #[test]
    fn parse_rfc3339_unix_fractional_offset_and_leap_day() {
        assert_eq!(
            parse_rfc3339_unix("2024-01-01T00:00:00Z"),
            Some(1_704_067_200)
        );
        assert_eq!(
            parse_rfc3339_unix("2024-01-01T00:00:00.500Z"),
            Some(1_704_067_200)
        );
        assert_eq!(
            parse_rfc3339_unix("2024-01-01T00:00:00+00:00"),
            Some(1_704_067_200)
        );
        assert_eq!(
            parse_rfc3339_unix(" 2024-01-01T00:00:00Z "),
            Some(1_704_067_200)
        );
        let leap = parse_rfc3339_unix("2024-02-29T00:00:00Z").expect("leap");
        assert!(leap > 1_704_067_200);
        assert!(parse_rfc3339_unix("2024-01-01").is_none());
    }

    #[test]
    fn env_u64_and_env_flag_are_strict_except_whitespace() {
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", None, || {
            assert_eq!(
                env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", CHECK_INTERVAL_SECS),
                CHECK_INTERVAL_SECS
            );
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some("90"), || {
            assert_eq!(env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", 1), 90);
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some(" 45 "), || {
            assert_eq!(
                env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", CHECK_INTERVAL_SECS),
                45,
                "systemd/AppImage env a menudo trae espacios o newline"
            );
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some(""), || {
            assert_eq!(env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", 7), 7);
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some("nope"), || {
            assert_eq!(env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", 7), 7);
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some("-1"), || {
            assert_eq!(env_u64("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", 7), 7);
        });
        with_env("IDIOTEQUE_CEF_STARTUP_DELAY_SECS", Some("0"), || {
            assert_eq!(
                env_u64("IDIOTEQUE_CEF_STARTUP_DELAY_SECS", STARTUP_DELAY_SECS),
                0
            );
        });

        with_env("IDIOTEQUE_CEF_SKIP_STRIP", None, || {
            assert!(!env_flag("IDIOTEQUE_CEF_SKIP_STRIP"));
        });
        with_env("IDIOTEQUE_CEF_SKIP_STRIP", Some("1"), || {
            assert!(env_flag("IDIOTEQUE_CEF_SKIP_STRIP"));
        });
        for value in ["0", "true", "yes", "1 ", " 1"] {
            with_env("IDIOTEQUE_CEF_SKIP_STRIP", Some(value), || {
                assert!(
                    !env_flag("IDIOTEQUE_CEF_SKIP_STRIP"),
                    "solo el literal 1 activa el flag, got {value:?}"
                );
            });
        }
    }

    #[test]
    fn check_is_due_honors_interval_env() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        paths.ensure_dirs().unwrap();
        assert!(check_is_due(&paths), "sin lastCheckAt ⇒ due");

        let mut saved = state::load(&paths);
        saved.last_check_at = Some(state::now_rfc3339());
        state::save(&paths, &saved).unwrap();

        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some("86400"), || {
            assert!(!check_is_due(&paths));
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some("0"), || {
            assert!(check_is_due(&paths));
        });
        with_env("IDIOTEQUE_CEF_CHECK_INTERVAL_SECS", Some("garbage"), || {
            assert!(
                !check_is_due(&paths),
                "basura ⇒ default 24h; lastCheckAt reciente no está vencido"
            );
        });
    }

    #[test]
    fn resolve_urls_env_overrides_and_trims() {
        with_env("IDIOTEQUE_CEF_INDEX_URL", None, || {
            let (index, download) = resolve_urls();
            assert_eq!(index, "https://cef-builds.spotifycdn.com/index.json");
            assert_eq!(download, "https://cef-builds.spotifycdn.com/");
        });
        with_env(
            "IDIOTEQUE_CEF_INDEX_URL",
            Some("https://mirror.example/cef/index.json"),
            || {
                let (index, download) = resolve_urls();
                assert_eq!(index, "https://mirror.example/cef/index.json");
                assert_eq!(
                    download, "https://mirror.example/cef/",
                    "el env no usa downloadBaseUrl de base.json"
                );
            },
        );
        with_env(
            "IDIOTEQUE_CEF_INDEX_URL",
            Some("  https://mirror.example/cef/index.json  "),
            || {
                let (index, download) = resolve_urls();
                assert_eq!(index, "https://mirror.example/cef/index.json");
                assert_eq!(download, "https://mirror.example/cef/");
            },
        );
        with_env("IDIOTEQUE_CEF_INDEX_URL", Some("   "), || {
            let (index, _) = resolve_urls();
            assert_eq!(index, "https://cef-builds.spotifycdn.com/index.json");
        });
    }

    #[test]
    fn download_base_from_index_adversarial() {
        assert_eq!(download_base_from_index("index.json"), "index.json/");
        assert_eq!(download_base_from_index(""), "/");
        assert_eq!(
            download_base_from_index("https://cdn.example/cef/"),
            "https://cdn.example/cef/"
        );
        assert_eq!(
            download_base_from_index("https://cdn.example/cef//index.json"),
            "https://cdn.example/cef/"
        );
    }
}
