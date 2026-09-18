//! CEF App / Client / handlers and UI-thread command dispatch.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use cef::sys::cef_event_flags_t;
use cef::{
    wrap_app, wrap_browser_process_handler, wrap_client, wrap_context_menu_handler,
    wrap_display_handler, wrap_focus_handler, wrap_keyboard_handler, wrap_life_span_handler,
    wrap_load_handler, wrap_request_handler, wrap_task, *,
};

use crate::args::HostArgs;
use crate::exit::{self, fatal};
use crate::platform;
use crate::protocol::{self, FocusOwner, HostCommand, HostEvent};
use crate::slot::{self, Manifest};

const MENU_INSPECT: i32 = 26500; // MENU_ID_USER_FIRST
const MENU_RELOAD: i32 = 26501;

const VK_TAB: i32 = 0x09;
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

/// Embed stays Ozone X11 (XWayland). Health is windowless and must not open
/// an X11 Ozone window. Native Wayland is not the embed path.
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
            platform: "x11",
            disable_gpu: false,
            use_gl: None,
            use_angle: None,
        }
    }
}

fn ozone_platform(health_check: bool) -> &'static str {
    ozone_mode(health_check).platform
}

/// `IDIOTEQUE_CEF_ARGS` cannot flip embed to Wayland or health to ozone-x11.
/// Windowless health + `--ozone-platform=x11` is the combo that fails this
/// CEF; native Wayland embed is not supported.
fn effective_ozone_platform(health_check: bool, extra_switches: &[String]) -> &'static str {
    let _ = extra_switches;
    ozone_platform(health_check)
}

fn required_alloy_native_switches() -> &'static [&'static str] {
    &["use-alloy-style", "use-native"]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PopupDisposition {
    /// CEF: non-zero cancels the new window.
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

/// First `close`: `close_browser` if we have one, then always quit.
/// Already closing: quit again. Ozone X11 children often never fire
/// `on_before_close` (software presenter errors), so waiting on that
/// callback alone would hang the host.
fn close_plan(already_closing: bool, has_browser: bool) -> ClosePlan {
    ClosePlan {
        close_browser: !already_closing && has_browser,
        quit_loop: true,
    }
}

fn should_quit_on_before_close(browser_id: i32, main_id: i32) -> bool {
    main_id == 0 || browser_id == main_id
}

/// Health success: emit `health` and quit the loop. Do not `close_browser`
/// — windowless/headless teardown via that path SIGTRAPs on this CEF.
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

/// Alloy native children wrap Tab inside the document, so `OnTakeFocus` often
/// never fires. The trap beacons this prefix (console) or the custom scheme.
const TAKE_FOCUS_BEACON: &str = "idioteque:take-focus:";
const TAKE_FOCUS_URL: &str = "idioteque://chrome/take-focus?next=";

pub(crate) const TAKE_FOCUS_SCRIPT: &str = r#"(function(){
  function visible(el){
    if (el.tabIndex < 0) return false;
    var st = window.getComputedStyle(el);
    if (!st || st.visibility === 'hidden' || st.display === 'none') return false;
    return el.getClientRects().length > 0;
  }
  function list(){
    var sel = 'a[href],button:not([disabled]),input:not([disabled]):not([type="hidden"]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
    return Array.prototype.filter.call(document.querySelectorAll(sel), visible);
  }
  function beacon(next){
    try { var a = document.activeElement; if (a && a.blur) a.blur(); } catch (e) {}
    try { console.info('idioteque:take-focus:' + (next ? '1' : '0')); } catch (e) {}
    try { location.assign('idioteque://chrome/take-focus?next=' + (next ? '1' : '0')); } catch (e) {}
  }
  window.__idiotequeHandleTab = function(forward){
    var items = list();
    var active = document.activeElement;
    var empty = items.length === 0;
    var first = empty ? null : items[0];
    var last = empty ? null : items[items.length - 1];
    var atStart = empty || !active || active === document.body || active === document.documentElement || active === first;
    var atEnd = empty || active === last;
    if ((forward && atEnd) || (!forward && atStart)) {
      beacon(!!forward);
      return true;
    }
    if (empty) return false;
    var i = items.indexOf(active);
    var target = i < 0 ? (forward ? first : last) : items[i + (forward ? 1 : -1)];
    if (target) target.focus();
    return false;
  };
  if (window.__idiotequeTakeFocus) return;
  window.__idiotequeTakeFocus = 1;
  window.addEventListener('keydown', function(e){
    if (e.key !== 'Tab' || e.ctrlKey || e.altKey || e.metaKey || e.isComposing) return;
    if (window.__idiotequeHandleTab(!e.shiftKey)) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  }, true);
})();"#;

fn parse_take_focus_beacon(message: &str) -> Option<bool> {
    let text = message.trim();
    if let Some(rest) = text.strip_prefix(TAKE_FOCUS_BEACON) {
        return match rest {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        };
    }
    if let Some(rest) = text.strip_prefix(TAKE_FOCUS_URL) {
        return match rest.chars().next() {
            Some('1') => Some(true),
            Some('0') => Some(false),
            _ => None,
        };
    }
    if !text.starts_with("idioteque:") || !text.contains("take-focus") {
        return None;
    }
    if let Some(rest) = text.split("next=").nth(1) {
        return match rest.chars().next() {
            Some('1') => Some(true),
            Some('0') => Some(false),
            _ => None,
        };
    }
    None
}

