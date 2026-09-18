//! CEF App / Client / handlers and UI-thread command dispatch.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use cef::sys::cef_event_flags_t;
use cef::{
    wrap_app, wrap_browser_process_handler, wrap_client, wrap_context_menu_handler,
    wrap_display_handler, wrap_keyboard_handler, wrap_life_span_handler, wrap_load_handler,
    wrap_request_handler, wrap_task, *,
};

use crate::args::HostArgs;
use crate::exit::{self, fatal};
use crate::platform;
use crate::protocol::{self, HostCommand, HostEvent};
use crate::slot::{self, Manifest};

const MENU_INSPECT: i32 = 26500;
const MENU_RELOAD: i32 = 26501;

const VK_ESCAPE: i32 = 0x1B;
const VK_LEFT: i32 = 0x25;
const VK_RIGHT: i32 = 0x27;
const VK_B: i32 = 0x42;
const VK_L_LOWER: i32 = 0x6C;
const VK_I: i32 = 0x49;
const VK_L: i32 = 0x4C;
const VK_R: i32 = 0x52;
const VK_F5: i32 = 0x74;
const VK_F12: i32 = 0x7B;

const FLAG_SHIFT: u32 = cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0;
const FLAG_CTRL: u32 = cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
const FLAG_ALT: u32 = cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;

/// Chromium `net::ERR_ABORTED`. Contract §4.3: do not emit `load-error` for it.
const ERR_ABORTED: i32 = -3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OzoneMode {
    platform: &'static str,
    disable_gpu: bool,
    use_gl: Option<&'static str>,
    use_angle: Option<&'static str>,
}

fn ozone_mode(health_check: bool) -> OzoneMode {
    if health_check {
        OzoneMode {
            platform: "headless",
            disable_gpu: true,
            use_gl: Some("angle"),
            use_angle: Some("swiftshader"),
        }
    } else {
        OzoneMode {
            platform: "wayland",
            disable_gpu: false,
            use_gl: None,
            use_angle: None,
        }
    }
}

fn ozone_platform(health_check: bool) -> &'static str {
    ozone_mode(health_check).platform
}

/// `IDIOTEQUE_CEF_ARGS` cannot flip visible off wayland or health off headless.
fn effective_ozone_platform(health_check: bool, extra_switches: &[String]) -> &'static str {
    let _ = extra_switches;
    ozone_platform(health_check)
}

