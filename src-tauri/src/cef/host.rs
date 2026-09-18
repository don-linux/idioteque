//! Spawn y protocolo de `cef-host`. Sin tipos Tauri en `spawn_host` / `HostProcess`.

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::ipc::{encode_command, parse_event, HostCommand, HostEvent};
use super::manifest::{self, EffectiveSource};
use super::paths::{self, CefPaths};

const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_OS_BOUNDS: (i32, i32, i32, i32) = (0, 0, 1200, 800);
const DEFAULT_SCALE: f64 = 1.0;
const NO_DISPLAY: &str = "sin compositor Wayland";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBoot {
    pub cef: String,
    pub chromium: String,
    pub api_version: u32,
    pub source: String,
    pub no_sandbox: bool,
}

#[derive(Debug, Clone)]
pub struct HostLaunch {
    pub binary: PathBuf,
    pub cef_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub bounds: Option<(i32, i32, i32, i32)>,
    pub scale: f64,
    pub url: String,
    pub health_check: bool,
    pub no_sandbox: bool,
    /// `--idq-log`: Chromium lo trunca al arrancar. Distinto de `stderr_file`.
    pub log_file: Option<PathBuf>,
    /// stderr del host (fatals). Append; no es el `log_file` de Chromium.
    pub stderr_file: Option<PathBuf>,
}

pub struct HostProcess {
    child: Arc<Mutex<Child>>,
    stdin: Mutex<ChildStdin>,
    events: Option<Receiver<HostEvent>>,
    #[allow(dead_code)]
    pid: u32,
}

pub struct CefState {
    host: Mutex<Option<HostProcess>>,
    pub no_sandbox: AtomicBool,
}

impl Default for CefState {
    fn default() -> Self {
        Self {
            host: Mutex::new(None),
            no_sandbox: AtomicBool::new(false),
        }
    }
}

impl CefState {
    pub fn host_alive(&self) -> bool {
        match self.host.lock() {
            Ok(mut guard) => match guard.as_mut() {
                Some(host) => host.is_alive(),
                None => false,
            },
            Err(_) => false,
        }
    }

    #[cfg(test)]
    fn set_host_for_test(&self, process: HostProcess) {
        *self.host.lock().expect("host lock") = Some(process);
    }
}

pub fn spawn_host(launch: &HostLaunch) -> Result<HostProcess, String> {
    let mut cmd = Command::new(&launch.binary);
    cmd.arg("--idq-cef-dir").arg(&launch.cef_dir);
    cmd.arg("--idq-cache-dir").arg(&launch.cache_dir);

    if let Some((x, y, w, h)) = launch.bounds {
        cmd.arg("--idq-bounds").arg(format!("{x},{y},{w},{h}"));
    }
    cmd.arg("--idq-scale").arg(launch.scale.to_string());
    let url = if launch.url.trim().is_empty() {
        "about:blank"
    } else {
        launch.url.as_str()
    };
    cmd.arg("--idq-url").arg(url);
    if launch.health_check {
        cmd.arg("--idq-health-check");
    }
    if launch.no_sandbox {
        cmd.arg("--idq-no-sandbox");
    }
    if let Some(log) = &launch.log_file {
        cmd.arg("--idq-log").arg(log);
    }

    prepend_lib_path(&mut cmd, &launch.cef_dir);
    super::sandbox::apply_devel_sandbox_env(&mut cmd, &launch.cef_dir);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(stderr_stdio(launch.stderr_file.as_ref())?);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd
        .spawn()
        .map_err(|error| format!("No se pudo lanzar cef-host: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "cef-host no tiene stdout".to_string())?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "cef-host no tiene stdin".to_string())?;
    let pid = child.id();
    let child = Arc::new(Mutex::new(child));
    let reader_child = Arc::clone(&child);
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            if line.trim().is_empty() {
                continue;
            }
            match parse_event(&line) {
                Ok(event) => {
                    if tx.send(event).is_err() {
                        break;
                    }
                }
                Err(error) => eprintln!("[cef] {error}"),
            }
        }

        let code = loop {
            let mut guard = match reader_child.lock() {
                Ok(guard) => guard,
                Err(_) => break 1,
            };
            match guard.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(1),
                Ok(None) => {
                    drop(guard);
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break 1,
            }
        };
        let _ = tx.send(HostEvent::Exit { code });
    });

    Ok(HostProcess {
        child,
        stdin: Mutex::new(stdin),
        events: Some(rx),
        pid,
    })
}

impl HostProcess {
    /// Extrae el receptor de eventos (una sola vez).
    pub fn take_events(&mut self) -> Receiver<HostEvent> {
        self.events
            .take()
            .expect("los eventos de cef-host ya se están leyendo")
    }

