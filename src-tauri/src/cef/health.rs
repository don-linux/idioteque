//! Health check del candidate CEF (contrato 4.5). Solo informa; no denylista.

use std::fs;
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use super::host::{spawn_host, HostLaunch};
use super::ipc::HostEvent;

/// Tiempo máximo que el ADE espera al health check (contrato 4.5).
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(45);

const KILL_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthFailure {
    Exit(i32),
    Timeout,
    /// El proceso terminó por señal / sin código de salida.
    Crashed,
    /// Salió 0 sin un evento `health ok:true`.
    NoHandshake,
    Spawn(String),
}

impl HealthFailure {
    pub fn denylist_reason(&self) -> String {
        match self {
            HealthFailure::Exit(code) => format!("health-exit-{code}"),
            HealthFailure::Timeout => "health-timeout".to_string(),
            HealthFailure::Crashed => "health-crashed".to_string(),
            HealthFailure::NoHandshake => "health-no-handshake".to_string(),
            HealthFailure::Spawn(_) => "health-spawn".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthOutcome {
    Passed {
        cef: String,
        chromium: String,
        api_version: u32,
    },
    Failed(HealthFailure),
}

pub struct HealthRun<'a> {
    pub binary: &'a Path,
    pub slot_dir: &'a Path,
    pub cache_dir: &'a Path,
    pub no_sandbox: bool,
    pub log_file: Option<&'a Path>,
    pub timeout: Duration,
}

/// Arranca `cef-host --idq-health-check` y clasifica el resultado.
///
/// Pasa solo si hubo `health ok:true` **y** el proceso salió con código 0.
/// Siempre intenta borrar `cache_dir` al terminar. Nunca denylista.
pub fn run_health_check(run: &HealthRun) -> HealthOutcome {
    let _cleanup = RemoveDir(run.cache_dir);

    let launch = HostLaunch {
        binary: run.binary.to_path_buf(),
        cef_dir: run.slot_dir.to_path_buf(),
        cache_dir: run.cache_dir.to_path_buf(),
        parent_xid: None,
        bounds: None,
        scale: 1.0,
        url: "about:blank".into(),
        health_check: true,
        no_sandbox: run.no_sandbox || super::sandbox::wants_no_sandbox(run.slot_dir),
        // Chromium trunca `--idq-log`; el fatal del host va a stderr.
        log_file: None,
        stderr_file: run.log_file.map(Path::to_path_buf),
    };

    let mut host = match spawn_host(&launch) {
        Ok(host) => host,
        Err(error) => return HealthOutcome::Failed(HealthFailure::Spawn(error)),
    };
    let events = host.take_events();

    let deadline = Instant::now() + run.timeout;
    let mut handshake: Option<(String, String, u32)> = None;
    let mut saw_fatal = false;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            host.kill_graceful(KILL_GRACE);
            return HealthOutcome::Failed(HealthFailure::Timeout);
        }

        match events.recv_timeout(remaining) {
            Ok(HostEvent::Health {
                ok: true,
                cef,
                chromium,
                api_version,
            }) => {
                handshake = Some((cef, chromium, api_version));
            }
            Ok(HostEvent::Fatal { .. }) => {
                saw_fatal = true;
            }
            Ok(HostEvent::Exit { code: 0 }) => {
                return match handshake {
                    Some((cef, chromium, api_version)) => HealthOutcome::Passed {
                        cef,
                        chromium,
                        api_version,
                    },
                    None => HealthOutcome::Failed(HealthFailure::NoHandshake),
                };
            }
            Ok(HostEvent::Exit { code }) => {
                return HealthOutcome::Failed(failure_from_exit(code, saw_fatal));
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {
                host.kill_graceful(KILL_GRACE);
                return HealthOutcome::Failed(HealthFailure::Timeout);
            }
            Err(RecvTimeoutError::Disconnected) => {
                return HealthOutcome::Failed(HealthFailure::Crashed);
            }
        }
    }
}

/// `spawn_host` traduce una muerte por señal (`ExitStatus::code() == None`) a `Exit { code: 1 }`.
/// Los códigos de contrato (4.7) nunca usan `1` sin un `fatal` previo.
fn failure_from_exit(code: i32, saw_fatal: bool) -> HealthFailure {
    if code == 1 && !saw_fatal {
        HealthFailure::Crashed
    } else {
        HealthFailure::Exit(code)
    }
}

struct RemoveDir<'a>(&'a Path);