fn required_alloy_native_switches() -> &'static [&'static str] {
    &["use-alloy-style", "use-native"]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PopupDisposition {
    cancel: bool,
    load_in_main: Option<String>,
}

fn popup_disposition(target_url: Option<&str>) -> PopupDisposition {
    PopupDisposition {
        cancel: true,
        load_in_main: target_url.map(str::to_string),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClosePlan {
    close_browser: bool,
    quit_loop: bool,
}

fn close_plan(already_closing: bool, has_browser: bool) -> ClosePlan {
    ClosePlan {
        close_browser: !already_closing && has_browser,
        quit_loop: true,
    }
}

fn should_quit_on_before_close(browser_id: i32, main_id: i32) -> bool {
    main_id == 0 || browser_id == main_id
}

fn health_success_teardown() -> ClosePlan {
    ClosePlan {
        close_browser: false,
        quit_loop: true,
    }
}

fn take_main_browser(is_popup: bool, ready_already_sent: bool) -> bool {
    !is_popup && !ready_already_sent
}

fn emit_ready_event(health_check: bool) -> bool {
    !health_check
}

fn is_main_browser(main_id: i32, browser_id: i32) -> bool {
    main_id == 0 || browser_id == main_id
}

fn emit_chrome_ui_event(health_check: bool, is_main: bool) -> bool {
    !health_check && is_main
}

fn is_host_keydown(kind: KeyEventType) -> bool {
    kind == KeyEventType::RAWKEYDOWN || kind == KeyEventType::KEYDOWN
}

fn host_key_code(windows: i32) -> i32 {
    if windows == VK_L_LOWER {
        VK_L
    } else {
        windows
    }
}

fn shortcut_event(health_check: bool, is_main: bool, chord: &str) -> Option<HostEvent> {
    if !emit_chrome_ui_event(health_check, is_main) {
        return None;
    }
    Some(HostEvent::Shortcut {
        chord: chord.to_string(),
    })
}

fn emit_load_error(
    health_check: bool,
    is_main: bool,
    is_main_frame: bool,
    error_code: i32,
) -> bool {
    !health_check && is_main && is_main_frame && error_code != ERR_ABORTED
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreKeyAction {
    Ignore,
    Shortcut(&'static str),
    ToggleDevtools,
    Reload { ignore_cache: bool },
    Back,
    Forward,
    Stop,
}

fn pre_key_action(keydown: bool, key: i32, ctrl: bool, shift: bool, alt: bool) -> PreKeyAction {
    if !keydown {
        return PreKeyAction::Ignore;
    }
    let key = host_key_code(key);
    if ctrl && !alt && key == VK_B {
        return PreKeyAction::Shortcut(if shift { "ctrl+shift+b" } else { "ctrl+b" });
    }
    if key == VK_F12 || (ctrl && shift && !alt && key == VK_I) {
        return PreKeyAction::ToggleDevtools;
    }
    if key == VK_F5 || (ctrl && !alt && key == VK_R) {
        return PreKeyAction::Reload {
            ignore_cache: ctrl && shift && key == VK_R,
        };
    }
    if alt && !ctrl && key == VK_LEFT {
        return PreKeyAction::Back;
    }
    if alt && !ctrl && key == VK_RIGHT {
        return PreKeyAction::Forward;
    }
    if key == VK_ESCAPE {
        return PreKeyAction::Stop;
    }
    PreKeyAction::Ignore
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderTerm {
    Crashed,
    Killed,
    Abnormal,
    Oom,
    LaunchFailed,
    Other(u32),
}

fn render_term_from_status(status: TerminationStatus) -> RenderTerm {
    if status == TerminationStatus::PROCESS_CRASHED {
        RenderTerm::Crashed
    } else if status == TerminationStatus::PROCESS_WAS_KILLED {
        RenderTerm::Killed
    } else if status == TerminationStatus::ABNORMAL_TERMINATION {
        RenderTerm::Abnormal
    } else if status == TerminationStatus::PROCESS_OOM {
        RenderTerm::Oom
    } else if status == TerminationStatus::LAUNCH_FAILED {
        RenderTerm::LaunchFailed
    } else {
        RenderTerm::Other(status.get_raw())
    }
}

fn render_crash_status(kind: RenderTerm, error_string: Option<&str>) -> String {
    match kind {
        RenderTerm::Crashed => "crashed".to_string(),
        RenderTerm::Killed => "killed".to_string(),
        RenderTerm::Abnormal => "abnormal".to_string(),
        RenderTerm::Oom => "oom".to_string(),
        RenderTerm::LaunchFailed => "launch-failed".to_string(),
        RenderTerm::Other(raw) => error_string
            .map(str::to_string)
            .unwrap_or_else(|| format!("{raw}")),
    }
}

static STATE: OnceLock<Arc<AppState>> = OnceLock::new();

pub struct AppState {
    pub args: HostArgs,
    pub manifest: Manifest,
    pub browser: Mutex<Option<Browser>>,
    pub main_id: AtomicI32,
    pub ready_sent: AtomicBool,
    pub health_emitted: AtomicBool,
    pub closing: AtomicBool,
    pub health_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// `--disable-dev-shm-usage`: only when `/dev/shm` is unusable (`shm::decide`).
    pub disable_dev_shm: bool,
}

impl AppState {
    pub fn new(args: HostArgs, manifest: Manifest, disable_dev_shm: bool) -> Arc<Self> {
        Arc::new(Self {
            args,
            manifest,
            disable_dev_shm,
            browser: Mutex::new(None),
            main_id: AtomicI32::new(0),
            ready_sent: AtomicBool::new(false),
            health_emitted: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            health_cancel: Mutex::new(None),
        })
    }

    pub fn install(self: &Arc<Self>) {
        let _ = STATE.set(Arc::clone(self));
    }

    fn lock_browser(&self) -> Option<Browser> {
        self.browser.lock().ok().and_then(|g| g.clone())
    }

    fn versions(&self) -> (String, String, u32) {
        (
            self.manifest.cef_version.clone(),
            self.manifest.chromium_version.clone(),
            slot::HOST_API_VERSION,
        )
    }

    fn is_main(&self, browser: &Browser) -> bool {
        is_main_browser(self.main_id.load(Ordering::SeqCst), browser.identifier())
    }

    fn emit_nav(&self, browser: &Browser, url: Option<String>) {
        if !emit_chrome_ui_event(self.args.health_check, self.is_main(browser)) {
            return;
        }
        let url = url.unwrap_or_else(|| frame_url(browser));
        if url.is_empty() {
            return;
        }
        protocol::emit(&HostEvent::Nav {
            url,
            can_go_back: browser.can_go_back() != 0,
            can_go_forward: browser.can_go_forward() != 0,
            loading: browser.is_loading() != 0,
        });
    }

    fn toggle_devtools(&self, inspect_at: Option<Point>) {
        let Some(browser) = self.lock_browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        if inspect_at.is_none() && host.has_dev_tools() != 0 {
            host.close_dev_tools();
            return;
        }
        let window_info = WindowInfo::default();
        let settings = BrowserSettings::default();
        host.show_dev_tools(
            Some(&window_info),
            None,
            Some(&settings),
            inspect_at.as_ref(),
        );
    }

    fn request_close(&self) {
        let already_closing = self
            .closing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err();
        let plan = close_plan(already_closing, self.lock_browser().is_some());
        if plan.close_browser {
            if let Some(browser) = self.lock_browser() {
                if let Some(host) = browser.host() {
                    host.close_browser(1);
                }
            }
        }
        if plan.quit_loop {
            quit_message_loop();
        }
    }

    fn apply_visibility(&self, visible: bool) {
        if let Some(browser) = self.lock_browser() {
            if let Some(host) = browser.host() {
                host.was_hidden(platform::hidden_flag(visible));
            }
        }
    }

    fn apply_bounds(&self, x: i32, y: i32, w: i32, h: i32) {
        let _ = (x, y);
        let w = platform::clamp_extent(w);
        let h = platform::clamp_extent(h);
        if let Some(browser) = self.lock_browser() {
            if let Some(host) = browser.host() {
                host.notify_move_or_resize_started();
                let _ = (w, h);
                host.was_resized();
            }
        }
    }
}

fn frame_url(browser: &Browser) -> String {
    browser
        .main_frame()
        .map(|f| CefString::from(&f.url()).to_string())
        .unwrap_or_default()
}

fn cef_str(s: &str) -> CefString {
    CefString::from(s)
}

pub fn make_app(state: Arc<AppState>) -> App {
    let handler = HostBrowserProcessHandler::new(state.clone(), Arc::new(Mutex::new(None)));
    HostApp::new(state, handler)
}

pub fn post_cmd(cmd: HostCommand) {
    if currently_on(ThreadId::UI) != 0 {
        dispatch(&cmd);
        return;
    }
    let mut task = HostTask::new(cmd);
    let _ = post_task(ThreadId::UI, Some(&mut task));
}

pub fn dispatch(cmd: &HostCommand) {
    let Some(state) = STATE.get() else {
        return;
    };
    match cmd {
        HostCommand::Navigate { url } => {
            if let Some(browser) = state.lock_browser() {
                if let Some(frame) = browser.main_frame() {
                    frame.load_url(Some(&cef_str(url)));
                }
            }
        }
        HostCommand::Back => {
            if let Some(b) = state.lock_browser() {
                b.go_back();
            }
        }
        HostCommand::Forward => {
            if let Some(b) = state.lock_browser() {
                b.go_forward();
            }
        }
        HostCommand::Stop => {
            if let Some(b) = state.lock_browser() {
                b.stop_load();
            }
        }
        HostCommand::Reload { ignore_cache } => {
            if let Some(b) = state.lock_browser() {
                if *ignore_cache {
                    b.reload_ignore_cache();
                } else {
                    b.reload();
                }
            }
        }
        HostCommand::SetBounds { x, y, w, h } => state.apply_bounds(*x, *y, *w, *h),
        HostCommand::Show => state.apply_visibility(true),
        HostCommand::Hide => state.apply_visibility(false),
        HostCommand::Devtools => state.toggle_devtools(None),
        HostCommand::Close => state.request_close(),
    }
}

fn has_switch(command_line: &CommandLine, name: &str) -> bool {
    command_line.has_switch(Some(&cef_str(name))) != 0
}

fn add_switch(command_line: &mut CommandLine, name: &str) {
    if !has_switch(command_line, name) {
        command_line.append_switch(Some(&cef_str(name)));
    }
}

fn add_switch_value(command_line: &mut CommandLine, name: &str, value: &str) {
    if !has_switch(command_line, name) {
        command_line.append_switch_with_value(Some(&cef_str(name)), Some(&cef_str(value)));
    }
}

fn apply_switches(state: &AppState, command_line: &mut CommandLine) {
    for name in required_alloy_native_switches() {
        add_switch(command_line, name);
    }
    add_switch(command_line, "no-first-run");
    add_switch(command_line, "disable-background-networking");
    add_switch(command_line, "disable-component-update");
    add_switch(command_line, "disable-default-apps");
    add_switch(command_line, "disable-sync");
    add_switch(command_line, "disable-crash-reporter");
    add_switch(command_line, "disable-breakpad");

    let ozone = ozone_mode(state.args.health_check);
    add_switch_value(command_line, "ozone-platform", ozone.platform);
    if ozone.disable_gpu {
        add_switch(command_line, "disable-gpu");
    }
    if let Some(use_gl) = ozone.use_gl {
        add_switch_value(command_line, "use-gl", use_gl);
    }
    if let Some(use_angle) = ozone.use_angle {
        add_switch_value(command_line, "use-angle", use_angle);
    }

    if state.args.no_sandbox {
        add_switch(command_line, "no-sandbox");
        add_switch(command_line, "no-zygote");
    }
    if crate::shm::command_line_disables_dev_shm(state.disable_dev_shm, &state.args.extra_switches)
    {
        add_switch(command_line, crate::shm::DISABLE_DEV_SHM_USAGE);
    }
    if std::env::var("IDIOTEQUE_CEF_SOFTWARE_GL").is_ok_and(|v| v == "1") {
        add_switch(command_line, "disable-gpu-sandbox");
        add_switch_value(command_line, "use-gl", "angle");
        add_switch_value(command_line, "use-angle", "swiftshader");
        if !has_switch(command_line, "disable-features") {
            add_switch_value(
                command_line,
                "disable-features",
                "Vulkan,VaapiVideoDecoder,UseChromeOSDirectVideoDecoder",
            );
        }
    }
    if let Some(scale) = state.args.scale {
        let value = format!("{scale}");
        command_line.append_switch_with_value(
            Some(&cef_str("force-device-scale-factor")),
            Some(&cef_str(&value)),
        );
    }
    for raw in &state.args.extra_switches {
        let s = raw.trim();
        let s = s.strip_prefix("--").unwrap_or(s);
        if s.is_empty() {
            continue;
        }
        if let Some((k, v)) = s.split_once('=') {
            command_line.append_switch_with_value(Some(&cef_str(k)), Some(&cef_str(v)));
        } else {
            command_line.append_switch(Some(&cef_str(s)));
        }
    }
    command_line.append_switch_with_value(
        Some(&cef_str("ozone-platform")),
        Some(&cef_str(effective_ozone_platform(
            state.args.health_check,
            &state.args.extra_switches,
        ))),
    );
}

fn window_info_for(state: &AppState) -> WindowInfo {
    let args = &state.args;
    if args.health_check {
        return crate::health::window_info();
    }
    let bounds = Rect {
        x: args.bounds.x,
        y: args.bounds.y,
        width: platform::clamp_extent(args.bounds.w),
        height: platform::clamp_extent(args.bounds.h),
    };
    WindowInfo {
        runtime_style: RuntimeStyle::ALLOY,
        window_name: cef_str(platform::WINDOW_NAME),
        bounds,
        ..Default::default()
    }
}

fn window_info_is_toplevel(info: &WindowInfo) -> bool {
    info.parent_window == 0 && info.windowless_rendering_enabled == 0
}

fn make_client(state: Arc<AppState>) -> Client {
    let life = HostLifeSpan::new(state.clone());
    let load = HostLoad::new(state.clone());
    let display = HostDisplay::new(state.clone());
    let keyboard = HostKeyboard::new(state.clone());
    let menu = HostMenu::new(state.clone());
    let request = HostRequest::new();
    let render = if state.args.health_check {
        Some(crate::health::HealthRenderHandler::new())
    } else {
        None
    };
    HostClient::new(life, load, display, keyboard, menu, request, render)
}

wrap_task! {
    struct HostTask {
        cmd: HostCommand,
    }

    impl Task {
        fn execute(&self) {
            dispatch(&self.cmd);
        }
    }
}

wrap_app! {
    struct HostApp {
        state: Arc<AppState>,
        handler: BrowserProcessHandler,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(cmd) = command_line else {
                return;
            };
            apply_switches(&self.state, cmd);
            let _ = process_type;
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(self.handler.clone())
        }
    }
}

wrap_browser_process_handler! {
    struct HostBrowserProcessHandler {
        state: Arc<AppState>,
        client: Arc<Mutex<Option<Client>>>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let mut client = make_client(self.state.clone());
            if let Ok(mut slot) = self.client.lock() {
                *slot = Some(client.clone());
            }

            let url = cef_str(&self.state.args.url);
            let window_info = window_info_for(&self.state);
            let mut settings = BrowserSettings::default();
            if self.state.args.health_check {
                settings.windowless_frame_rate = 1;
                settings.background_color = 0xFF1C1E22;
                settings.webgl = State::DISABLED;
            }
            let ok = browser_host_create_browser(
                Some(&window_info),
                Some(&mut client),
                Some(&url),
                Some(&settings),
                None,
                None,
            );
            if ok == 0 {
                fatal(exit::INIT_FAILED, "browser_host_create_browser failed");
            }
        }

        fn on_before_child_process_launch(&self, command_line: Option<&mut CommandLine>) {
            let Some(cmd) = command_line else {
                return;
            };
            apply_switches(&self.state, cmd);
        }

        fn default_client(&self) -> Option<Client> {
            self.client.lock().ok().and_then(|g| g.clone())
        }
    }
}

wrap_client! {
    struct HostClient {
        life: LifeSpanHandler,
        load: LoadHandler,
        display: DisplayHandler,
        keyboard: KeyboardHandler,
        menu: ContextMenuHandler,
        request: RequestHandler,
        render: Option<RenderHandler>,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life.clone())
        }
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }
        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(self.keyboard.clone())
        }
        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.menu.clone())
        }
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.request.clone())
        }
        fn render_handler(&self) -> Option<RenderHandler> {
            self.render.clone()
        }
    }
}