fn take_focus_from_beacon(health_check: bool, is_main: bool, message: &str) -> Option<HostEvent> {
    parse_take_focus_beacon(message)
        .and_then(|next| focus_handoff_event(health_check, is_main, FocusHandoff::App { next }))
}

fn should_inject_take_focus_trap(
    health_check: bool,
    is_main_browser: bool,
    is_main_frame: bool,
) -> bool {
    !health_check && is_main_browser && is_main_frame
}

pub(crate) fn inject_take_focus_trap(frame: &Frame) {
    frame.execute_java_script(
        Some(&cef_str(TAKE_FOCUS_SCRIPT)),
        Some(&cef_str("idioteque-take-focus-trap")),
        1,
    );
}

fn tab_dispatch_script(next: bool) -> String {
    let flag = if next { "true" } else { "false" };
    format!(
        "(function(){{if(typeof window.__idiotequeHandleTab==='function'){{window.__idiotequeHandleTab({flag});return;}}{TAKE_FOCUS_SCRIPT}if(typeof window.__idiotequeHandleTab==='function')window.__idiotequeHandleTab({flag});}})();"
    )
}

fn dispatch_tab_in_page(browser: Option<&mut Browser>, next: bool) {
    let Some(browser) = browser else {
        return;
    };
    let Some(frame) = browser.main_frame() else {
        return;
    };
    let script = tab_dispatch_script(next);
    frame.execute_java_script(
        Some(&cef_str(&script)),
        Some(&cef_str("idioteque-tab-dispatch")),
        1,
    );
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

/// Keyboard handoff from `CefFocusHandler`. Main browser only, same filter as nav/title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusHandoff {
    Browser,
    App { next: bool },
}

fn focus_handoff_event(
    health_check: bool,
    is_main: bool,
    handoff: FocusHandoff,
) -> Option<HostEvent> {
    if !emit_chrome_ui_event(health_check, is_main) {
        return None;
    }
    Some(match handoff {
        FocusHandoff::Browser => HostEvent::Focus {
            owner: FocusOwner::Browser,
            next: None,
        },
        FocusHandoff::App { next } => HostEvent::Focus {
            owner: FocusOwner::App,
            next: Some(next),
        },
    })
}

/// `{"cmd":"focus"}` moves X11 onto the CEF child. `{"cmd":"unfocus"}` must not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BrowserFocusPlan {
    x11: bool,
    set_focus: i32,
}

fn give_browser_focus_plan() -> BrowserFocusPlan {
    BrowserFocusPlan {
        x11: true,
        set_focus: 1,
    }
}

fn drop_browser_focus_plan() -> BrowserFocusPlan {
    BrowserFocusPlan {
        x11: false,
        set_focus: 0,
    }
}

fn should_ungrab_on_unfocus() -> bool {
    true
}

fn should_drop_modified_char(kind: KeyEventType, ctrl: bool, alt: bool) -> bool {
    kind == KeyEventType::CHAR && (ctrl || alt)
}

fn printable_char(raw: u32) -> Option<char> {
    let ch = char::from_u32(raw)?;
    if ch.is_control() || ch == '\0' {
        None
    } else {
        Some(ch)
    }
}

fn vk_typed_char(windows: i32, shift: bool) -> Option<char> {
    let key = host_key_code(windows);
    match key {
        0x20 => Some(' '),
        0x30..=0x39 if !shift => char::from_u32(key as u32),
        0x41..=0x5A => {
            let letter = char::from_u32(key as u32)?;
            Some(if shift {
                letter
            } else {
                letter.to_ascii_lowercase()
            })
        }
        _ => None,
    }
}

/// CHAR is dropped when we consume KEYDOWN, so read the glyph from keydown too.
fn key_text(event: &KeyEvent) -> Option<String> {
    if event.modifiers & (FLAG_CTRL | FLAG_ALT) != 0 {
        return None;
    }
    if let Some(ch) = printable_char(event.character as u32) {
        if event.type_ == KeyEventType::CHAR || is_host_keydown(event.type_) {
            return Some(ch.to_string());
        }
    }
    if !is_host_keydown(event.type_) {
        return None;
    }
    vk_typed_char(
        event.windows_key_code,
        event.modifiers & FLAG_SHIFT != 0,
    )
    .map(|ch| ch.to_string())
}

fn keys_event(health_check: bool, is_main: bool, text: String) -> Option<HostEvent> {
    if !emit_chrome_ui_event(health_check, is_main) || text.is_empty() {
        return None;
    }
    Some(HostEvent::Keys { text })
}

fn apply_browser_focus(state: &AppState, plan: BrowserFocusPlan) {
    state
        .app_owns_keyboard
        .store(plan.set_focus == 0, Ordering::SeqCst);
    if plan.x11 {
        platform::focus_window(state.xid());
    }
    if plan.set_focus == 0 && should_ungrab_on_unfocus() {
        eprintln!("[cef] ungrab host X11");
        platform::ungrab_input();
    }
    if let Some(browser) = state.lock_browser() {
        if let Some(host) = browser.host() {
            host.set_focus(plan.set_focus);
        }
        if plan.set_focus == 0 {
            blur_page_keyboard(&browser);
        }
    }
}

