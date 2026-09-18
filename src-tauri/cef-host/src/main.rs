//! cef-host: loads libcef from a slot and embeds a browser as an X11 child.
//! Protocol and flags: `docs/cef/CONTRACT.md` §4.

mod app;
mod args;
mod exit;
mod health;
mod platform;
mod protocol;
mod sandbox;
mod shm;
mod slot;

use std::path::{Path, PathBuf};
use std::process;

use cef::{wrap_app, wrap_render_process_handler, *};

use crate::args::HostArgs;
use crate::exit::fatal;
use crate::protocol::HostEvent;
use crate::slot::HOST_API_VERSION;

/// First-instruction gate (contract 4.2). `--type` wins over `--idq-info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EarlyPath {
    CefSubprocess,
    CompiledInfo,
    ContinueInit,
}

fn early_path(is_subprocess: bool, info: bool) -> EarlyPath {
    if is_subprocess {
        EarlyPath::CefSubprocess
    } else if info {
        EarlyPath::CompiledInfo
    } else {
        EarlyPath::ContinueInit
    }
}

/// Chromium re-exec: `--type` or `--type=renderer|gpu-process|…`.
fn argv_is_cef_subprocess<S: AsRef<str>>(argv: &[S]) -> bool {
    argv.iter().any(|a| {
        let a = a.as_ref();
        a == "--type" || a.starts_with("--type=")
    })
}

/// `--idq-info` / `--idq-info=…` as a flag token (not `--idq-information`).
#[cfg(test)]
fn argv_requests_idq_info<S: AsRef<str>>(argv: &[S]) -> bool {
    argv.iter().skip(1).any(|a| {
        let a = a.as_ref();
        a == "--idq-info" || a.starts_with("--idq-info=")
    })
}

fn compiled_info_event(api_version: u32, cef_compiled: impl Into<String>) -> HostEvent {
    HostEvent::Info {
        api_version,
        cef_compiled: cef_compiled.into(),
    }
}

fn compiled_info_event_from_build() -> HostEvent {
    compiled_info_event(HOST_API_VERSION, slot::compiled_cef_version())
}

fn compiled_info_exit_code() -> i32 {
    exit::OK
}

fn install_stdout_hygiene() {
    unsafe {
        let proto_fd = libc::dup(1);
        if proto_fd < 0 {
            eprintln!("cef-host: dup(stdout) failed");
            process::exit(exit::BAD_ARGS);
        }
        if libc::dup2(2, 1) < 0 {
            eprintln!("cef-host: dup2(stderr, stdout) failed");
            process::exit(exit::BAD_ARGS);
        }
        protocol::init_from_raw_fd(proto_fd);
    }
}

fn compiled_info_and_exit() -> ! {
    protocol::emit(&compiled_info_event_from_build());
    process::exit(compiled_info_exit_code());
}

fn compose_ld_library_path(slot: &str, existing: Option<&str>) -> String {
    match existing {
        Some(existing) if !existing.is_empty() => {
            if existing.split(':').any(|p| p == slot) {
                existing.to_string()
            } else {
                format!("{slot}:{existing}")
            }
        }
        _ => slot.to_string(),
    }
}

fn prepend_ld_library_path(slot: &Path) {
    let slot = slot.display().to_string();
    let existing = std::env::var("LD_LIBRARY_PATH").ok();
    std::env::set_var(
        "LD_LIBRARY_PATH",
        compose_ld_library_path(&slot, existing.as_deref()),
    );
}

fn vk_swiftshader_icd(slot: &Path) -> PathBuf {
    slot.join("vk_swiftshader_icd.json")
}

fn needs_xdg_runtime_fallback(value: Option<&str>) -> bool {
    value.map(str::is_empty).unwrap_or(true)
}

fn default_gsettings_backend() -> &'static str {
    "memory"
}

/// Chromium only understands `unix:` / `tcp:` session buses; anything else spins.
fn keep_dbus_session_bus_address(addr: &str) -> bool {
    addr.starts_with("unix:") || addr.starts_with("tcp:")
}