wrap_life_span_handler! {
    struct HostLifeSpan {
        state: Arc<AppState>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let url = target_url.map(|u| u.to_string());
            let disposition = popup_disposition(url.as_deref());
            if let (Some(browser), Some(url)) = (browser, disposition.load_in_main.as_deref()) {
                if let Some(frame) = browser.main_frame() {
                    frame.load_url(Some(&cef_str(url)));
                }
            }
            i32::from(disposition.cancel)
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            if !take_main_browser(browser.is_popup() != 0, false) {
                return;
            }
            if self
                .state
                .ready_sent
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return;
            }
            let stored = browser.clone();
            self.state.main_id.store(browser.identifier(), Ordering::SeqCst);
            if let Ok(mut slot) = self.state.browser.lock() {
                *slot = Some(stored);
            }
            if !emit_ready_event(self.state.args.health_check) {
                return;
            }
            if let Some(host) = browser.host() {
                host.was_hidden(0);
            }
            let (cef, chromium, api_version) = self.state.versions();
            protocol::emit(&HostEvent::Ready {
                cef,
                chromium,
                api_version,
            });
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            let main = self.state.main_id.load(Ordering::SeqCst);
            if should_quit_on_before_close(browser.identifier(), main) {
                if let Ok(mut slot) = self.state.browser.lock() {
                    *slot = None;
                }
                quit_message_loop();
            }
        }
    }
}