    pub fn send(&self, cmd: &HostCommand) -> Result<(), String> {
        let mut stdin = self.stdin.lock().map_err(|error| error.to_string())?;
        let encoded = encode_command(cmd);
        stdin
            .write_all(encoded.as_bytes())
            .map_err(|error| format!("No se pudo enviar el comando al navegador: {error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("No se pudo enviar el comando al navegador: {error}"))
    }

    pub fn is_alive(&mut self) -> bool {
        let mut child = match self.child.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        }
    }

    pub fn kill_graceful(&mut self, timeout: Duration) {
        let _ = self.send(&HostCommand::Close);
        let deadline = Instant::now() + timeout;
        while self.is_alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.is_alive() {
            if let Ok(mut child) = self.child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    #[allow(dead_code)]
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

pub fn kill_on_exit(state: &CefState) {
    if let Ok(mut guard) = state.host.lock() {
        if let Some(mut host) = guard.take() {
            drop(guard);
            host.kill_graceful(GRACEFUL_TIMEOUT);
        }
    }
}

#[tauri::command]
pub fn browser_spawn(
    app: AppHandle,
    state: State<CefState>,
    url: String,
    bounds: Option<Bounds>,
    scale: Option<f64>,
    on_event: Channel<HostEvent>,
) -> Result<BrowserBoot, String> {
    require_wayland_display()?;
    take_and_kill(&state)?;

    let paths = CefPaths::from_app(&app)?;
    paths.ensure_dirs()?;
    let slot = manifest::resolve_effective(&paths)?;
    let binary = paths::host_binary_path(&app)?;
    let scale = resolve_scale(scale);
    let os_bounds = resolve_os_bounds(bounds.as_ref(), Some(scale));

    let no_sandbox =
        super::sandbox::wants_no_sandbox(&slot.dir) || state.no_sandbox.load(Ordering::SeqCst);
    if no_sandbox {
        state.no_sandbox.store(true, Ordering::SeqCst);
    }

    let launch = HostLaunch {
        binary,
        cef_dir: slot.dir.clone(),
        cache_dir: paths.profile(),
        bounds: Some(os_bounds),
        scale,
        url,
        health_check: false,
        no_sandbox,
        log_file: Some(paths.logs_dir().join("chromium.log")),
        stderr_file: Some(paths.logs_dir().join("cef-host.log")),
    };

    let mut process = spawn_host(&launch)?;
    let events = process.take_events();
    {
        let mut guard = state.host.lock().map_err(|error| error.to_string())?;
        *guard = Some(process);
    }

    start_forward_thread(app.clone(), on_event, events, launch);

    let source = match slot.source {
        EffectiveSource::Bundled => "bundled",
        EffectiveSource::Installed => "installed",
    };

    Ok(BrowserBoot {
        cef: slot.manifest.cef_version,
        chromium: slot.manifest.chromium_version,
        api_version: paths::base_info().host_api_version,
        source: source.to_string(),
        no_sandbox,
    })
}

#[tauri::command]
pub fn browser_command(state: State<CefState>, cmd: HostCommand) -> Result<(), String> {
    with_host(&state, |host| host.send(&cmd))
}

#[tauri::command]
pub fn browser_set_bounds(
    state: State<CefState>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale: f64,
) -> Result<(), String> {
    let (px, py, pw, ph) = physical_bounds(x, y, w, h, scale);
    with_host(&state, |host| {
        host.send(&HostCommand::SetBounds {
            x: px,
            y: py,
            w: pw.max(1),
            h: ph.max(1),
        })
    })
}

#[tauri::command]
pub fn browser_set_visible(state: State<CefState>, visible: bool) -> Result<(), String> {
    let cmd = if visible {
        HostCommand::Show
    } else {
        HostCommand::Hide
    };
    with_host(&state, |host| host.send(&cmd))
}

#[tauri::command]
pub fn browser_kill(app: AppHandle, state: State<CefState>) -> Result<(), String> {
    let result = take_and_kill(&state);
    promote_pending_after_close(&app);
    result
}

/// Un candidate que pasó el health check mientras el navegador estaba abierto
/// se promueve ahora que el host ya no corre (CONTRACT 8).
fn promote_pending_after_close(app: &AppHandle) {
    use tauri::Emitter;

    let Ok(paths) = CefPaths::from_app(app) else {
        return;
    };
    if super::state::load(&paths).pending_promotion.is_none() {
        return;
    }
    match super::promote::promote_candidate(&paths, false) {
        Ok(super::promote::PromoteResult::Promoted(promoted)) => {
            let _ = app.emit(
                "cef-update",
                super::updater::UpdateEvent::Updated {
                    chromium: promoted.chromium_version,
                    cef: promoted.cef_version,
                },
            );
        }
        Ok(_) => {}
        Err(error) => eprintln!("[cef] promoción pendiente: {error}"),
    }
}

fn start_forward_thread(
    app: AppHandle,
    on_event: Channel<HostEvent>,
    events: Receiver<HostEvent>,
    launch: HostLaunch,
) {
    std::thread::spawn(move || {
        let state = app.state::<CefState>();
        pump_host_events(
            events,
            &launch,
            |retry| {
                eprintln!("[cef] sandbox no disponible, reintentando con --idq-no-sandbox");
                state.no_sandbox.store(true, Ordering::SeqCst);
                let mut process = spawn_host(retry)?;
                let next = process.take_events();
                if let Ok(mut guard) = state.host.lock() {
                    *guard = Some(process);
                }
                Ok(next)
            },
            |event| on_event.send(event).is_ok(),
        );
    });
}

/// Recorre eventos del host. El `fatal` del primer intento no llega al
/// Channel si todavía se puede reintentar sin sandbox (contrato 5).
fn pump_host_events(
    mut events: Receiver<HostEvent>,
    launch: &HostLaunch,
    mut spawn_retry: impl FnMut(&HostLaunch) -> Result<Receiver<HostEvent>, String>,
    mut emit: impl FnMut(HostEvent) -> bool,
) {
    use super::sandbox::{forward_action, ForwardAction};

    let mut saw_ready = false;
    let mut retried = false;
    let mut held_fatal: Option<HostEvent> = None;

    loop {
        let event = match events.recv() {
            Ok(event) => event,
            Err(_) => break,
        };

        match forward_action(&event, saw_ready, retried, launch.no_sandbox) {
            ForwardAction::Send => {
                if matches!(event, HostEvent::Ready { .. }) {
                    saw_ready = true;
                }
                if let HostEvent::Shortcut { chord } = &event {
                    eprintln!("[cef] shortcut forwarded {chord}");
                }
                if !emit(event) {
                    break;
                }
            }
            ForwardAction::HoldFatal => {
                held_fatal = Some(event);
            }
            ForwardAction::Drop => {}
            ForwardAction::RetrySandbox => {
                let mut retry = launch.clone();
                retry.no_sandbox = true;
                match spawn_retry(&retry) {
                    Ok(next) => {
                        retried = true;
                        held_fatal = None;
                        events = next;
                    }
                    Err(error) => {
                        eprintln!("[cef] reintento sin sandbox falló: {error}");
                        if let Some(fatal) = held_fatal.take() {
                            if !emit(fatal) {
                                break;
                            }
                        }
                        let _ = emit(event);
                        break;
                    }
                }
            }
            ForwardAction::FlushFatalThenSend => {
                if let Some(fatal) = held_fatal.take() {
                    if !emit(fatal) {
                        break;
                    }
                }
                if !emit(event) {
                    break;
                }
            }
        }
    }
}

fn take_and_kill(state: &CefState) -> Result<(), String> {
    let existing = {
        let mut guard = state.host.lock().map_err(|error| error.to_string())?;
        guard.take()
    };
    if let Some(mut host) = existing {
        host.kill_graceful(GRACEFUL_TIMEOUT);
    }
    Ok(())
}

fn with_host<T>(
    state: &CefState,
    f: impl FnOnce(&HostProcess) -> Result<T, String>,
) -> Result<T, String> {
    let guard = state.host.lock().map_err(|error| error.to_string())?;
    let host = guard
        .as_ref()
        .ok_or_else(|| "No hay un navegador en ejecución".to_string())?;
    f(host)
}

fn physical_bounds(x: f64, y: f64, w: f64, h: f64, scale: f64) -> (i32, i32, i32, i32) {
    (
        (x * scale).round() as i32,
        (y * scale).round() as i32,
        (w * scale).round() as i32,
        (h * scale).round() as i32,
    )
}

fn resolve_scale(scale: Option<f64>) -> f64 {
    scale.unwrap_or(DEFAULT_SCALE)
}

fn resolve_os_bounds(bounds: Option<&Bounds>, scale: Option<f64>) -> (i32, i32, i32, i32) {
    match bounds {
        Some(bounds) => {
            let scale = resolve_scale(scale);
            let (x, y, w, h) = physical_bounds(bounds.x, bounds.y, bounds.w, bounds.h, scale);
            (x, y, w.max(1), h.max(1))
        }
        None => DEFAULT_OS_BOUNDS,
    }
}

fn has_wayland_display(value: Option<&OsStr>) -> bool {
    matches!(value, Some(value) if !value.is_empty())
}

fn require_wayland_display() -> Result<(), String> {
    if has_wayland_display(std::env::var_os("WAYLAND_DISPLAY").as_deref()) {
        Ok(())
    } else {
        Err(NO_DISPLAY.to_string())
    }
}

fn stderr_stdio(log_file: Option<&PathBuf>) -> Result<Stdio, String> {
    match log_file {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|error| {
                    format!("No se pudo abrir el log `{}`: {error}", path.display())
                })?;
            Ok(Stdio::from(file))
        }
        None => Ok(Stdio::inherit()),
    }
}

fn prepend_lib_path(cmd: &mut Command, cef_dir: &std::path::Path) {
    prepend_env(cmd, "LD_LIBRARY_PATH", cef_dir, ":");
}

/// Contrato 4.1: el directorio del slot va *delante* de lo que ya hubiera.
fn compose_search_path(prefix: &str, existing: Option<&str>, sep: &str) -> String {
    match existing {
        Some(existing) if !existing.is_empty() => format!("{prefix}{sep}{existing}"),
        _ => prefix.to_string(),
    }
}

fn prepend_env(cmd: &mut Command, key: &str, cef_dir: &std::path::Path, sep: &str) {
    let prefix = cef_dir.display().to_string();
    let existing = std::env::var(key).ok();
    cmd.env(key, compose_search_path(&prefix, existing.as_deref(), sep));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use tempfile::TempDir;

    #[cfg(unix)]
    fn write_script(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, body).expect("script");
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    #[cfg(unix)]
    fn launch(binary: PathBuf, cef_dir: PathBuf, cache_dir: PathBuf) -> HostLaunch {
        HostLaunch {
            binary,
            cef_dir,
            cache_dir,
            bounds: Some(DEFAULT_OS_BOUNDS),
            scale: 1.0,
            url: "about:blank".into(),
            health_check: false,
            no_sandbox: false,
            log_file: None,
            stderr_file: None,
        }
    }

    #[cfg(unix)]
    const FAKE_HOST: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    *'"cmd":"close"'*) exit 0 ;;
  esac
done
exit 0
"#;

    #[cfg(unix)]
    const EXIT_15: &str = r#"#!/bin/sh
exit 15
"#;

    #[cfg(unix)]
    const EXIT_15_FATAL: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"fatal","message":"sandbox","code":15}'
exit 15
"#;

