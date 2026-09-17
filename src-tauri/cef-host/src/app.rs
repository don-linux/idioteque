//! CEF App / Client / handlers and UI-thread command dispatch.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
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

const MENU_INSPECT: i32 = 26500; // MENU_ID_USER_FIRST
const MENU_RELOAD: i32 = 26501;

const VK_ESCAPE: i32 = 0x1B;
const VK_LEFT: i32 = 0x25;
const VK_RIGHT: i32 = 0x27;
const VK_B: i32 = 0x42;
const VK_I: i32 = 0x49;
const VK_L: i32 = 0x4C;
const VK_R: i32 = 0x52;
const VK_F5: i32 = 0x74;
const VK_F12: i32 = 0x7B;

const FLAG_SHIFT: u32 = cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0;
const FLAG_CTRL: u32 = cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
const FLAG_ALT: u32 = cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;

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
    /// Ventana X intermedia (visual por defecto) entre el hueco del ADE y CEF.
    pub shim_xid: AtomicU64,
}

impl AppState {
    pub fn new(args: HostArgs, manifest: Manifest) -> Arc<Self> {
        Arc::new(Self {
            args,
            manifest,
            browser: Mutex::new(None),
            main_id: AtomicI32::new(0),
            ready_sent: AtomicBool::new(false),
            health_emitted: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            health_cancel: Mutex::new(None),
            shim_xid: AtomicU64::new(0),
        })
    }

    pub fn install(self: &Arc<Self>) {
        let _ = STATE.set(Arc::clone(self));
    }

    fn lock_browser(&self) -> Option<Browser> {
        self.browser.lock().ok().and_then(|g| g.clone())
    }

    fn xid(&self) -> u64 {
        self.lock_browser()
            .and_then(|b| b.host())
            .map(|h| platform::xid_from_handle(h.window_handle()))
            .unwrap_or(0)
    }

    fn versions(&self) -> (String, String, u32) {
        (
            self.manifest.cef_version.clone(),
            self.manifest.chromium_version.clone(),
            slot::HOST_API_VERSION,
        )
    }

    /// Solo el browser principal alimenta la barra: DevTools y otros popups no.
    fn is_main(&self, browser: &Browser) -> bool {
        let main = self.main_id.load(Ordering::SeqCst);
        main == 0 || browser.identifier() == main
    }