wrap_load_handler! {
    struct HostLoad {
        state: Arc<AppState>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            browser: Option<&mut Browser>,
            _is_loading: ::std::os::raw::c_int,
            _can_go_back: ::std::os::raw::c_int,
            _can_go_forward: ::std::os::raw::c_int,
        ) {
            let Some(browser) = browser else {
                return;
            };
            self.state.emit_nav(browser, None);
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if frame.is_main() == 0 {
                return;
            }
            if let Some(browser) = browser.as_deref() {
                if !self.state.is_main(browser) {
                    return;
                }
            }
            if self.state.args.health_check {
                if self
                    .state
                    .health_emitted
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    if let Ok(guard) = self.state.health_cancel.lock() {
                        if let Some(flag) = guard.as_ref() {
                            flag.store(true, Ordering::SeqCst);
                        }
                    }
                    let (cef, chromium, api_version) = self.state.versions();
                    protocol::emit(&HostEvent::Health {
                        ok: true,
                        cef,
                        chromium,
                        api_version,
                    });
                    let plan = health_success_teardown();
                    if plan.close_browser {
                        if let Some(browser) = browser {
                            if let Some(host) = browser.host() {
                                host.close_browser(1);
                            }
                        }
                    }
                    if plan.quit_loop {
                        quit_message_loop();
                    }
                }
                return;
            }
            let _ = browser;
            protocol::emit(&HostEvent::LoadEnd {
                status: http_status_code,
            });
        }

        fn on_load_error(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let is_main = browser
                .as_ref()
                .map(|b| self.state.is_main(b))
                .unwrap_or(true);
            let is_main_frame = frame.as_ref().map(|f| f.is_main() != 0).unwrap_or(true);
            if !emit_load_error(
                self.state.args.health_check,
                is_main,
                is_main_frame,
                error_code.get_raw(),
            ) {
                return;
            }
            protocol::emit(&HostEvent::LoadError {
                code: error_code.get_raw(),
                text: error_text.map(|s| s.to_string()).unwrap_or_default(),
                url: failed_url.map(|s| s.to_string()).unwrap_or_default(),
            });
        }
    }
}