const BLUR_PAGE_SCRIPT: &str =
    r#"(function(){try{var a=document.activeElement;if(a&&a.blur)a.blur();}catch(e){}})();"#;

fn blur_page_keyboard(browser: &Browser) {
    let Some(frame) = browser.main_frame() else {
        return;
    };
    frame.execute_java_script(
        Some(&cef_str(BLUR_PAGE_SCRIPT)),
        Some(&cef_str("idioteque-blur-page")),
        1,
    );
}

fn take_focus_should_emit(last: &Mutex<Option<(bool, Instant)>>, next: bool) -> bool {
    let Ok(mut guard) = last.lock() else {
        return true;
    };
    if let Some((prev, at)) = *guard {
        if prev == next && at.elapsed() < Duration::from_millis(80) {
            return false;
        }
    }
    *guard = Some((next, Instant::now()));
    true
}

fn emit_take_focus(state: &AppState, is_main: bool, message: &str) -> bool {
    let Some(event) = take_focus_from_beacon(state.args.health_check, is_main, message) else {
        return false;
    };
    let next = match &event {
        HostEvent::Focus {
            next: Some(value), ..
        } => *value,
        _ => true,
    };
    if !take_focus_should_emit(&state.last_take_focus, next) {
        eprintln!("[cef] take-focus debounce next={next}");
        return true;
    }
    state.app_owns_keyboard.store(true, Ordering::SeqCst);
    protocol::emit(&event);
    true
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
    Tab { next: bool },
    ToggleDevtools,
    Reload { ignore_cache: bool },
    Back,
    Forward,
    Stop,
}

/// While chrome owns the caret, Ozone still sees keys (pointer stays over the
/// child; `set_focus(0)` is often a no-op). Swallow page keys; keep shortcuts.
fn should_swallow_page_key(app_owns_keyboard: bool, action: PreKeyAction) -> bool {
    app_owns_keyboard && !matches!(action, PreKeyAction::Shortcut(_))
}

/// `OnGotFocus` / `focus owner=browser`: chrome no longer swallows page keys.
fn app_owns_keyboard_after_handoff(handoff: FocusHandoff) -> bool {
    matches!(handoff, FocusHandoff::App { .. })
}

/// Enter in the URL bar: the loaded page owns keys; stop forwarding glyphs.
fn app_owns_keyboard_after_navigate() -> bool {
    false
}