    fn emit_nav(&self, browser: &Browser, url: Option<String>) {
        if self.args.health_check || !self.is_main(browser) {
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
        if self
            .closing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            quit_message_loop();
            return;
        }
        if let Some(browser) = self.lock_browser() {
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }
        // Do not wait only on on_before_close: Ozone X11 child windows
        // sometimes never deliver it (software presenter errors).
        quit_message_loop();
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
        HostCommand::SetBounds { x, y, w, h } => {
            let xid = state.xid();
            let shim = state.shim_xid.load(Ordering::SeqCst);
            let (cx, cy) = if shim != 0 {
                platform::move_resize(shim, *x, *y, *w, *h);
                (0, 0)
            } else {
                (*x, *y)
            };
            if let Some(browser) = state.lock_browser() {
                if let Some(host) = browser.host() {
                    host.notify_move_or_resize_started();
                    platform::move_resize(xid, cx, cy, *w, *h);
                    host.was_resized();
                }
            } else {
                platform::move_resize(xid, cx, cy, *w, *h);
            }
        }
        HostCommand::Show => {
            let shim = state.shim_xid.load(Ordering::SeqCst);
            if shim != 0 {
                platform::map_window(shim);
            }
            platform::map_window(state.xid());
            if let Some(browser) = state.lock_browser() {
                if let Some(host) = browser.host() {
                    host.was_hidden(0);
                }
            }
        }
        HostCommand::Hide => {
            platform::unmap_window(state.xid());
            let shim = state.shim_xid.load(Ordering::SeqCst);
            if shim != 0 {
                platform::unmap_window(shim);
            }
            if let Some(browser) = state.lock_browser() {
                if let Some(host) = browser.host() {
                    host.was_hidden(1);
                }
            }
        }
        HostCommand::Focus => {
            platform::focus_window(state.xid());
            if let Some(browser) = state.lock_browser() {
                if let Some(host) = browser.host() {
                    host.set_focus(1);
                }
            }
        }
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
    // Native windows, not Chrome-runtime Views (Views needs a GPU compositor
    // that deadlocks on this Xvnc without DRI3 during CefInitialize).
    add_switch(command_line, "use-alloy-style");
    add_switch(command_line, "use-native");
    add_switch(command_line, "no-first-run");
    add_switch(command_line, "disable-background-networking");
    add_switch(command_line, "disable-component-update");
    add_switch(command_line, "disable-default-apps");
    add_switch(command_line, "disable-sync");
    add_switch(command_line, "disable-crash-reporter");
    add_switch(command_line, "disable-breakpad");

    if state.args.health_check {
        add_switch_value(command_line, "ozone-platform", "headless");
        add_switch(command_line, "disable-gpu");
        add_switch_value(command_line, "use-gl", "angle");
        add_switch_value(command_line, "use-angle", "swiftshader");
    } else {
        add_switch_value(command_line, "ozone-platform", "x11");
    }

    if state.args.no_sandbox {
        add_switch(command_line, "no-sandbox");
        add_switch(command_line, "no-zygote");
        add_switch(command_line, "disable-dev-shm-usage");
    }
    // Software GL for X servers without DRI3 (the dev VM). A user machine that
    // merely lacks the sandbox keeps its GPU: pass these via IDIOTEQUE_CEF_ARGS
    // or IDIOTEQUE_CEF_SOFTWARE_GL=1 when needed.
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
}

fn window_info_for(state: &AppState) -> WindowInfo {
    let args = &state.args;
    if args.health_check {
        return crate::health::window_info();
    }
    let bounds = Rect {
        x: args.bounds.x,
        y: args.bounds.y,
        width: args.bounds.w.max(1),
        height: args.bounds.h.max(1),
    };
    let base = WindowInfo {
        runtime_style: RuntimeStyle::ALLOY,
        window_name: cef_str("idioteque"),
        bounds: bounds.clone(),
        ..Default::default()
    };
    if let Some(parent) = args.parent {
        let shim = platform::create_default_visual_child(parent, bounds.width, bounds.height);
        if shim == 0 {
            fatal(exit::NO_X11, "no se pudo crear la ventana intermedia X11");
        }
        state.shim_xid.store(shim, Ordering::SeqCst);
        let inner = Rect {
            x: 0,
            y: 0,
            width: bounds.width,
            height: bounds.height,
        };
        base.set_as_child(shim as cef::sys::cef_window_handle_t, &inner)
    } else {
        base
    }
}

fn make_client(state: Arc<AppState>) -> Client {
    let life = HostLifeSpan::new(state.clone());
    let load = HostLoad::new(state.clone());
    let display = HostDisplay::new(state.clone());
    let keyboard = HostKeyboard::new(state.clone());
    let menu = HostMenu::new(state.clone());
    let request = HostRequest::new(state.clone());
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
            if let (Some(browser), Some(url)) = (browser, target_url) {
                if let Some(frame) = browser.main_frame() {
                    frame.load_url(Some(url));
                }
            }
            1
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            if browser.is_popup() != 0 {
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
            if self.state.args.health_check {
                return;
            }
            let xid = browser
                .host()
                .map(|h| platform::xid_from_handle(h.window_handle()))
                .unwrap_or(0);
            if self.state.args.parent.is_some() {
                let b = &self.state.args.bounds;
                let shim = self.state.shim_xid.load(Ordering::SeqCst);
                platform::reparent(xid, shim, 0, 0);
                platform::move_resize(xid, 0, 0, b.w, b.h);
                platform::map_window(xid);
                if let Some(host) = browser.host() {
                    host.notify_move_or_resize_started();
                    host.was_resized();
                    host.was_hidden(0);
                }
            }
            let (cef, chromium, api_version) = self.state.versions();
            protocol::emit(&HostEvent::Ready {
                cef,
                chromium,
                api_version,
                xid,
            });
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            let main = self.state.main_id.load(Ordering::SeqCst);
            if browser.identifier() == main || main == 0 {
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
                    // Windowless/headless teardown via close_browser often SIGTRAPs
                    // on this CEF/X11 combo. Quitting the loop is enough; main
                    // then shuts down and exits 0.
                    quit_message_loop();
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
            if self.state.args.health_check {
                return;
            }
            if let Some(browser) = browser {
                if !self.state.is_main(browser) {
                    return;
                }
            }
            if let Some(frame) = frame {
                if frame.is_main() == 0 {
                    return;
                }
            }
            if error_code == Errorcode::ABORTED {
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
            if self.state.args.health_check {
                return;
            }
            if let Some(browser) = browser {
                if !self.state.is_main(browser) {
                    return;
                }
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
            if event.type_ != KeyEventType::RAWKEYDOWN {
                return 0;
            }
            let mods = event.modifiers;
            let ctrl = mods & FLAG_CTRL != 0;
            let shift = mods & FLAG_SHIFT != 0;
            let alt = mods & FLAG_ALT != 0;
            let key = event.windows_key_code;

            if ctrl && !alt && key == VK_B {
                let chord = if shift { "ctrl+shift+b" } else { "ctrl+b" };
                protocol::emit(&HostEvent::Shortcut {
                    chord: chord.to_string(),
                });
                return 1;
            }
            if ctrl && !alt && !shift && key == VK_L {
                protocol::emit(&HostEvent::Shortcut {
                    chord: "ctrl+l".to_string(),
                });
                return 1;
            }
            if key == VK_F12 || (ctrl && shift && !alt && key == VK_I) {
                self.state.toggle_devtools(None);
                return 1;
            }
            if key == VK_F5 || (ctrl && !alt && key == VK_R) {
                if let Some(browser) = browser {
                    if ctrl && shift && key == VK_R {
                        browser.reload_ignore_cache();
                    } else {
                        browser.reload();
                    }
                }
                return 1;
            }
            if alt && !ctrl && key == VK_LEFT {
                if let Some(browser) = browser {
                    browser.go_back();
                }
                return 1;
            }
            if alt && !ctrl && key == VK_RIGHT {
                if let Some(browser) = browser {
                    browser.go_forward();
                }
                return 1;
            }
            if key == VK_ESCAPE {
                if let Some(browser) = browser {
                    browser.stop_load();
                }
                return 1;
            }
            0
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
    struct HostRequest {
        state: Arc<AppState>,
    }

    impl RequestHandler {
        fn on_render_process_terminated(
            &self,
            _browser: Option<&mut Browser>,
            status: TerminationStatus,
            _error_code: ::std::os::raw::c_int,
            error_string: Option<&CefString>,
        ) {
            let status = if status == TerminationStatus::PROCESS_CRASHED {
                "crashed".to_string()
            } else if status == TerminationStatus::PROCESS_WAS_KILLED {
                "killed".to_string()
            } else if status == TerminationStatus::ABNORMAL_TERMINATION {
                "abnormal".to_string()
            } else if status == TerminationStatus::PROCESS_OOM {
                "oom".to_string()
            } else if status == TerminationStatus::LAUNCH_FAILED {
                "launch-failed".to_string()
            } else if let Some(s) = error_string {
                s.to_string()
            } else {
                format!("{}", status.get_raw())
            };
            protocol::emit(&HostEvent::RenderCrashed { status });
        }
    }
}
