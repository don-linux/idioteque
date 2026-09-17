//! Spawn y protocolo de `cef-host`. Sin tipos Tauri en `spawn_host` / `HostProcess`.

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

const X11_REQUIRED: &str = "El navegador necesita X11 (arranca idioteque con GDK_BACKEND=x11)";
const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(2);

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
    pub parent_xid: Option<u64>,
    pub bounds: Option<(i32, i32, i32, i32)>,
    pub scale: f64,
    pub url: String,
    pub health_check: bool,
    pub no_sandbox: bool,
    pub log_file: Option<PathBuf>,
}

pub struct HostProcess {
    child: Arc<Mutex<Child>>,
    stdin: Mutex<ChildStdin>,
    events: Option<Receiver<HostEvent>>,
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
}

pub fn spawn_host(launch: &HostLaunch) -> Result<HostProcess, String> {
    let mut cmd = Command::new(&launch.binary);
    cmd.arg("--idq-cef-dir").arg(&launch.cef_dir);
    cmd.arg("--idq-cache-dir").arg(&launch.cache_dir);

    if let Some(xid) = launch.parent_xid {
        cmd.arg("--idq-parent").arg(xid.to_string());
    }
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
    cmd.env(
        "CHROME_DEVEL_SANDBOX",
        launch.cef_dir.join("chrome-sandbox"),
    );

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(stderr_stdio(launch.log_file.as_ref())?);

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
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|error| error.to_string())?;
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
    window: tauri::Window,
    state: State<CefState>,
    url: String,
    bounds: Bounds,
    scale: f64,
    on_event: Channel<HostEvent>,
) -> Result<BrowserBoot, String> {
    take_and_kill(&state)?;

    let paths = CefPaths::from_app(&app)?;
    paths.ensure_dirs()?;
    let slot = manifest::resolve_effective(&paths)?;
    let binary = paths::host_binary_path(&app)?;
    let physical = physical_bounds(bounds.x, bounds.y, bounds.w, bounds.h, scale);
    let parent_xid = parent_xid(&window)?;

    let no_sandbox = env_no_sandbox() || state.no_sandbox.load(Ordering::SeqCst);
    if no_sandbox {
        state.no_sandbox.store(true, Ordering::SeqCst);
    }

    let launch = HostLaunch {
        binary,
        cef_dir: slot.dir.clone(),
        cache_dir: paths.profile(),
        parent_xid,
        bounds: Some(physical),
        scale,
        url,
        health_check: false,
        no_sandbox,
        log_file: Some(paths.logs_dir().join("cef-host.log")),
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
    let (x, y, w, h) = physical_bounds(x, y, w, h, scale);
    with_host(&state, |host| {
        host.send(&HostCommand::SetBounds { x, y, w, h })
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
pub fn browser_kill(state: State<CefState>) -> Result<(), String> {
    take_and_kill(&state)
}

fn start_forward_thread(
    app: AppHandle,
    on_event: Channel<HostEvent>,
    events: Receiver<HostEvent>,
    launch: HostLaunch,
) {
    std::thread::spawn(move || {
        let mut events = events;
        let mut saw_ready = false;
        let mut retried_sandbox = false;

        loop {
            let event = match events.recv() {
                Ok(event) => event,
                Err(_) => break,
            };

            match &event {
                HostEvent::Ready { .. } => {
                    saw_ready = true;
                    if on_event.send(event).is_err() {
                        break;
                    }
                }
                HostEvent::Exit { code: 15 } if !saw_ready && !retried_sandbox && !launch.no_sandbox =>
                {
                    retried_sandbox = true;
                    eprintln!("[cef] sandbox no disponible, reintentando con --idq-no-sandbox");
                    let state = app.state::<CefState>();
                    state.no_sandbox.store(true, Ordering::SeqCst);
                    let mut retry = launch.clone();
                    retry.no_sandbox = true;
                    match spawn_host(&retry) {
                        Ok(mut process) => {
                            let next = process.take_events();
                            if let Ok(mut guard) = state.host.lock() {
                                *guard = Some(process);
                            }
                            events = next;
                        }
                        Err(error) => {
                            eprintln!("[cef] reintento sin sandbox falló: {error}");
                            let _ = on_event.send(event);
                            break;
                        }
                    }
                }
                _ => {
                    if on_event.send(event).is_err() {
                        break;
                    }
                }
            }
        }
    });
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

fn with_host<T>(state: &CefState, f: impl FnOnce(&HostProcess) -> Result<T, String>) -> Result<T, String> {
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

fn env_no_sandbox() -> bool {
    matches!(std::env::var("IDIOTEQUE_CEF_NO_SANDBOX"), Ok(value) if value == "1")
}

fn stderr_stdio(log_file: Option<&PathBuf>) -> Result<Stdio, String> {
    match log_file {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!("No se pudo crear `{}`: {error}", parent.display())
                })?;
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
    #[cfg(target_os = "linux")]
    {
        prepend_env(cmd, "LD_LIBRARY_PATH", cef_dir, ":");
    }
    #[cfg(target_os = "macos")]
    {
        prepend_env(cmd, "DYLD_FALLBACK_LIBRARY_PATH", cef_dir, ":");
    }
    #[cfg(target_os = "windows")]
    {
        prepend_env(cmd, "PATH", cef_dir, ";");
    }
}

fn prepend_env(cmd: &mut Command, key: &str, cef_dir: &std::path::Path, sep: &str) {
    let prefix = cef_dir.display().to_string();
    let value = match std::env::var(key) {
        Ok(existing) if !existing.is_empty() => format!("{prefix}{sep}{existing}"),
        _ => prefix,
    };
    cmd.env(key, value);
}

#[cfg(target_os = "linux")]
fn parent_xid(window: &tauri::Window) -> Result<Option<u64>, String> {
    use gtk::prelude::*;

    let gtk_window = window
        .gtk_window()
        .map_err(|_| X11_REQUIRED.to_string())?;
    let gdk_window = gtk_window.window().ok_or_else(|| X11_REQUIRED.to_string())?;
    let x11_window = gdk_window
        .downcast::<gdkx11::X11Window>()
        .map_err(|_| X11_REQUIRED.to_string())?;
    Ok(Some(x11_window.xid() as u64))
}

#[cfg(not(target_os = "linux"))]
fn parent_xid(_window: &tauri::Window) -> Result<Option<u64>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            parent_xid: None,
            bounds: Some((0, 36, 1200, 700)),
            scale: 1.0,
            url: "about:blank".into(),
            health_check: false,
            no_sandbox: false,
            log_file: None,
        }
    }

    #[cfg(unix)]
    const FAKE_HOST: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200,"xid":1}'
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
    const IGNORE_CLOSE: &str = r#"#!/bin/sh
printf '%s\n' '{"event":"ready","cef":"x","chromium":"y","apiVersion":15200,"xid":1}'
exec sleep 30
"#;

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
                xid: 1,
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
        assert_eq!(physical_bounds(10.2, 20.6, 100.4, 50.5, 2.0), (20, 41, 201, 101));
        assert_eq!(physical_bounds(0.0, 36.0, 1200.0, 700.0, 1.0), (0, 36, 1200, 700));
    }

    #[test]
    fn host_alive_default_is_false() {
        let state = CefState::default();
        assert!(!state.host_alive());
    }
}