wrap_display_handler! {
    struct HostDisplay {
        state: Arc<AppState>,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            let Some(browser) = browser else {
                return;
            };
            self.state
                .emit_nav(browser, url.map(|u| u.to_string()));
        }

        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            let is_main = browser
                .as_ref()
                .map(|b| self.state.is_main(b))
                .unwrap_or(true);
            if !emit_chrome_ui_event(self.state.args.health_check, is_main) {
                return;
            }
            protocol::emit(&HostEvent::Title {
                title: title.map(|t| t.to_string()).unwrap_or_default(),
            });
        }
    }
}

wrap_keyboard_handler! {
    struct HostKeyboard {
        state: Arc<AppState>,
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: Option<&mut cef::sys::XEvent>,
            _is_keyboard_shortcut: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some(event) = event else {
                return 0;
            };
            let mods = event.modifiers;
            let ctrl = mods & FLAG_CTRL != 0;
            let shift = mods & FLAG_SHIFT != 0;
            let alt = mods & FLAG_ALT != 0;
            let action = pre_key_action(
                is_host_keydown(event.type_),
                event.windows_key_code,
                ctrl,
                shift,
                alt,
            );
            match action {
                PreKeyAction::Ignore => 0,
                PreKeyAction::Shortcut(chord) => {
                    let is_main = browser
                        .as_ref()
                        .map(|b| self.state.is_main(b))
                        .unwrap_or(true);
                    if let Some(event) =
                        shortcut_event(self.state.args.health_check, is_main, chord)
                    {
                        protocol::emit(&event);
                    }
                    1
                }
                PreKeyAction::ToggleDevtools => {
                    self.state.toggle_devtools(None);
                    1
                }
                PreKeyAction::Reload { ignore_cache } => {
                    if let Some(browser) = browser {
                        if ignore_cache {
                            browser.reload_ignore_cache();
                        } else {
                            browser.reload();
                        }
                    }
                    1
                }
                PreKeyAction::Back => {
                    if let Some(browser) = browser {
                        browser.go_back();
                    }
                    1
                }
                PreKeyAction::Forward => {
                    if let Some(browser) = browser {
                        browser.go_forward();
                    }
                    1
                }
                PreKeyAction::Stop => {
                    if let Some(browser) = browser {
                        browser.stop_load();
                    }
                    1
                }
            }
        }
    }
}