fn prepare_runtime_env(slot: &Path, cache_dir: &Path) {
    prepend_ld_library_path(slot);

    sandbox::apply_devel_sandbox_env(slot);

    let icd = vk_swiftshader_icd(slot);
    if icd.is_file() {
        std::env::set_var("VK_ICD_FILENAMES", &icd);
    }

    if needs_xdg_runtime_fallback(std::env::var("XDG_RUNTIME_DIR").ok().as_deref()) {
        let dir = cache_dir.join("xdg-runtime");
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
    }

    if std::env::var_os("GSETTINGS_BACKEND").is_none() {
        std::env::set_var("GSETTINGS_BACKEND", default_gsettings_backend());
    }

    // A broken session bus address makes Chromium spin on D-Bus during init.
    if let Ok(addr) = std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        if !keep_dbus_session_bus_address(&addr) {
            std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        }
    }
}

fn api_hash_or_die() {
    let hash = api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    if hash.is_null() {
        fatal(
            exit::API_INCOMPAT,
            "cef_api_hash rejected CEF_API_VERSION_LAST",
        );
    }
}

fn is_cef_subprocess() -> bool {
    let argv: Vec<String> = std::env::args().collect();
    argv_is_cef_subprocess(&argv)
}

fn run_subprocess() -> ! {
    api_hash_or_die();
    let cef_args = cef::args::Args::new();
    let mut sub_app = make_subprocess_app();
    let ret = execute_process(
        Some(cef_args.as_main_args()),
        Some(&mut sub_app),
        std::ptr::null_mut(),
    );
    process::exit(if ret >= 0 { ret } else { exit::INIT_FAILED });
}

fn windowless_rendering_enabled(health_check: bool) -> i32 {
    i32::from(health_check)
}

fn wants_health_watchdog(health_check: bool) -> bool {
    health_check
}

fn spawn_stdin_reader_after_init(health_check: bool) -> bool {
    !health_check
}

fn initialize_failure_code(no_sandbox: bool) -> i32 {
    if no_sandbox {
        exit::INIT_FAILED
    } else {
        exit::SANDBOX
    }
}

/// Windowless health skips `CefShutdown` — CHECKs on this CEF/X11 combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterMessageLoop {
    ExitOkSkipShutdown,
    ShutdownThenOk,
}

fn after_message_loop(health_check: bool) -> AfterMessageLoop {
    if health_check {
        AfterMessageLoop::ExitOkSkipShutdown
    } else {
        AfterMessageLoop::ShutdownThenOk
    }
}