fn pre_key_action(keydown: bool, key: i32, ctrl: bool, shift: bool, alt: bool) -> PreKeyAction {
    if !keydown {
        return PreKeyAction::Ignore;
    }
    let key = host_key_code(key);
    if !ctrl && !alt && key == VK_TAB {
        return PreKeyAction::Tab { next: !shift };
    }
    if ctrl && !alt && key == VK_B {
        return PreKeyAction::Shortcut(if shift { "ctrl+shift+b" } else { "ctrl+b" });
    }
    if ctrl && !alt && !shift && key == VK_L {
        return PreKeyAction::Shortcut("ctrl+l");
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
    /// Ventana X intermedia (visual por defecto) entre el hueco del ADE y CEF.
    pub shim_xid: AtomicU64,
    /// `--disable-dev-shm-usage`: solo si `/dev/shm` no sirve (`shm::decide`).
    pub disable_dev_shm: bool,
    /// ADE chrome owns keys (`unfocus`). Ozone still delivers page keys until
    /// a real click, so `on_pre_key_event` swallows them.
    pub app_owns_keyboard: AtomicBool,
    last_take_focus: Mutex<Option<(bool, Instant)>>,
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
            shim_xid: AtomicU64::new(0),
            app_owns_keyboard: AtomicBool::new(false),
            last_take_focus: Mutex::new(None),
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
        is_main_browser(self.main_id.load(Ordering::SeqCst), browser.identifier())
    }

    fn emit_focus(&self, browser: &Browser, handoff: FocusHandoff) {
        if let Some(event) =
            focus_handoff_event(self.args.health_check, self.is_main(browser), handoff)
        {
            self.app_owns_keyboard
                .store(app_owns_keyboard_after_handoff(handoff), Ordering::SeqCst);
            protocol::emit(&event);
        }
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
        // Do not wait only on on_before_close: Ozone X11 child windows
        // sometimes never deliver it (software presenter errors).
        if plan.quit_loop {
            quit_message_loop();
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
            state
                .app_owns_keyboard
                .store(app_owns_keyboard_after_navigate(), Ordering::SeqCst);
            if let Some(browser) = state.lock_browser() {
                if let Some(frame) = browser.main_frame() {
                    frame.load_url(Some(&cef_str(url)));
                }
                if let Some(host) = browser.host() {
                    host.set_focus(1);
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
        HostCommand::Focus => apply_browser_focus(state, give_browser_focus_plan()),
        HostCommand::Unfocus => apply_browser_focus(state, drop_browser_focus_plan()),
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
    // Official Chromium workaround (`kDisableDevShmUsage` / crbug/715363) only
    // when `/dev/shm` is unusable (permissions, ENOSPC, EDQUOT) or when
    // `IDIOTEQUE_CEF_ARGS` forces the flag. Docker's 64 MiB default is one
    // failure case, not product policy. Never tied to the sandbox: see shm.rs.
    if crate::shm::command_line_disables_dev_shm(state.disable_dev_shm, &state.args.extra_switches)
    {
        add_switch(command_line, crate::shm::DISABLE_DEV_SHM_USAGE);
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
    // After extras: lock ozone so IDIOTEQUE_CEF_ARGS cannot move embed to
    // Wayland or health to ozone-x11. Chromium's switch map keeps last write.
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
    let focus = HostFocus::new(state.clone());
    HostClient::new(life, load, display, keyboard, menu, request, render, focus)
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
        focus: FocusHandler,
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
        fn focus_handler(&self) -> Option<FocusHandler> {
            Some(self.focus.clone())
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
            if disposition.cancel {
                1
            } else {
                0
            }
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

        fn on_load_start(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if frame.is_main() == 0 {
                return;
            }
            let is_main = browser
                .as_ref()
                .map(|b| self.state.is_main(b))
                .unwrap_or(true);
            if should_inject_take_focus_trap(self.state.args.health_check, is_main, true) {
                inject_take_focus_trap(frame);
            }
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
            let is_main_browser = browser
                .as_ref()
                .map(|b| self.state.is_main(b))
                .unwrap_or(true);
            if should_inject_take_focus_trap(self.state.args.health_check, is_main_browser, true)
            {
                inject_take_focus_trap(frame);
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

        fn on_console_message(
            &self,
            browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(message) = message else {
                return 0;
            };
            let is_main = browser
                .as_ref()
                .map(|b| self.state.is_main(b))
                .unwrap_or(true);
            let text = message.to_string();
            if !emit_take_focus(&self.state, is_main, &text) {
                return 0;
            }
            eprintln!("[cef] take-focus console {text}");
            1
        }
    }
}

wrap_focus_handler! {
    struct HostFocus {
        state: Arc<AppState>,
    }

    impl FocusHandler {
        fn on_got_focus(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            self.state
                .app_owns_keyboard
                .store(false, Ordering::SeqCst);
            self.state.emit_focus(browser, FocusHandoff::Browser);
        }

        fn on_take_focus(&self, browser: Option<&mut Browser>, next: ::std::os::raw::c_int) {
            let Some(browser) = browser else {
                return;
            };
            self.state
                .emit_focus(browser, FocusHandoff::App { next: next != 0 });
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
            os_event: Option<&mut cef::sys::XEvent>,
            _is_keyboard_shortcut: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some(event) = event else {
                return 0;
            };
            let mods = event.modifiers;
            let ctrl = mods & FLAG_CTRL != 0;
            let shift = mods & FLAG_SHIFT != 0;
            let alt = mods & FLAG_ALT != 0;
            if should_drop_modified_char(event.type_, ctrl, alt) {
                eprintln!(
                    "[cef] drop modified CHAR win={:#x}",
                    event.windows_key_code
                );
                return 1;
            }
            let action = pre_key_action(
                is_host_keydown(event.type_),
                event.windows_key_code,
                ctrl,
                shift,
                alt,
            );
            if ctrl {
                eprintln!(
                    "[cef] pre-key ctrl type={:?} win={:#x} action={action:?}",
                    event.type_, event.windows_key_code
                );
            }
            if should_swallow_page_key(
                self.state.app_owns_keyboard.load(Ordering::SeqCst),
                action,
            ) {
                let is_main = browser
                    .as_ref()
                    .map(|b| self.state.is_main(b))
                    .unwrap_or(true);
                if let Some(text) = key_text(event) {
                    if let Some(event) =
                        keys_event(self.state.args.health_check, is_main, text.clone())
                    {
                        eprintln!("[cef] emit keys {text:?}");
                        protocol::emit(&event);
                    }
                }
                let _ = os_event;
                eprintln!(
                    "[cef] swallow page key win={:#x} while app owns keyboard",
                    event.windows_key_code
                );
                return 1;
            }
            match action {
                PreKeyAction::Ignore => 0,
                PreKeyAction::Tab { next } => {
                    eprintln!("[cef] tab-edge dispatch next={next}");
                    dispatch_tab_in_page(browser, next);
                    1
                }
                PreKeyAction::Shortcut(chord) => {
                    let is_main = browser
                        .as_ref()
                        .map(|b| self.state.is_main(b))
                        .unwrap_or(true);
                    if let Some(event) =
                        shortcut_event(self.state.args.health_check, is_main, chord)
                    {
                        self.state
                            .app_owns_keyboard
                            .store(true, Ordering::SeqCst);
                        eprintln!("[cef] emit shortcut {chord}");
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
    struct HostRequest {
        state: Arc<AppState>,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(request) = request else {
                return 0;
            };
            let url = CefString::from(&request.url()).to_string();
            let is_main = browser
                .as_ref()
                .map(|b| self.state.is_main(b))
                .unwrap_or(true);
            if !emit_take_focus(&self.state, is_main, &url) {
                return 0;
            }
            eprintln!("[cef] take-focus beacon {url}");
            1
        }

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

    #[test]
    fn ozone_embed_is_x11_not_wayland_or_headless() {
        let mode = ozone_mode(false);
        assert_eq!(mode.platform, "x11");
        assert_ne!(mode.platform, "wayland");
        assert_ne!(mode.platform, "headless");
        assert_eq!(ozone_platform(false), "x11");
        assert!(!mode.disable_gpu);
        assert_eq!(mode.use_gl, None);
        assert_eq!(mode.use_angle, None);
    }

    #[test]
    fn ozone_health_is_headless_not_x11() {
        let mode = ozone_mode(true);
        assert_eq!(mode.platform, "headless");
        assert_ne!(mode.platform, "x11");
        assert_ne!(mode.platform, "wayland");
        assert_eq!(ozone_platform(true), "headless");
        assert!(mode.disable_gpu);
        assert_eq!(mode.use_gl, Some("angle"));
        assert_eq!(mode.use_angle, Some("swiftshader"));
    }

    #[test]
    fn ozone_modes_are_mutually_exclusive() {
        assert_ne!(ozone_platform(false), ozone_platform(true));
        assert_ne!(ozone_mode(false), ozone_mode(true));
    }

    #[test]
    fn ozone_does_not_depend_on_shm_policy() {
        // shm/--disable-dev-shm-usage is another slice. ozone_mode takes
        // only health_check — both modes stay valid with the shm switch on or off.
        assert_eq!(ozone_mode(false).platform, ozone_platform(false));
        assert_eq!(ozone_mode(true).platform, ozone_platform(true));
        assert_ne!(ozone_platform(false), ozone_platform(true));
    }

    fn consumes_key(action: PreKeyAction) -> bool {
        !matches!(action, PreKeyAction::Ignore)
    }

    #[test]
    fn extras_cannot_override_ozone_to_wayland_or_swap_mode() {
        let wayland = ["--ozone-platform=wayland".to_string()];
        let x11 = ["--ozone-platform=x11".to_string()];
        let headless = ["ozone-platform=headless".to_string()];
        assert_eq!(effective_ozone_platform(false, &wayland), "x11");
        assert_eq!(effective_ozone_platform(true, &wayland), "headless");
        assert_eq!(effective_ozone_platform(true, &x11), "headless");
        assert_eq!(effective_ozone_platform(false, &headless), "x11");
    }

    #[test]
    fn alloy_native_required_for_embed_and_health() {
        let switches = required_alloy_native_switches();
        assert!(switches.contains(&"use-alloy-style"));
        assert!(switches.contains(&"use-native"));
        assert!(!switches.contains(&"ozone-platform"));
    }

    #[test]
    fn popup_always_cancels_and_loads_main() {
        let with_url = popup_disposition(Some("https://example.test/a"));
        assert!(with_url.cancel);
        assert_eq!(
            with_url.load_in_main.as_deref(),
            Some("https://example.test/a")
        );

        let empty = popup_disposition(Some(""));
        assert!(empty.cancel);
        assert_eq!(empty.load_in_main.as_deref(), Some(""));

        let none = popup_disposition(None);
        assert!(none.cancel);
        assert_eq!(none.load_in_main, None);

        let js = popup_disposition(Some("javascript:alert(1)"));
        assert!(js.cancel);
        assert_eq!(js.load_in_main.as_deref(), Some("javascript:alert(1)"));
    }

    #[test]
    fn close_always_quits_when_on_before_close_is_missing() {
        let first = close_plan(false, true);
        assert!(first.close_browser);
        assert!(
            first.quit_loop,
            "Ozone X11 child may never fire on_before_close"
        );

        let no_browser = close_plan(false, false);
        assert!(!no_browser.close_browser);
        assert!(no_browser.quit_loop);

        let again = close_plan(true, true);
        assert!(
            !again.close_browser,
            "already closing must not close_browser again"
        );
        assert!(again.quit_loop);
    }

    #[test]
    fn before_close_quits_only_for_main_browser() {
        assert!(should_quit_on_before_close(7, 7));
        assert!(should_quit_on_before_close(3, 0));
        assert!(
            !should_quit_on_before_close(99, 7),
            "DevTools/popup close must not quit the host"
        );
    }

    #[test]
    fn health_success_quits_without_close_browser() {
        let plan = health_success_teardown();
        assert!(!plan.close_browser);
        assert!(plan.quit_loop);
        assert!(!emit_ready_event(true));
        assert!(emit_ready_event(false));
        assert!(take_main_browser(false, false));
        assert!(!take_main_browser(true, false));
        assert!(!take_main_browser(false, true));
    }

    #[test]
    fn shortcut_contract_chords_are_consumed() {
        for (shift, chord) in [(false, "ctrl+b"), (true, "ctrl+shift+b")] {
            let action = pre_key_action(true, VK_B, true, shift, false);
            assert_eq!(action, PreKeyAction::Shortcut(chord));
            assert!(consumes_key(action));
        }
        let ctrl_l = pre_key_action(true, VK_L, true, false, false);
        assert_eq!(ctrl_l, PreKeyAction::Shortcut("ctrl+l"));
        assert!(consumes_key(ctrl_l));
    }

    #[test]
    fn shortcut_modifiers_do_not_false_positive() {
        assert_eq!(
            pre_key_action(true, VK_B, true, false, true),
            PreKeyAction::Ignore,
            "ctrl+alt+b is not a host shortcut"
        );
        assert_eq!(
            pre_key_action(true, VK_L, true, true, false),
            PreKeyAction::Ignore,
            "ctrl+shift+l is not ctrl+l"
        );
        assert_eq!(
            pre_key_action(true, VK_L, false, false, false),
            PreKeyAction::Ignore
        );
        assert_eq!(
            pre_key_action(true, VK_B, false, false, false),
            PreKeyAction::Ignore
        );
        assert_eq!(
            pre_key_action(false, VK_B, true, false, false),
            PreKeyAction::Ignore,
            "only RAWKEYDOWN"
        );
        assert_eq!(
            pre_key_action(true, VK_I, false, false, false),
            PreKeyAction::Ignore
        );
    }

    #[test]
    fn reload_back_forward_stop_devtools_keys() {
        assert_eq!(
            pre_key_action(true, VK_F12, false, false, false),
            PreKeyAction::ToggleDevtools
        );
        assert_eq!(
            pre_key_action(true, VK_F12, true, true, true),
            PreKeyAction::ToggleDevtools,
            "F12 toggles even with modifiers"
        );
        assert_eq!(
            pre_key_action(true, VK_I, true, true, false),
            PreKeyAction::ToggleDevtools
        );
        assert_eq!(
            pre_key_action(true, VK_F5, false, false, false),
            PreKeyAction::Reload {
                ignore_cache: false
            }
        );
        assert_eq!(
            pre_key_action(true, VK_F5, true, true, false),
            PreKeyAction::Reload {
                ignore_cache: false
            },
            "ctrl+shift+F5 is still a normal reload"
        );
        assert_eq!(
            pre_key_action(true, VK_R, true, false, false),
            PreKeyAction::Reload {
                ignore_cache: false
            }
        );
        assert_eq!(
            pre_key_action(true, VK_R, true, true, false),
            PreKeyAction::Reload { ignore_cache: true }
        );
        assert_eq!(
            pre_key_action(true, VK_LEFT, false, false, true),
            PreKeyAction::Back
        );
        assert_eq!(
            pre_key_action(true, VK_RIGHT, false, false, true),
            PreKeyAction::Forward
        );
        assert_eq!(
            pre_key_action(true, VK_LEFT, true, false, true),
            PreKeyAction::Ignore,
            "ctrl+alt+left is not back"
        );
        assert_eq!(
            pre_key_action(true, VK_ESCAPE, false, false, false),
            PreKeyAction::Stop
        );
        assert_eq!(
            pre_key_action(true, VK_ESCAPE, true, true, true),
            PreKeyAction::Stop,
            "Escape always stops"
        );
        assert!(consumes_key(pre_key_action(
            true, VK_F5, false, false, false
        )));
        assert!(!consumes_key(pre_key_action(
            true, VK_L, false, false, false
        )));
    }

    #[test]
    fn render_crash_status_uses_contract_labels() {
        assert_eq!(render_crash_status(RenderTerm::Crashed, None), "crashed");
        assert_eq!(render_crash_status(RenderTerm::Killed, Some("x")), "killed");
        assert_eq!(render_crash_status(RenderTerm::Abnormal, None), "abnormal");
        assert_eq!(render_crash_status(RenderTerm::Oom, None), "oom");
        assert_eq!(
            render_crash_status(RenderTerm::LaunchFailed, None),
            "launch-failed"
        );
        assert_eq!(
            render_crash_status(RenderTerm::Other(42), Some("gpu-reset")),
            "gpu-reset"
        );
        assert_eq!(render_crash_status(RenderTerm::Other(7), None), "7");
        assert_eq!(
            render_crash_status(RenderTerm::Other(7), Some("")),
            "",
            "empty error_string is still used when present"
        );
    }

    #[test]
    fn chrome_events_skip_health_and_non_main() {
        assert!(is_main_browser(0, 99));
        assert!(is_main_browser(4, 4));
        assert!(!is_main_browser(4, 5));
        assert!(!emit_chrome_ui_event(true, true));
        assert!(!emit_chrome_ui_event(false, false));
        assert!(emit_chrome_ui_event(false, true));
        assert!(!emit_load_error(true, true, true, -105));
        assert!(!emit_load_error(false, false, true, -105));
        assert!(!emit_load_error(false, true, false, -105));
        assert!(!emit_load_error(false, true, true, ERR_ABORTED));
        assert_eq!(ERR_ABORTED, -3);
        assert!(emit_load_error(false, true, true, -105));
    }

    #[test]
    fn focus_handler_emits_only_for_main_browser() {
        assert_eq!(
            focus_handoff_event(true, true, FocusHandoff::Browser),
            None,
            "health-check must not emit chrome focus"
        );
        assert_eq!(
            focus_handoff_event(false, false, FocusHandoff::Browser),
            None,
            "DevTools / popup must not emit focus"
        );
        assert_eq!(
            focus_handoff_event(false, false, FocusHandoff::App { next: true }),
            None
        );
        assert_eq!(
            focus_handoff_event(false, true, FocusHandoff::Browser),
            Some(HostEvent::Focus {
                owner: FocusOwner::Browser,
                next: None,
            })
        );
    }

    #[test]
    fn shortcut_ctrl_l_emits_only_for_main_browser() {
        assert_eq!(
            shortcut_event(false, true, "ctrl+l"),
            Some(HostEvent::Shortcut {
                chord: "ctrl+l".into()
            })
        );
        assert_eq!(
            shortcut_event(true, true, "ctrl+l"),
            None,
            "health-check must not emit chrome shortcuts"
        );
        assert_eq!(
            shortcut_event(false, false, "ctrl+l"),
            None,
            "DevTools / popup must not emit ctrl+l"
        );
        assert_eq!(
            shortcut_event(false, true, "ctrl+b"),
            Some(HostEvent::Shortcut {
                chord: "ctrl+b".into()
            })
        );
    }

    #[test]
    fn take_focus_console_emits_owner_app_for_main_browser() {
        assert_eq!(
            take_focus_from_beacon(false, true, "idioteque:take-focus:1"),
            Some(HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(true),
            })
        );
        assert_eq!(
            take_focus_from_beacon(false, true, "idioteque:take-focus:0"),
            Some(HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(false),
            })
        );
        assert_ne!(
            take_focus_from_beacon(false, true, "idioteque:take-focus:1"),
            take_focus_from_beacon(false, true, "idioteque:take-focus:0")
        );
        assert_eq!(
            take_focus_from_beacon(true, true, "idioteque:take-focus:1"),
            None
        );
        assert_eq!(
            take_focus_from_beacon(false, false, "idioteque:take-focus:0"),
            None
        );
    }

    #[test]
    fn take_focus_beacon_rejects_page_noise() {
        assert_eq!(
            parse_take_focus_beacon("idioteque:take-focus:1"),
            Some(true)
        );
        assert_eq!(
            parse_take_focus_beacon("  idioteque:take-focus:0\n"),
            Some(false)
        );
        assert_eq!(parse_take_focus_beacon("idioteque:take-focus:2"), None);
        assert_eq!(parse_take_focus_beacon("idioteque:take-focus:"), None);
        assert_eq!(parse_take_focus_beacon("take-focus:1"), None);
        assert_eq!(parse_take_focus_beacon("console.info"), None);
        assert_eq!(parse_take_focus_beacon(""), None);
        assert_eq!(
            parse_take_focus_beacon("idioteque://chrome/take-focus?next=1"),
            Some(true)
        );
        assert_eq!(
            parse_take_focus_beacon(&format!("{TAKE_FOCUS_URL}0")),
            Some(false)
        );
        assert_eq!(
            take_focus_from_beacon(false, true, "idioteque://chrome/take-focus?next=1"),
            Some(HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(true),
            })
        );
        assert!(TAKE_FOCUS_SCRIPT.contains(TAKE_FOCUS_BEACON));
        assert!(TAKE_FOCUS_SCRIPT.contains(TAKE_FOCUS_URL));
        assert!(TAKE_FOCUS_SCRIPT.contains("window.addEventListener"));
        assert!(
            !TAKE_FOCUS_SCRIPT.contains("document.addEventListener"),
            "capture must be on window so it can run before page document listeners"
        );
        assert!(TAKE_FOCUS_SCRIPT.contains("__idiotequeHandleTab"));
        assert!(TAKE_FOCUS_SCRIPT.contains("location.assign"));
        assert!(TAKE_FOCUS_SCRIPT.contains("keydown"));
        assert!(TAKE_FOCUS_SCRIPT.contains("Tab"));
        assert!(TAKE_FOCUS_SCRIPT.contains("preventDefault"));
        assert!(TAKE_FOCUS_SCRIPT.contains("__idiotequeTakeFocus"));
        assert!(
            TAKE_FOCUS_SCRIPT.contains("}, true)"),
            "Tab trap must register with capture:true"
        );
        assert!(TAKE_FOCUS_SCRIPT.contains("a.blur"));
        assert!(BLUR_PAGE_SCRIPT.contains("activeElement"));
        assert!(should_swallow_page_key(
            true,
            pre_key_action(true, VK_TAB, false, false, false)
        ));
        assert!(should_swallow_page_key(
            true,
            pre_key_action(true, VK_L, false, false, false)
        ));
        assert!(!should_swallow_page_key(
            true,
            pre_key_action(true, VK_L, true, false, false)
        ));
        assert!(!should_swallow_page_key(
            false,
            pre_key_action(true, VK_TAB, false, false, false)
        ));
        let debounce = Mutex::new(None);
        assert!(take_focus_should_emit(&debounce, true));
        assert!(
            !take_focus_should_emit(&debounce, true),
            "console + scheme must not emit focus twice"
        );
        assert!(take_focus_should_emit(&debounce, false));
        assert_eq!(
            keys_event(false, true, "A".into()),
            Some(HostEvent::Keys { text: "A".into() })
        );
        assert_eq!(keys_event(true, true, "A".into()), None);
        assert_eq!(keys_event(false, false, "A".into()), None);
        assert_eq!(keys_event(false, true, String::new()), None);
        assert!(should_drop_modified_char(KeyEventType::CHAR, true, false));
        assert!(should_drop_modified_char(KeyEventType::CHAR, false, true));
        assert!(!should_drop_modified_char(KeyEventType::CHAR, false, false));
        assert!(!should_drop_modified_char(
            KeyEventType::RAWKEYDOWN,
            true,
            false
        ));
        assert!(tab_dispatch_script(true).contains("__idiotequeHandleTab(true)"));
        assert!(tab_dispatch_script(false).contains("__idiotequeHandleTab(false)"));
        assert!(is_host_keydown(KeyEventType::RAWKEYDOWN));
        assert!(is_host_keydown(KeyEventType::KEYDOWN));
        assert!(!is_host_keydown(KeyEventType::CHAR));
        assert_eq!(host_key_code(VK_L_LOWER), VK_L);
        assert_eq!(
            pre_key_action(true, VK_TAB, false, false, false),
            PreKeyAction::Tab { next: true }
        );
        assert_eq!(
            pre_key_action(true, VK_TAB, false, true, false),
            PreKeyAction::Tab { next: false }
        );
        assert_eq!(
            pre_key_action(true, VK_L_LOWER, true, false, false),
            PreKeyAction::Shortcut("ctrl+l")
        );
        assert!(consumes_key(pre_key_action(
            true, VK_TAB, false, false, false
        )));
        assert!(should_inject_take_focus_trap(false, true, true));
        assert!(
            !should_inject_take_focus_trap(true, true, true),
            "health-check must not inject the Tab trap"
        );
        assert!(!should_inject_take_focus_trap(false, false, true));
        assert!(!should_inject_take_focus_trap(false, true, false));
    }

    #[test]
    fn vk_typed_char_covers_letters_digits_and_space() {
        assert_eq!(vk_typed_char(VK_L, false), Some('l'));
        assert_eq!(vk_typed_char(VK_L, true), Some('L'));
        assert_eq!(vk_typed_char(VK_L_LOWER, false), Some('l'));
        assert_eq!(vk_typed_char(0x54, true), Some('T'));
        assert_eq!(vk_typed_char(0x31, false), Some('1'));
        assert_eq!(vk_typed_char(0x31, true), None);
        assert_eq!(vk_typed_char(0x20, false), Some(' '));
        assert_eq!(vk_typed_char(0x10, false), None, "Shift is not text");
        assert_eq!(printable_char(b'A' as u32), Some('A'));
        assert_eq!(printable_char(0), None);
        assert_eq!(printable_char(9), None);
    }

    #[test]
    fn keys_event_emits_only_for_main_browser() {
        assert_eq!(
            keys_event(false, true, "A".into()),
            Some(HostEvent::Keys { text: "A".into() })
        );
        assert_eq!(
            keys_event(true, true, "A".into()),
            None,
            "health-check must not emit keys"
        );
        assert_eq!(
            keys_event(false, false, "A".into()),
            None,
            "DevTools / popup must not emit keys"
        );
        assert_eq!(keys_event(false, true, String::new()), None);
    }

    #[test]
    fn on_take_focus_next_true_and_false() {
        assert_eq!(
            focus_handoff_event(false, true, FocusHandoff::App { next: true }),
            Some(HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(true),
            })
        );
        assert_eq!(
            focus_handoff_event(false, true, FocusHandoff::App { next: false }),
            Some(HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(false),
            })
        );
        assert_ne!(
            focus_handoff_event(false, true, FocusHandoff::App { next: true }),
            focus_handoff_event(false, true, FocusHandoff::App { next: false })
        );
    }

    #[test]
    fn got_focus_stops_swallowing_page_keys() {
        assert!(
            !app_owns_keyboard_after_handoff(FocusHandoff::Browser),
            "OnGotFocus must release chrome's swallow"
        );
        assert!(!should_swallow_page_key(
            app_owns_keyboard_after_handoff(FocusHandoff::Browser),
            PreKeyAction::Ignore
        ));
        assert!(!should_swallow_page_key(
            app_owns_keyboard_after_handoff(FocusHandoff::Browser),
            PreKeyAction::Tab { next: true }
        ));
        assert!(app_owns_keyboard_after_handoff(FocusHandoff::App {
            next: true
        }));
        assert!(should_swallow_page_key(
            app_owns_keyboard_after_handoff(FocusHandoff::App { next: false }),
            PreKeyAction::Ignore
        ));
        assert_eq!(drop_browser_focus_plan().set_focus, 0);
        assert_eq!(give_browser_focus_plan().set_focus, 1);
        assert!(
            !app_owns_keyboard_after_navigate(),
            "URL-owned navigation must release chrome's swallow"
        );
        assert!(!should_swallow_page_key(
            app_owns_keyboard_after_navigate(),
            PreKeyAction::Ignore
        ));
    }

    #[test]
    fn unfocus_never_calls_xsetinputfocus() {
        let drop = drop_browser_focus_plan();
        assert!(
            !drop.x11,
            "Unfocus must not call XSetInputFocus; ADE already owns the toplevel"
        );
        assert_eq!(drop.set_focus, 0);
        let give = give_browser_focus_plan();
        assert!(give.x11, "Focus still moves X11 onto the CEF child");
        assert_eq!(give.set_focus, 1);
        assert_ne!(drop, give);
        assert!(
            should_ungrab_on_unfocus(),
            "ADE XUngrab cannot release Ozone's grab on the host display"
        );
        assert!(should_swallow_page_key(true, PreKeyAction::Ignore));
        assert!(should_swallow_page_key(
            true,
            PreKeyAction::Tab { next: true }
        ));
        assert!(!should_swallow_page_key(
            true,
            PreKeyAction::Shortcut("ctrl+l")
        ));
    }
}