impl Drop for RemoveDir<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[cfg(unix)]
    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, body).expect("script");
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    fn setup_dirs(tmp: &TempDir) -> (PathBuf, PathBuf) {
        let slot = tmp.path().join("slot");
        let cache = tmp.path().join("health-cache-1");
        fs::create_dir_all(&slot).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("junk"), b"x").unwrap();
        (slot, cache)
    }

    fn run_with(binary: &Path, slot: &Path, cache: &Path, timeout: Duration) -> HealthOutcome {
        run_health_check(&HealthRun {
            binary,
            slot_dir: slot,
            cache_dir: cache,
            no_sandbox: false,
            log_file: None,
            timeout,
        })
    }

    fn assert_cache_gone(cache: &Path) {
        assert!(
            !cache.exists(),
            "el directorio de caché del health check debía borrarse"
        );
    }

    #[test]
    fn denylist_reasons_match_contract() {
        assert_eq!(HealthFailure::Exit(10).denylist_reason(), "health-exit-10");
        assert_eq!(HealthFailure::Timeout.denylist_reason(), "health-timeout");
        assert_eq!(HealthFailure::Crashed.denylist_reason(), "health-crashed");
        assert_eq!(
            HealthFailure::NoHandshake.denylist_reason(),
            "health-no-handshake"
        );
        assert_eq!(
            HealthFailure::Spawn("x".into()).denylist_reason(),
            "health-spawn"
        );
    }

    #[test]
    fn health_timeout_constant_is_45s() {
        assert_eq!(HEALTH_TIMEOUT, Duration::from_secs(45));
    }

    #[test]
    fn spawn_failure_is_reported_and_cache_removed() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let missing = tmp.path().join("no-such-host");
        let outcome = run_with(&missing, &slot, &cache, Duration::from_secs(1));
        match outcome {
            HealthOutcome::Failed(HealthFailure::Spawn(message)) => {
                assert!(message.contains("No se pudo lanzar cef-host"));
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
        assert_cache_gone(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn health_ok_and_exit_zero_is_passed() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let binary = write_script(
            tmp.path(),
            "ok-host",
            r#"#!/bin/sh
printf '%s\n' '{"event":"health","ok":true,"cef":"152.0.6+g708dc14+chromium-152.0.7977.83","chromium":"152.0.7977.83","apiVersion":15200}'
exit 0
"#,
        );
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(3));
        assert_eq!(
            outcome,
            HealthOutcome::Passed {
                cef: "152.0.6+g708dc14+chromium-152.0.7977.83".into(),
                chromium: "152.0.7977.83".into(),
                api_version: 15200,
            }
        );
        assert_cache_gone(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn health_ok_then_nonzero_exit_is_failure() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let binary = write_script(
            tmp.path(),
            "ok-then-10",
            r#"#!/bin/sh
printf '%s\n' '{"event":"health","ok":true,"cef":"152.0.6+g708dc14+chromium-152.0.7977.83","chromium":"152.0.7977.83","apiVersion":15200}'
exit 10
"#,
        );
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(3));
        match &outcome {
            HealthOutcome::Failed(failure @ HealthFailure::Exit(10)) => {
                assert_eq!(failure.denylist_reason(), "health-exit-10");
            }
            other => panic!("expected Exit(10), got {other:?}"),
        }
        assert_cache_gone(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn health_ok_false_and_exit_zero_is_no_handshake() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let binary = write_script(
            tmp.path(),
            "ok-false-host",
            r#"#!/bin/sh
printf '%s\n' '{"event":"health","ok":false,"cef":"x","chromium":"y","apiVersion":15200}'
exit 0
"#,
        );
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(3));
        match &outcome {
            HealthOutcome::Failed(failure @ HealthFailure::NoHandshake) => {
                assert_eq!(failure.denylist_reason(), "health-no-handshake");
            }
            other => panic!("expected NoHandshake, got {other:?}"),
        }
        assert_cache_gone(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn fatal_then_exit_10_is_exit_failure() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let binary = write_script(
            tmp.path(),
            "fatal-host",
            r#"#!/bin/sh
printf '%s\n' '{"event":"fatal","message":"api","code":10}'
exit 10
"#,
        );
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(3));
        match &outcome {
            HealthOutcome::Failed(failure @ HealthFailure::Exit(10)) => {
                assert_eq!(failure.denylist_reason(), "health-exit-10");
            }
            other => panic!("expected Exit(10), got {other:?}"),
        }
        assert_cache_gone(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn sleep_past_timeout_is_timeout_and_process_is_gone() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let pidfile = tmp.path().join("host.pid");
        let binary = write_script(
            tmp.path(),
            "slow-host",
            &format!(
                "#!/bin/sh\necho $$ > '{}'\nexec sleep 30\n",
                pidfile.display()
            ),
        );
        let started = Instant::now();
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(1));
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "el timeout no debía esperar al sleep de 30 s"
        );
        match &outcome {
            HealthOutcome::Failed(failure @ HealthFailure::Timeout) => {
                assert_eq!(failure.denylist_reason(), "health-timeout");
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
        assert_cache_gone(&cache);

        let pid: u32 = fs::read_to_string(&pidfile)
            .expect("pidfile")
            .trim()
            .parse()
            .expect("pid");
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "el proceso del health check debía terminar tras el timeout"
        );
    }

    #[cfg(unix)]
    #[test]
    fn exit_zero_without_health_is_no_handshake() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let binary = write_script(tmp.path(), "silent-host", "#!/bin/sh\nexit 0\n");
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(3));
        match &outcome {
            HealthOutcome::Failed(failure @ HealthFailure::NoHandshake) => {
                assert_eq!(failure.denylist_reason(), "health-no-handshake");
            }
            other => panic!("expected NoHandshake, got {other:?}"),
        }
        assert_cache_gone(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn kill_dash_nine_is_crashed() {
        let tmp = TempDir::new().unwrap();
        let (slot, cache) = setup_dirs(&tmp);
        let binary = write_script(tmp.path(), "crash-host", "#!/bin/sh\nkill -9 $$\n");
        let outcome = run_with(&binary, &slot, &cache, Duration::from_secs(3));
        match &outcome {
            HealthOutcome::Failed(failure @ HealthFailure::Crashed) => {
                assert_eq!(failure.denylist_reason(), "health-crashed");
            }
            other => panic!("expected Crashed, got {other:?}"),
        }
        assert_cache_gone(&cache);
    }
}