fn main() {
    // Subprocesses must reach execute_process before dup2/X11: Chromium may
    // already have wired stdout and fds for GPU/renderer IPC.
    if is_cef_subprocess() {
        run_subprocess();
    }

    install_stdout_hygiene();

    #[cfg(target_os = "linux")]
    platform::init_threads();

    let parsed = HostArgs::parse();
    if matches!(early_path(false, parsed.info), EarlyPath::CompiledInfo) {
        compiled_info_and_exit();
    }

    api_hash_or_die();

    let cef_args = cef::args::Args::new();
    let mut sub_app = make_subprocess_app();
    let ret = execute_process(
        Some(cef_args.as_main_args()),
        Some(&mut sub_app),
        std::ptr::null_mut(),
    );
    if ret >= 0 {
        process::exit(ret);
    }

    let (cef_dir, cache_dir) = parsed.require_slot_and_cache();
    prepare_runtime_env(&cef_dir, &cache_dir);

    // Solo el proceso browser llega aquí; los subprocesos heredan env y switches.
    let shm_policy = shm::decide(&cache_dir);
    if let Err(error) = shm::apply_env(&shm_policy) {
        eprintln!("cef-host: no se pudo preparar TMPDIR para shm: {error}");
    }
    eprintln!("cef-host: shm {shm_policy:?}");

    #[cfg(target_os = "linux")]
    {
        platform::init_threads();
        platform::ensure_display();
    }

    let manifest = slot::validate(&cef_dir);
    let _ = std::fs::create_dir_all(&cache_dir);

    let log_path = parsed.log_path(&cache_dir);
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let exe = std::env::current_exe().unwrap_or_else(|e| {
        fatal(exit::BAD_ARGS, format!("current_exe: {e}"))
    });

    let mut settings = Settings {
        no_sandbox: if parsed.no_sandbox { 1 } else { 0 },
        windowless_rendering_enabled: windowless_rendering_enabled(parsed.health_check),
        log_severity: LogSeverity::WARNING,
        background_color: 0xFF1C1E22,
        remote_debugging_port: 0,
        locale: CefString::from("en-US"),
        ..Default::default()
    };
    settings.browser_subprocess_path = CefString::from(exe.to_string_lossy().as_ref());
    settings.root_cache_path = CefString::from(cache_dir.to_string_lossy().as_ref());
    settings.resources_dir_path = CefString::from(cef_dir.to_string_lossy().as_ref());
    let locales = cef_dir.join("locales");
    settings.locales_dir_path = CefString::from(locales.to_string_lossy().as_ref());
    settings.log_file = CefString::from(log_path.to_string_lossy().as_ref());

    let state = app::AppState::new(parsed.clone(), manifest, shm_policy.disable_dev_shm());
    if wants_health_watchdog(parsed.health_check) {
        let cancel = health::start_watchdog();
        if let Ok(mut slot) = state.health_cancel.lock() {
            *slot = Some(cancel);
        }
    }
    state.install();

    let mut app = app::make_app(state.clone());
    let ok = initialize(
        Some(cef_args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if ok != 1 {
        if parsed.no_sandbox {
            fatal(initialize_failure_code(true), "cef_initialize failed");
        } else {
            fatal(
                initialize_failure_code(false),
                "cef_initialize failed (sandbox may be unavailable)",
            );
        }
    }

    if spawn_stdin_reader_after_init(parsed.health_check) {
        protocol::spawn_stdin_reader();
    }
    run_message_loop();
    match after_message_loop(parsed.health_check) {
        AfterMessageLoop::ExitOkSkipShutdown => {
            // Avoid CefShutdown CHECKs after a windowless health run.
            process::exit(exit::OK);
        }
        AfterMessageLoop::ShutdownThenOk => {
            shutdown();
            process::exit(exit::OK);
        }
    }
}

fn make_subprocess_app() -> App {
    SubprocessApp::new(HostRenderProcess::new())
}

fn should_inject_renderer_trap(is_main_frame: bool) -> bool {
    is_main_frame
}

wrap_render_process_handler! {
    struct HostRenderProcess;

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _context: Option<&mut V8Context>,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if !should_inject_renderer_trap(frame.is_main() != 0) {
                return;
            }
            crate::app::inject_take_focus_trap(frame);
        }
    }
}