    #[cfg(unix)]
    const EXIT_16_FATAL: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"fatal","message":"sin compositor Wayland","code":16}'
exit 16
"#;

    /// Sale 15 + fatal salvo que el ADE haya pasado `--idq-no-sandbox` (contrato 5).
    #[cfg(unix)]
    const SANDBOX_FATAL_UNTIL_NO_SANDBOX: &str = r#"#!/bin/sh
case " $* " in
  *" --idq-no-sandbox "*)
    printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
    while IFS= read -r line || [ -n "$line" ]; do
      case "$line" in
        *'"cmd":"close"'*) exit 0 ;;
      esac
    done
    exit 0
    ;;
esac
printf '%s\n' '{"event":"fatal","message":"sandbox","code":15}'
exit 15
"#;

    #[cfg(unix)]
    const READY_THEN_EXIT_15: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
exit 15
"#;

    #[cfg(unix)]
    const IGNORE_CLOSE: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
exec sleep 30
"#;

    #[cfg(unix)]
    const MULTI_EVENT: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
printf '%s\n' '{"event":"title","title":"one"}'
printf '%s\n' '{"event":"title","title":"two"}'
printf '%s\n' '{"event":"title","title":"three"}'
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    *'"cmd":"close"'*) exit 0 ;;
  esac
