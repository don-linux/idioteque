//! cef-host: loads libcef from a slot and embeds a browser as an X11 child.
//! Protocol and flags: `docs/cef/CONTRACT.md` §4.

mod app;
mod args;
mod exit;
mod health;
mod platform;
mod protocol;
mod slot;

use std::path::Path;
use std::process;

use cef::wrap_app;
use cef::*;

use crate::args::HostArgs;
use crate::exit::fatal;
use crate::protocol::HostEvent;
use crate::slot::HOST_API_VERSION;

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
    protocol::emit(&HostEvent::Info {
        api_version: HOST_API_VERSION,
        cef_compiled: slot::compiled_cef_version(),
    });
    process::exit(exit::OK);
}

fn prepend_ld_library_path(slot: &Path) {
    let slot = slot.display().to_string();
    match std::env::var("LD_LIBRARY_PATH") {
        Ok(existing) if !existing.is_empty() => {
            if !existing.split(':').any(|p| p == slot) {
                std::env::set_var("LD_LIBRARY_PATH", format!("{slot}:{existing}"));
            }
        }
        _ => std::env::set_var("LD_LIBRARY_PATH", &slot),
    }
}

fn prepare_runtime_env(slot: &Path, cache_dir: &Path) {
    prepend_ld_library_path(slot);

    let sandbox = slot.join("chrome-sandbox");
    if sandbox.is_file() && std::env::var_os("CHROME_DEVEL_SANDBOX").is_none() {
        std::env::set_var("CHROME_DEVEL_SANDBOX", &sandbox);
    }

    let icd = slot.join("vk_swiftshader_icd.json");
    if icd.is_file() {
        std::env::set_var("VK_ICD_FILENAMES", &icd);
    }

    if std::env::var("XDG_RUNTIME_DIR")
        .map(|s| s.is_empty())
        .unwrap_or(true)
    {
        let dir = cache_dir.join("xdg-runtime");
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
    }

    if std::env::var_os("GSETTINGS_BACKEND").is_none() {
        std::env::set_var("GSETTINGS_BACKEND", "memory");
    }

    // A broken session bus address makes Chromium spin on D-Bus during init.
    if let Ok(addr) = std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        if !addr.starts_with("unix:") && !addr.starts_with("tcp:") {
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
    std::env::args().any(|a| a == "--type" || a.starts_with("--type="))
}

fn run_subprocess() -> ! {
    api_hash_or_die();
    let cef_args = cef::args::Args::new();
    let mut sub_app = SubprocessApp::new();
    let ret = execute_process(
        Some(cef_args.as_main_args()),
        Some(&mut sub_app),
        std::ptr::null_mut(),
    );
    process::exit(if ret >= 0 { ret } else { exit::INIT_FAILED });
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
    if parsed.info {
        compiled_info_and_exit();
    }

    api_hash_or_die();

    let cef_args = cef::args::Args::new();
    let mut sub_app = SubprocessApp::new();
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
        windowless_rendering_enabled: if parsed.health_check { 1 } else { 0 },
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

    let state = app::AppState::new(parsed.clone(), manifest);
    if parsed.health_check {
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
            fatal(exit::INIT_FAILED, "cef_initialize failed");
        } else {
            fatal(
                exit::SANDBOX,
                "cef_initialize failed (sandbox may be unavailable)",
            );
        }
    }

    if !parsed.health_check {
        protocol::spawn_stdin_reader();
    }
    run_message_loop();
    if parsed.health_check {
        // Avoid CefShutdown CHECKs after a windowless health run.
        process::exit(exit::OK);
    }
    shutdown();
    process::exit(exit::OK);
}

wrap_app! {
    struct SubprocessApp;

    impl App {}
}