wrap_app! {
    struct SubprocessApp {
        render: RenderProcessHandler,
    }

    impl App {
        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(self.render.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn renderer_trap_is_main_frame_only() {
        assert!(should_inject_renderer_trap(true));
        assert!(!should_inject_renderer_trap(false));
    }

    #[test]
    fn subprocess_gate_wins_over_idq_info() {
        assert_eq!(
            early_path(true, true),
            EarlyPath::CefSubprocess,
            "--type must reach execute_process before --idq-info"
        );
        assert_eq!(early_path(true, false), EarlyPath::CefSubprocess);
        assert_eq!(early_path(false, true), EarlyPath::CompiledInfo);
        assert_eq!(early_path(false, false), EarlyPath::ContinueInit);
    }

    #[test]
    fn idq_info_plus_health_check_is_still_compiled_info() {
        let flags = argv(&[
            "cef-host",
            "--idq-info",
            "--idq-health-check",
            "--idq-cef-dir",
            "/missing-slot",
        ]);
        assert!(argv_requests_idq_info(&flags));
        assert!(!argv_is_cef_subprocess(&flags));
        assert_eq!(
            early_path(false, argv_requests_idq_info(&flags)),
            EarlyPath::CompiledInfo
        );
    }

    #[test]
    fn idq_info_token_shapes() {
        assert!(argv_requests_idq_info(&argv(&["cef-host", "--idq-info"])));
        assert!(argv_requests_idq_info(&argv(&["cef-host", "--idq-info="])));
        assert!(argv_requests_idq_info(&argv(&["cef-host", "--idq-info=1"])));
        assert!(argv_requests_idq_info(&argv(&[
            "cef-host",
            "--idq-info=false"
        ])));
        assert!(argv_requests_idq_info(&argv(&[
            "cef-host",
            "--ozone-platform=x11",
            "--idq-info",
            "--idq-no-sandbox",
        ])));
    }

    #[test]
    fn idq_info_is_not_a_prefix_match() {
        assert!(
            !argv_requests_idq_info(&argv(&["cef-host", "--idq-information"])),
            "--idq-information is an unknown --idq-* token"
        );
        assert!(!argv_requests_idq_info(&argv(&["cef-host", "--idq-info-please"])));
        assert!(!argv_requests_idq_info(&argv(&["cef-host", "--info"])));
        assert!(!argv_requests_idq_info(&argv(&["cef-host", "--idq-health-check"])));
        assert!(!argv_requests_idq_info(&argv(&["cef-host"])));
        assert!(
            !argv_requests_idq_info(&argv(&["--idq-info"])),
            "argv[0] is the program name, not a flag"
        );
        assert!(!argv_requests_idq_info(&argv(&["cef-host", "--IDQ-INFO"])));
        assert!(!argv_requests_idq_info(&argv(&["cef-host", "idq-info"])));
    }

    #[test]
    fn chromium_type_switch_is_subprocess() {
        assert!(argv_is_cef_subprocess(&argv(&[
            "cef-host",
            "--type=renderer"
        ])));
        assert!(argv_is_cef_subprocess(&argv(&["cef-host", "--type=gpu-process"])));
        assert!(argv_is_cef_subprocess(&argv(&["cef-host", "--type"])));
        assert!(argv_is_cef_subprocess(&argv(&["cef-host", "--type="])));
        assert!(!argv_is_cef_subprocess(&argv(&["cef-host", "--idq-info"])));
        assert!(!argv_is_cef_subprocess(&argv(&["cef-host", "--idq-type=renderer"])));
        assert!(!argv_is_cef_subprocess(&argv(&["cef-host", "--typename"])));
        assert_eq!(
            early_path(
                argv_is_cef_subprocess(&argv(&["cef-host", "--type=utility", "--idq-info"])),
                argv_requests_idq_info(&argv(&["cef-host", "--type=utility", "--idq-info"])),
            ),
            EarlyPath::CefSubprocess
        );
    }

    #[test]
    fn compiled_info_json_matches_contract() {
        let event = compiled_info_event(15200, "152.0.6+g708dc14+chromium-152.0.7977.83");
        let json = serde_json::to_string(&event).expect("serialize info");
        assert_eq!(
            json,
            r#"{"event":"info","apiVersion":15200,"cefCompiled":"152.0.6+g708dc14+chromium-152.0.7977.83"}"#
        );
        assert!(!json.contains("cefDir"));
        assert!(!json.contains("health"));
    }

    #[test]
    fn compiled_info_uses_host_api_and_compiled_cef() {
        let event = compiled_info_event_from_build();
        match event {
            HostEvent::Info {
                api_version,
                cef_compiled,
            } => {
                assert_eq!(api_version, HOST_API_VERSION);
                assert_eq!(api_version, 15200);
                assert!(
                    cef_compiled.starts_with("152."),
                    "compiled CEF should be 152.x, got {cef_compiled}"
                );
                assert!(cef_compiled.contains('+'));
            }
            other => panic!("expected Info, got {other:?}"),
        }
        assert_eq!(compiled_info_exit_code(), 0);
        assert_eq!(compiled_info_exit_code(), exit::OK);
    }

    #[test]
    fn compiled_info_does_not_boot_cef_or_health() {
        assert!(!wants_health_watchdog(false));
        assert_eq!(windowless_rendering_enabled(false), 0);
        assert!(spawn_stdin_reader_after_init(false));
        assert_eq!(after_message_loop(false), AfterMessageLoop::ShutdownThenOk);
        assert_eq!(
            early_path(false, true),
            EarlyPath::CompiledInfo,
            "--idq-info exits before require_slot / DISPLAY / initialize"
        );
    }

    #[test]
    fn health_check_settings_and_watchdog_gate() {
        assert_eq!(windowless_rendering_enabled(true), 1);
        assert_eq!(windowless_rendering_enabled(false), 0);
        assert!(wants_health_watchdog(true));
        assert!(!wants_health_watchdog(false));
        assert!(!spawn_stdin_reader_after_init(true));
        assert_eq!(
            after_message_loop(true),
            AfterMessageLoop::ExitOkSkipShutdown,
            "keep skip-CefShutdown after windowless health"
        );
        assert_eq!(health::WATCHDOG_TIMEOUT, std::time::Duration::from_secs(30));
        assert_eq!(health::watchdog_exit_code(), 12);
    }

    #[test]
    fn initialize_failure_is_11_without_sandbox_else_15() {
        assert_eq!(initialize_failure_code(true), 11);
        assert_eq!(initialize_failure_code(true), exit::INIT_FAILED);
        assert_eq!(initialize_failure_code(false), 15);
        assert_eq!(initialize_failure_code(false), exit::SANDBOX);
    }

    #[test]
    fn ld_library_path_prepends_slot_once() {
        assert_eq!(compose_ld_library_path("/slot", None), "/slot");
        assert_eq!(compose_ld_library_path("/slot", Some("")), "/slot");
        assert_eq!(
            compose_ld_library_path("/slot", Some("/usr/lib")),
            "/slot:/usr/lib"
        );
        assert_eq!(
            compose_ld_library_path("/slot", Some("/slot:/usr/lib")),
            "/slot:/usr/lib"
        );
        assert_eq!(
            compose_ld_library_path("/slot", Some("/usr/lib:/slot")),
            "/usr/lib:/slot",
            "already present later in the path is left alone"
        );
    }

    #[test]
    fn dbus_session_keeps_only_unix_or_tcp() {
        assert!(keep_dbus_session_bus_address(
            "unix:path=/run/user/1000/bus"
        ));
        assert!(keep_dbus_session_bus_address("unix:abstract=/tmp/dbus"));
        assert!(keep_dbus_session_bus_address("tcp:host=127.0.0.1,port=1234"));
        assert!(!keep_dbus_session_bus_address("autolaunch:"));
        assert!(!keep_dbus_session_bus_address("nonce-tcp:host=1"));
        assert!(!keep_dbus_session_bus_address(""));
        assert!(!keep_dbus_session_bus_address("unix"));
        assert!(!keep_dbus_session_bus_address("UNIX:path=/bus"));
        assert!(!keep_dbus_session_bus_address("disabled:"));
    }

    #[test]
    fn xdg_runtime_fallback_and_gsettings_defaults() {
        assert!(needs_xdg_runtime_fallback(None));
        assert!(needs_xdg_runtime_fallback(Some("")));
        assert!(!needs_xdg_runtime_fallback(Some("/run/user/1000")));
        assert_eq!(default_gsettings_backend(), "memory");
        assert_eq!(
            vk_swiftshader_icd(Path::new("/slot")).as_path(),
            Path::new("/slot/vk_swiftshader_icd.json")
        );
    }
}