done
exit 0
"#;

    #[cfg(unix)]
    const GARBAGE_THEN_READY: &str = r#"#!/bin/sh
printf '\n'
printf '   \n'
printf 'not-json\n'
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    *'"cmd":"close"'*) exit 0 ;;
  esac
done
exit 0
"#;

    #[cfg(unix)]
    const DUMP_SPAWN_ENV: &str = r#"#!/bin/sh
cache=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--idq-cache-dir" ]; then
    cache="$arg"
  fi
  prev="$arg"
done
mkdir -p "$cache"
{
  printf '%s' "$LD_LIBRARY_PATH"
} > "$cache/ld_library_path"
: > "$cache/args"
for arg in "$@"; do
  printf '%s\n' "$arg" >> "$cache/args"
done
case " $* " in
  *" --idq-no-sandbox "*) printf '1' > "$cache/no_sandbox" ;;
  *) printf '0' > "$cache/no_sandbox" ;;
esac
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200}'
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    *'"cmd":"close"'*) exit 0 ;;
  esac
done
exit 0
"#;

    #[cfg(unix)]
    fn spawn_ok(binary: PathBuf, root: &std::path::Path, cache_name: &str) -> HostProcess {
        spawn_host(&launch(binary, root.to_path_buf(), root.join(cache_name))).expect("spawn")
    }

    #[cfg(unix)]
    #[test]
    fn spawn_host_ready_send_close_and_exit() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "fake-host", FAKE_HOST);
        let mut host = spawn_host(&launch(
            binary,
            tmp.path().to_path_buf(),
            tmp.path().join("cache"),
        ))
        .expect("spawn");
        let events = host.take_events();

        let first = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        assert_eq!(
            first,
            HostEvent::Ready {
                cef: "x".into(),
                chromium: "y".into(),
                api_version: 15200,
            }
        );
        assert!(host.is_alive());
        host.send(&HostCommand::Navigate {
            url: "https://example.com".into(),
        })
        .expect("send");
        host.send(&HostCommand::Close).expect("close");
        let exit = events.recv_timeout(Duration::from_secs(3)).expect("exit");
        assert_eq!(exit, HostEvent::Exit { code: 0 });
        assert!(!host.is_alive());
    }

    #[cfg(unix)]
    #[test]
    fn spawn_host_exit_15_is_synthesized() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "exit-15", EXIT_15);
        let mut host = spawn_host(&launch(
            binary,
            tmp.path().to_path_buf(),
            tmp.path().join("cache"),
        ))
        .expect("spawn");
        let events = host.take_events();
        let exit = events.recv_timeout(Duration::from_secs(3)).expect("exit");
        assert_eq!(exit, HostEvent::Exit { code: 15 });
        assert!(!host.is_alive());
    }

    #[cfg(unix)]
    #[test]
    fn spawn_host_passes_os_window_flags() {
        let tmp = TempDir::new().unwrap();
        let slot = tmp.path().join("slot-dir");
        fs::create_dir_all(&slot).unwrap();
        let cache = tmp.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        let binary = write_script(tmp.path(), "dump-env", DUMP_SPAWN_ENV);
        let mut host = spawn_host(&launch(binary, slot.clone(), cache.clone())).expect("spawn");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        let args = fs::read_to_string(cache.join("args")).expect("args");
        let lines: Vec<&str> = args.lines().collect();
        assert_eq!(
            lines,
            [
                "--idq-cef-dir",
                slot.to_str().expect("utf8 slot"),
                "--idq-cache-dir",
                cache.to_str().expect("utf8 cache"),
                "--idq-bounds",
                "0,0,1200,800",
                "--idq-scale",
                "1",
                "--idq-url",
                "about:blank",
            ]
        );
        host.send(&HostCommand::Close).expect("close");
        let _ = events.recv_timeout(Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn kill_graceful_kills_a_stuck_host() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "stuck-host", IGNORE_CLOSE);
        let mut host = spawn_host(&launch(
            binary,
            tmp.path().to_path_buf(),
            tmp.path().join("cache"),
        ))
        .expect("spawn");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        assert!(host.is_alive());
        host.kill_graceful(Duration::from_millis(250));
        assert!(!host.is_alive());
        let exit = events.recv_timeout(Duration::from_secs(3)).expect("exit");
        match exit {
            HostEvent::Exit { code } => assert_ne!(code, 0),
            other => panic!("expected Exit, got {other:?}"),
        }
    }

    #[test]
    fn physical_bounds_rounds_css_times_scale() {
        assert_eq!(
            physical_bounds(10.2, 20.6, 100.4, 50.5, 2.0),
            (20, 41, 201, 101)
        );
        assert_eq!(
            physical_bounds(0.0, 36.0, 1200.0, 700.0, 1.0),
            (0, 36, 1200, 700)
        );
    }

    #[test]
    fn resolve_os_bounds_defaults_to_contract_window() {
        assert_eq!(resolve_os_bounds(None, None), (0, 0, 1200, 800));
        assert_eq!(resolve_os_bounds(None, Some(2.0)), (0, 0, 1200, 800));
        assert_eq!(resolve_scale(None), 1.0);
        assert_eq!(resolve_scale(Some(1.5)), 1.5);
        let bounds = Bounds {
            x: 10.2,
            y: 20.6,
            w: 100.4,
            h: 50.5,
        };
        assert_eq!(
            resolve_os_bounds(Some(&bounds), Some(2.0)),
            (20, 41, 201, 101)
        );
        let tiny = Bounds {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        };
        assert_eq!(resolve_os_bounds(Some(&tiny), Some(1.0)), (0, 0, 1, 1));
    }

    #[test]
    fn visible_spawn_requires_wayland_display() {
        assert_eq!(NO_DISPLAY, "sin compositor Wayland");
        assert!(!has_wayland_display(None));
        assert!(!has_wayland_display(Some(OsStr::new(""))));
        assert!(has_wayland_display(Some(OsStr::new("wayland-0"))));
        assert!(has_wayland_display(Some(OsStr::new("wayland-1"))));
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let display = OsString::from_vec(vec![0xff]);
            assert!(has_wayland_display(Some(display.as_os_str())));
        }
        #[cfg(not(unix))]
        {
            let _ = OsString::new();
        }
    }

    #[test]
    fn host_alive_default_is_false() {
        let state = CefState::default();
        assert!(!state.host_alive());
    }

    #[cfg(unix)]
    #[test]
    fn pump_swallows_fatal_and_forwards_ready_after_retry() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(
            tmp.path(),
            "sandbox-then-ready",
            SANDBOX_FATAL_UNTIL_NO_SANDBOX,
        );
        let first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let mut kept: Option<HostProcess> = None;
        let mut ui: Vec<HostEvent> = Vec::new();
        let mut retry_count = 0;

        pump_host_events(
            events,
            &first_launch,
            |retry| {
                retry_count += 1;
                assert!(
                    retry.no_sandbox,
                    "contrato 5: el retry debe llevar --idq-no-sandbox"
                );
                assert_eq!(retry.binary, first_launch.binary);
                let mut process = spawn_host(retry).expect("retry");
                let next = process.take_events();
                kept = Some(process);
                Ok(next)
            },
            |event| {
                let keep_going = !matches!(event, HostEvent::Ready { .. });
                ui.push(event);
                keep_going
            },
        );

        assert_eq!(retry_count, 1, "una sola vez: {ui:?}");
        assert!(
            ui.iter()
                .all(|event| !matches!(event, HostEvent::Fatal { .. })),
            "el fatal del primer intento no debe llegar: {ui:?}"
        );
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Ready { .. })),
            "el ready del retry debe llegar: {ui:?}"
        );
        drop(first);
        drop(kept);
    }

    #[cfg(unix)]
    #[test]
    fn pump_exit_15_without_fatal_still_retries_once() {
        let tmp = TempDir::new().unwrap();
        let dying = write_script(tmp.path(), "exit-15", EXIT_15);
        let ready = write_script(tmp.path(), "ready-host", FAKE_HOST);
        let first_launch = launch(dying, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let retry_launch = launch(
            ready,
            tmp.path().to_path_buf(),
            tmp.path().join("cache-retry"),
        );
        let mut kept: Option<HostProcess> = None;
        let mut ui = Vec::new();
        let mut retry_count = 0;

        pump_host_events(
            events,
            &first_launch,
            |retry| {
                retry_count += 1;
                assert!(retry.no_sandbox);
                let mut process = spawn_host(&retry_launch).expect("retry");
                let next = process.take_events();
                kept = Some(process);
                Ok(next)
            },
            |event| {
                let keep_going = !matches!(event, HostEvent::Ready { .. });
                ui.push(event);
                keep_going
            },
        );

        assert_eq!(retry_count, 1);
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Ready { .. })),
            "{ui:?}"
        );
        assert!(
            ui.iter()
                .all(|event| !matches!(event, HostEvent::Fatal { .. } | HostEvent::Exit { .. })),
            "ready corta el pump de prueba; no debe haberse filtrado fatal/exit: {ui:?}"
        );
        drop(first);
        drop(kept);
    }

    #[cfg(unix)]
    #[test]
    fn pump_does_not_retry_exit_15_after_ready() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "ready-then-15", READY_THEN_EXIT_15);
        let first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let mut ui = Vec::new();
        let mut retry_count = 0;

        pump_host_events(
            events,
            &first_launch,
            |_| {
                retry_count += 1;
                Err("no se debe reintentar después de ready".into())
            },
            |event| {
                ui.push(event);
                true
            },
        );

        assert_eq!(retry_count, 0, "{ui:?}");
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Ready { .. })),
            "{ui:?}"
        );
        assert_eq!(
            ui.iter()
                .filter(|event| matches!(event, HostEvent::Exit { code: 15 }))
                .count(),
            1,
            "{ui:?}"
        );
        drop(first);
    }

    #[cfg(unix)]
    #[test]
    fn pump_does_not_retry_when_first_launch_already_has_no_sandbox() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "exit-15-fatal", EXIT_15_FATAL);
        let mut first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        first_launch.no_sandbox = true;
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let mut ui = Vec::new();
        let mut retry_count = 0;

        pump_host_events(
            events,
            &first_launch,
            |_| {
                retry_count += 1;
                Err("ya iba sin sandbox".into())
            },
            |event| {
                ui.push(event);
                true
            },
        );

        assert_eq!(retry_count, 0, "{ui:?}");
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Fatal { code: 15, .. })),
            "sin retry el fatal sí llega: {ui:?}"
        );
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Exit { code: 15 })),
            "{ui:?}"
        );
        drop(first);
    }

    #[cfg(unix)]
    #[test]
    fn pump_retry_spawn_failure_flushes_held_fatal_and_exit() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "exit-15-fatal", EXIT_15_FATAL);
        let first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let mut ui = Vec::new();

        pump_host_events(
            events,
            &first_launch,
            |_| Err("no queda binario".into()),
            |event| {
                ui.push(event);
                true
            },
        );

        assert!(
            matches!(ui.first(), Some(HostEvent::Fatal { code: 15, .. })),
            "{ui:?}"
        );
        assert!(
            matches!(ui.last(), Some(HostEvent::Exit { code: 15 })),
            "{ui:?}"
        );
        drop(first);
    }

    #[cfg(unix)]
    #[test]
    fn pump_second_exit_15_after_retry_is_not_retried_again() {
        let tmp = TempDir::new().unwrap();
        let dying = write_script(tmp.path(), "exit-15-fatal", EXIT_15_FATAL);
        let first_launch = launch(
            dying.clone(),
            tmp.path().to_path_buf(),
            tmp.path().join("cache"),
        );
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let retry_launch = launch(
            dying,
            tmp.path().to_path_buf(),
            tmp.path().join("cache-retry"),
        );
        let mut kept: Option<HostProcess> = None;
        let mut ui = Vec::new();
        let mut retry_count = 0;

        pump_host_events(
            events,
            &first_launch,
            |retry| {
                retry_count += 1;
                assert!(retry.no_sandbox);
                let mut process = spawn_host(&retry_launch).expect("retry");
                let next = process.take_events();
                kept = Some(process);
                Ok(next)
            },
            |event| {
                ui.push(event);
                true
            },
        );

        assert_eq!(retry_count, 1, "el segundo 15 no relanza: {ui:?}");
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Fatal { code: 15, .. })),
            "el fatal del segundo intento sí llega: {ui:?}"
        );
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Exit { code: 15 })),
            "{ui:?}"
        );
        drop(first);
        drop(kept);
    }

    #[cfg(unix)]
    #[test]
    fn pump_stops_when_channel_drops_mid_stream() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "multi", MULTI_EVENT);
        let first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("spawn");
        let events = first.take_events();
        let mut ui = Vec::new();

        pump_host_events(
            events,
            &first_launch,
            |_| unreachable!("no hay retry"),
            |event| {
                ui.push(event.clone());
                !matches!(event, HostEvent::Title { title } if title == "one")
            },
        );

        assert!(
            matches!(ui.first(), Some(HostEvent::Ready { .. })),
            "{ui:?}"
        );
        assert!(
            ui.iter()
                .any(|event| matches!(event, HostEvent::Title { title } if title == "one")),
            "{ui:?}"
        );
        assert!(
            ui.iter()
                .all(|event| !matches!(event, HostEvent::Title { title } if title == "two" || title == "three")),
            "Channel caído no sigue drenando: {ui:?}"
        );
        first.kill_graceful(Duration::from_millis(250));
    }

    #[cfg(unix)]
    #[test]
    fn pump_stops_when_channel_drops_on_flushed_fatal() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "exit-16-fatal", EXIT_16_FATAL);
        let first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("spawn");
        let events = first.take_events();
        let mut ui = Vec::new();

        pump_host_events(
            events,
            &first_launch,
            |_| unreachable!("16 no se reintenta"),
            |event| {
                ui.push(event.clone());
                !matches!(event, HostEvent::Fatal { .. })
            },
        );

        assert_eq!(ui.len(), 1, "{ui:?}");
        assert!(matches!(ui[0], HostEvent::Fatal { code: 16, .. }), "{ui:?}");
        match &ui[0] {
            HostEvent::Fatal { message, code: 16 } => {
                assert_eq!(message, "sin compositor Wayland");
            }
            other => panic!("{other:?}"),
        }
        drop(first);
    }

    #[cfg(unix)]
    #[test]
    fn pump_stops_when_channel_drops_on_retry_ready() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(
            tmp.path(),
            "sandbox-then-ready",
            SANDBOX_FATAL_UNTIL_NO_SANDBOX,
        );
        let first_launch = launch(binary, tmp.path().to_path_buf(), tmp.path().join("cache"));
        let mut first = spawn_host(&first_launch).expect("first");
        let events = first.take_events();
        let mut kept: Option<HostProcess> = None;
        let mut ui = Vec::new();

        pump_host_events(
            events,
            &first_launch,
            |retry| {
                let mut process = spawn_host(retry).expect("retry");
                let next = process.take_events();
                kept = Some(process);
                Ok(next)
            },
            |event| {
                ui.push(event.clone());
                !matches!(event, HostEvent::Ready { .. })
            },
        );

        assert_eq!(ui.len(), 1, "{ui:?}");
        assert!(matches!(ui[0], HostEvent::Ready { .. }), "{ui:?}");
        drop(first);
        if let Some(mut host) = kept {
            host.kill_graceful(Duration::from_millis(250));
        }
    }

    #[cfg(unix)]
    #[test]
    fn spawn_reader_survives_dropped_event_channel_until_kill() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "stuck", IGNORE_CLOSE);
        let mut host = spawn_ok(binary, tmp.path(), "cache");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        drop(events);
        assert!(host.is_alive(), "tirar el Channel no mata al host");
        host.kill_graceful(Duration::from_millis(250));
        assert!(!host.is_alive());
    }

    #[cfg(unix)]
    #[test]
    fn take_and_kill_is_ok_on_empty_state() {
        let state = CefState::default();
        take_and_kill(&state).expect("vacío");
        assert!(!state.host_alive());
    }

    #[cfg(unix)]
    #[test]
    fn take_and_kill_reaps_a_live_host() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "stuck", IGNORE_CLOSE);
        let mut host = spawn_ok(binary, tmp.path(), "cache");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        assert!(host.is_alive());
        let state = CefState::default();
        state.set_host_for_test(host);
        assert!(state.host_alive());
        take_and_kill(&state).expect("kill");
        assert!(!state.host_alive());
    }

    #[cfg(unix)]
    #[test]
    fn double_spawn_take_and_kill_then_second_host() {
        let tmp = TempDir::new().unwrap();
        let stuck = write_script(tmp.path(), "stuck", IGNORE_CLOSE);
        let ready = write_script(tmp.path(), "ready", FAKE_HOST);
        let mut first = spawn_ok(stuck, tmp.path(), "cache-a");
        let first_events = first.take_events();
        let _ = first_events
            .recv_timeout(Duration::from_secs(3))
            .expect("ready");
        let first_pid = first.pid();
        let state = CefState::default();
        state.set_host_for_test(first);
        assert!(state.host_alive());

        take_and_kill(&state).expect("primer host");
        assert!(!state.host_alive());
        assert!(
            !pid_alive(first_pid),
            "el primer cef-host no puede seguir vivo tras el segundo spawn"
        );

        let mut second = spawn_ok(ready, tmp.path(), "cache-b");
        let second_events = second.take_events();
        let first_evt = second_events
            .recv_timeout(Duration::from_secs(3))
            .expect("ready 2");
        assert!(matches!(first_evt, HostEvent::Ready { .. }));
        assert!(second.is_alive());
        state.set_host_for_test(second);
        assert!(state.host_alive());
        take_and_kill(&state).expect("cleanup");
        assert!(!state.host_alive());
    }

    #[cfg(unix)]
    #[test]
    fn spawn_host_itself_allows_two_live_processes() {
        let tmp = TempDir::new().unwrap();
        let stuck = write_script(tmp.path(), "stuck", IGNORE_CLOSE);
        let mut a = spawn_ok(stuck.clone(), tmp.path(), "cache-a");
        let mut b = spawn_ok(stuck, tmp.path(), "cache-b");
        let ea = a.take_events();
        let eb = b.take_events();
        let _ = ea.recv_timeout(Duration::from_secs(3)).expect("a");
        let _ = eb.recv_timeout(Duration::from_secs(3)).expect("b");
        assert!(a.is_alive() && b.is_alive());
        a.kill_graceful(Duration::from_millis(250));
        b.kill_graceful(Duration::from_millis(250));
        assert!(!a.is_alive() && !b.is_alive());
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_take_and_kill_does_not_poison_state() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "stuck", IGNORE_CLOSE);
        let mut host = spawn_ok(binary, tmp.path(), "cache");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        let state = CefState::default();
        state.set_host_for_test(host);
        std::thread::scope(|scope| {
            scope.spawn(|| take_and_kill(&state).expect("a"));
            scope.spawn(|| take_and_kill(&state).expect("b"));
        });
        assert!(!state.host_alive());
        take_and_kill(&state).expect("idempotente");
    }

    #[cfg(unix)]
    #[test]
    fn kill_on_exit_reaps_installed_host() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "stuck", IGNORE_CLOSE);
        let mut host = spawn_ok(binary, tmp.path(), "cache");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        let state = CefState::default();
        state.set_host_for_test(host);
        kill_on_exit(&state);
        assert!(!state.host_alive());
    }

    #[cfg(unix)]
    #[test]
    #[should_panic(expected = "los eventos de cef-host ya se están leyendo")]
    fn take_events_twice_panics() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "ready", FAKE_HOST);
        let mut host = spawn_ok(binary, tmp.path(), "cache");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        let _ = host.take_events();
    }

    #[cfg(unix)]
    #[test]
    fn spawn_skips_garbage_stdout_and_still_emits_ready() {
        let tmp = TempDir::new().unwrap();
        let binary = write_script(tmp.path(), "garbage", GARBAGE_THEN_READY);
        let mut host = spawn_ok(binary, tmp.path(), "cache");
        let events = host.take_events();
        let first = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        assert!(matches!(first, HostEvent::Ready { .. }), "{first:?}");
        host.send(&HostCommand::Close).expect("close");
        let exit = events.recv_timeout(Duration::from_secs(3)).expect("exit");
        assert_eq!(exit, HostEvent::Exit { code: 0 });
    }

    #[cfg(unix)]
    #[test]
    fn spawn_prepending_ld_library_path_puts_slot_first() {
        let tmp = TempDir::new().unwrap();
        let slot = tmp.path().join("slot-dir");
        fs::create_dir_all(&slot).unwrap();
        let cache = tmp.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        let binary = write_script(tmp.path(), "dump-env", DUMP_SPAWN_ENV);
        let mut host = spawn_host(&launch(binary, slot.clone(), cache.clone())).expect("spawn");
        let events = host.take_events();
        let _ = events.recv_timeout(Duration::from_secs(3)).expect("ready");
        let got = fs::read_to_string(cache.join("ld_library_path")).expect("dump");
        let prefix = slot.display().to_string();
        let expected = compose_search_path(
            &prefix,
            std::env::var("LD_LIBRARY_PATH").ok().as_deref(),
            ":",
        );
        assert_eq!(got, expected, "LD_LIBRARY_PATH del hijo");
        assert!(
            got == prefix || got.starts_with(&format!("{prefix}:")),
            "el slot debe ir primero: {got}"
        );
        let no_sandbox = fs::read_to_string(cache.join("no_sandbox")).expect("flag");
        assert_eq!(no_sandbox, "0");
        host.send(&HostCommand::Close).expect("close");
        let _ = events.recv_timeout(Duration::from_secs(3));
    }

    #[test]
    fn compose_search_path_prepends_and_keeps_existing() {
        assert_eq!(compose_search_path("/slot", None, ":"), "/slot");
        assert_eq!(compose_search_path("/slot", Some(""), ":"), "/slot");
        assert_eq!(
            compose_search_path("/slot", Some("/usr/lib:/opt/lib"), ":"),
            "/slot:/usr/lib:/opt/lib"
        );
        assert_eq!(
            compose_search_path("/slot", Some("/slot:/usr/lib"), ":"),
            "/slot:/slot:/usr/lib"
        );
    }

    #[cfg(unix)]
    fn pid_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}