wrap_context_menu_handler! {
    struct HostMenu {
        state: Arc<AppState>,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(model) = model else {
                return;
            };
            let _ = model.add_separator();
            let _ = model.add_item(MENU_INSPECT, Some(&cef_str("Inspeccionar")));
            let _ = model.add_item(MENU_RELOAD, Some(&cef_str("Recargar")));
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            command_id: ::std::os::raw::c_int,
            _event_flags: EventFlags,
        ) -> ::std::os::raw::c_int {
            match command_id {
                MENU_INSPECT => {
                    let point = params.map(|p| Point {
                        x: p.xcoord(),
                        y: p.ycoord(),
                    });
                    self.state.toggle_devtools(point);
                    1
                }
                MENU_RELOAD => {
                    if let Some(b) = self.state.lock_browser() {
                        b.reload();
                    }
                    1
                }
                _ => 0,
            }
        }
    }
}

wrap_request_handler! {
    struct HostRequest;

    impl RequestHandler {
        fn on_render_process_terminated(
            &self,
            _browser: Option<&mut Browser>,
            status: TerminationStatus,
            _error_code: ::std::os::raw::c_int,
            error_string: Option<&CefString>,
        ) {
            let kind = render_term_from_status(status);
            let error = error_string.map(|s| s.to_string());
            protocol::emit(&HostEvent::RenderCrashed {
                status: render_crash_status(kind, error.as_deref()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Bounds, HostArgs};

    fn empty_args(health_check: bool) -> HostArgs {
        HostArgs {
            info: false,
            cef_dir: None,
            cache_dir: None,
            bounds: Bounds::default(),
            scale: None,
            url: "about:blank".into(),
            health_check,
            no_sandbox: false,
            log_file: None,
            extra_switches: Vec::new(),
        }
    }

    #[test]
    fn ozone_visible_is_wayland() {
        let mode = ozone_mode(false);
        assert_eq!(mode.platform, "wayland");
        assert_ne!(mode.platform, "headless");
        assert_eq!(ozone_platform(false), "wayland");
        assert!(!mode.disable_gpu);
    }

    #[test]
    fn ozone_health_is_headless() {
        let mode = ozone_mode(true);
        assert_eq!(mode.platform, "headless");
        assert_ne!(mode.platform, "wayland");
        assert!(mode.disable_gpu);
        assert_eq!(mode.use_gl, Some("angle"));
        assert_eq!(mode.use_angle, Some("swiftshader"));
    }

    #[test]
    fn extras_cannot_override_ozone() {
        let wayland = ["--ozone-platform=wayland".to_string()];
        let headless = ["--ozone-platform=headless".to_string()];
        assert_eq!(effective_ozone_platform(false, &headless), "wayland");
        assert_eq!(effective_ozone_platform(true, &wayland), "headless");
        assert_eq!(effective_ozone_platform(false, &[]), "wayland");
        assert_eq!(effective_ozone_platform(true, &[]), "headless");
    }

    #[test]
    fn alloy_native_required() {
        assert_eq!(
            required_alloy_native_switches(),
            &["use-alloy-style", "use-native"]
        );
    }

    #[test]
    fn window_info_visible_is_toplevel_alloy() {
        let manifest = Manifest {
            schema: Some(1),
            cef_version: "152.0.6+".into(),
            chromium_version: "152.0.7977.83".into(),
            platform: "linux64".into(),
            api_version_min: 13300,
            api_version_last: Some(15200),
            files: Vec::new(),
        };
        let state = AppState::new(empty_args(false), manifest, false);
        let info = window_info_for(&state);
        assert!(window_info_is_toplevel(&info));
        assert_eq!(info.runtime_style, RuntimeStyle::ALLOY);
        assert_eq!(info.window_name.to_string(), platform::WINDOW_NAME);
        assert_eq!(info.bounds.width, 1200);
        assert_eq!(info.bounds.height, 800);
    }

    #[test]
    fn close_plan_quits_and_closes_once() {
        assert_eq!(
            close_plan(false, true),
            ClosePlan {
                close_browser: true,
                quit_loop: true
            }
        );
        assert_eq!(
            close_plan(true, true),
            ClosePlan {
                close_browser: false,
                quit_loop: true
            }
        );
        assert_eq!(
            health_success_teardown(),
            ClosePlan {
                close_browser: false,
                quit_loop: true
            }
        );
    }

    #[test]
    fn shortcut_contract_chords_are_consumed() {
        assert_eq!(
            pre_key_action(true, VK_B, true, false, false),
            PreKeyAction::Shortcut("ctrl+b")
        );
        assert_eq!(
            pre_key_action(true, VK_B, true, true, false),
            PreKeyAction::Shortcut("ctrl+shift+b")
        );
        assert_eq!(pre_key_action(true, VK_L, true, false, false), PreKeyAction::Ignore);
        assert_eq!(
            pre_key_action(true, VK_F5, false, false, false),
            PreKeyAction::Reload {
                ignore_cache: false
            }
        );
        assert_eq!(
            pre_key_action(true, VK_F12, false, false, false),
            PreKeyAction::ToggleDevtools
        );
        assert_eq!(
            pre_key_action(true, VK_LEFT, false, false, true),
            PreKeyAction::Back
        );
        assert_eq!(
            pre_key_action(true, VK_RIGHT, false, false, true),
            PreKeyAction::Forward
        );
        assert_eq!(pre_key_action(true, VK_ESCAPE, false, false, false), PreKeyAction::Stop);
    }

    #[test]
    fn popup_loads_in_main_and_cancels() {
        let d = popup_disposition(Some("https://popup.test"));
        assert!(d.cancel);
        assert_eq!(d.load_in_main.as_deref(), Some("https://popup.test"));
    }

}
