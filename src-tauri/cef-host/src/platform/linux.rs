//! X11 helpers for the Linux host: map/unmap/resize/raise/focus the CEF child.

use std::env::VarError;
use std::ffi::CString;
use std::sync::OnceLock;

use x11_dl::xlib::{self, CurrentTime, Display as XDisplay, RevertToParent, Xlib};

use crate::exit::{self, fatal};

static XLIB: OnceLock<Xlib> = OnceLock::new();

fn xlib() -> &'static Xlib {
    XLIB.get_or_init(|| {
        let lib = Xlib::open().unwrap_or_else(|e| fatal(exit::NO_X11, format!("libX11: {e}")));
        // Must be the first Xlib call in the process, before CEF or us open a display.
        unsafe {
            if (lib.XInitThreads)() == 0 {
                fatal(exit::NO_X11, "XInitThreads failed");
            }
        }
        lib
    })
}

/// Load libX11 and call `XInitThreads` before any other Xlib use (including CEF).
pub fn init_threads() {
    let _ = xlib();
}

/// Why `ensure_display` refuses to talk to X before `XOpenDisplay`.
///
/// Contract §4.7: empty / unusable `DISPLAY` is exit 16. `WAYLAND_DISPLAY`
/// does not count — this host does not embed on native Wayland (CEF #2804).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayEnvError {
    Empty,
    InteriorNul,
}

impl DisplayEnvError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Empty => "DISPLAY is empty",
            Self::InteriorNul => "invalid DISPLAY",
        }
    }

    pub(crate) fn exit_code(self) -> i32 {
        exit::NO_X11
    }
}

pub(crate) fn display_env(display: &str) -> Result<(), DisplayEnvError> {
    if display.is_empty() {
        Err(DisplayEnvError::Empty)
    } else if display.as_bytes().contains(&0) {
        Err(DisplayEnvError::InteriorNul)
    } else {
        Ok(())
    }
}

/// `std::env::var("DISPLAY")` treats unset and non-UTF-8 the same as empty.
pub(crate) fn display_from_var(var: Result<String, VarError>) -> Result<String, DisplayEnvError> {
    match var {
        Ok(value) => {
            display_env(&value)?;
            Ok(value)
        }
        Err(VarError::NotPresent) | Err(VarError::NotUnicode(_)) => Err(DisplayEnvError::Empty),
    }
}

pub(crate) fn xopen_display_failed_message() -> &'static str {
    "XOpenDisplay failed"
}

/// Native Wayland parent windows are out of scope (CEF #2804). Embed is X11
/// or XWayland (`DISPLAY` + `--ozone-platform=x11`).
#[cfg(test)]
pub(crate) fn embed_protocol() -> &'static str {
    "x11"
}

/// Fail with exit 16 if DISPLAY is unset or XOpenDisplay fails.
pub fn ensure_display() {
    let display = match display_from_var(std::env::var("DISPLAY")) {
        Ok(value) => value,
        Err(err) => fatal(err.exit_code(), err.message()),
    };
    init_threads();
    let xlib = xlib();
    let c_display = CString::new(display)
        .unwrap_or_else(|_| fatal(exit::NO_X11, DisplayEnvError::InteriorNul.message()));
    unsafe {
        let dpy = (xlib.XOpenDisplay)(c_display.as_ptr());
        if dpy.is_null() {
            fatal(exit::NO_X11, xopen_display_failed_message());
        }
        (xlib.XCloseDisplay)(dpy);
    }
}

fn dpy() -> *mut XDisplay {
    let from_cef = cef::get_xdisplay() as *mut XDisplay;
    if !from_cef.is_null() {
        return from_cef;
    }
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    let c_display = match CString::new(display) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    unsafe { (xlib().XOpenDisplay)(c_display.as_ptr()) }
}

fn with_display(xid: u64, f: impl FnOnce(&Xlib, *mut XDisplay, xlib::Window)) {
    if !xid_usable(xid) {
        return;
    }
    let dpy = dpy();
    if dpy.is_null() {
        return;
    }
    f(xlib(), dpy, xid as xlib::Window);
    unsafe {
        (xlib().XFlush)(dpy);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowOp {
    Map,
    Unmap,
    Raise,
    Focus { revert: i32, time: xlib::Time },
    MoveResize { x: i32, y: i32, w: u32, h: u32 },
}

pub(crate) fn xid_usable(xid: u64) -> bool {
    xid != 0
}

/// XCreateWindow / XMoveResizeWindow reject a 0-size drawable (CEF #3396).
pub(crate) fn clamp_extent(n: i32) -> u32 {
    n.max(1) as u32
}

/// CONTRACT §4.4: `show` is `XMapWindow` then `XRaiseWindow`.
pub(crate) fn plan_show(xid: u64) -> Option<[WindowOp; 2]> {
    xid_usable(xid).then_some([WindowOp::Map, WindowOp::Raise])
}

pub(crate) fn plan_hide(xid: u64) -> Option<WindowOp> {
    xid_usable(xid).then_some(WindowOp::Unmap)
}

pub(crate) fn plan_focus(xid: u64) -> Option<WindowOp> {
    xid_usable(xid).then_some(WindowOp::Focus {
        revert: RevertToParent,
        time: CurrentTime,
    })
}

pub(crate) fn plan_move_resize(xid: u64, x: i32, y: i32, w: i32, h: i32) -> Option<WindowOp> {
    xid_usable(xid).then_some(WindowOp::MoveResize {
        x,
        y,
        w: clamp_extent(w),
        h: clamp_extent(h),
    })
}

pub(crate) fn reparent_applies(child: u64, parent: u64) -> bool {
    xid_usable(child) && xid_usable(parent)
}

/// CEF hangs off the shim at (0, 0), never directly off the GDK hole
/// (CONTRACT §4.1).
pub(crate) const SHIM_CHILD_X: i32 = 0;
pub(crate) const SHIM_CHILD_Y: i32 = 0;

pub fn map_window(xid: u64) {
    let Some(ops) = plan_show(xid) else {
        return;
    };
    debug_assert_eq!(ops[0], WindowOp::Map);
    with_display(xid, |x, dpy, w| unsafe {
        (x.XMapWindow)(dpy, w);
    });
    if ops.contains(&WindowOp::Raise) {
        raise_window(xid);
    }
}

pub fn unmap_window(xid: u64) {
    if plan_hide(xid).is_none() {
        return;
    }
    with_display(xid, |x, dpy, w| unsafe {
        (x.XUnmapWindow)(dpy, w);
    });
}

pub fn raise_window(xid: u64) {
    if !xid_usable(xid) {
        return;
    }
    with_display(xid, |x, dpy, w| unsafe {
        (x.XRaiseWindow)(dpy, w);
    });
}

pub fn move_resize(xid: u64, x: i32, y: i32, w: i32, h: i32) {
    let Some(WindowOp::MoveResize { x, y, w, h }) = plan_move_resize(xid, x, y, w, h) else {
        return;
    };
    with_display(xid, |xl, dpy, win| unsafe {
        (xl.XMoveResizeWindow)(dpy, win, x, y, w, h);
    });
}

pub fn focus_window(xid: u64) {
    let Some(WindowOp::Focus { revert, time }) = plan_focus(xid) else {
        return;
    };
    with_display(xid, |x, dpy, w| unsafe {
        (x.XSetInputFocus)(dpy, w, revert, time);
    });
}

pub fn reparent(child: u64, parent: u64, x: i32, y: i32) {
    if !reparent_applies(child, parent) {
        return;
    }
    let dpy = dpy();
    if dpy.is_null() {
        return;
    }
    unsafe {
        (xlib().XReparentWindow)(dpy, child as xlib::Window, parent as xlib::Window, x, y);
        (xlib().XFlush)(dpy);
    }
}

pub fn xid_from_handle(handle: cef::sys::cef_window_handle_t) -> u64 {
    handle as u64
}

/// Visual + colormap the shim must declare so it can hang off a hole whose
/// visual is not the default (GTK GL, or any non-default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShimVisualSource {
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShimColormapSource {
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShimWindowSpec {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub border_width: u32,
    pub class: u32,
    pub value_mask: u64,
    pub border_pixel: u64,
    pub visual: ShimVisualSource,
    pub colormap: ShimColormapSource,
    pub map_after_create: bool,
}

/// Chromium `CreateWindow` uses the default visual and a `CopyFromParent`
/// colormap (CEF #3294). A parent with another visual makes that request
/// `BadMatch`. The shim is the window that *can* be a child of the hole
/// because it sets colormap + border pixel explicitly, and CEF then hangs
/// off the shim with matching visuals.
///
/// hole-gdk may request the system visual on the hole. That is a second
/// belt, not a license to drop this one: a GL hole or a colormap mismatch
/// still BadMatches (CEF #2804). Policy: keep the shim.
#[cfg(test)]
pub(crate) fn copy_from_parent_colormap_badmatch(parent_visual_is_default: bool) -> bool {
    !parent_visual_is_default
}

pub(crate) fn shim_required(parent: u64, parent_visual_is_default: bool) -> bool {
    let _ = parent_visual_is_default;
    xid_usable(parent)
}

pub(crate) fn plan_shim(parent: u64, w: i32, h: i32) -> Option<ShimWindowSpec> {
    // Visual is unused: the shim is required whenever the parent xid is usable.
    if !shim_required(parent, true) {
        return None;
    }
    Some(ShimWindowSpec {
        x: SHIM_CHILD_X,
        y: SHIM_CHILD_Y,
        width: clamp_extent(w),
        height: clamp_extent(h),
        border_width: 0,
        class: xlib::InputOutput as u32,
        value_mask: xlib::CWColormap | xlib::CWBorderPixel | xlib::CWBackPixel,
        border_pixel: 0,
        visual: ShimVisualSource::Default,
        colormap: ShimColormapSource::Default,
        map_after_create: true,
    })
}

unsafe fn create_shim_window(
    x: &Xlib,
    dpy: *mut XDisplay,
    parent: xlib::Window,
    spec: ShimWindowSpec,
) -> xlib::Window {
    let screen = (x.XDefaultScreen)(dpy);
    let visual = match spec.visual {
        ShimVisualSource::Default => (x.XDefaultVisual)(dpy, screen),
    };
    let depth = (x.XDefaultDepth)(dpy, screen);
    let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
    attrs.colormap = match spec.colormap {
        ShimColormapSource::Default => (x.XDefaultColormap)(dpy, screen),
    };
    attrs.border_pixel = spec.border_pixel;
    attrs.background_pixel = (x.XBlackPixel)(dpy, screen);
    let window = (x.XCreateWindow)(
        dpy,
        parent,
        spec.x,
        spec.y,
        spec.width,
        spec.height,
        spec.border_width,
        depth,
        spec.class,
        visual,
        spec.value_mask,
        &mut attrs,
    );
    if window == 0 {
        return 0;
    }
    if spec.map_after_create {
        (x.XMapWindow)(dpy, window);
    }
    (x.XFlush)(dpy);
    window
}

/// Ventana intermedia con el visual por defecto del servidor.
///
/// El hueco que crea GTK lleva el visual GL que GDK elige para su toplevel;
/// Chromium crea su ventana con el visual por defecto y colormap
/// `CopyFromParent`, y con visuales distintos `CreateWindow` falla con
/// `BadMatch`. Este hijo declara colormap y border pixel explícitos, así que
/// puede colgar del hueco, y CEF cuelga de él sin conflicto.
pub fn create_default_visual_child(parent: u64, w: i32, h: i32) -> u64 {
    let Some(spec) = plan_shim(parent, w, h) else {
        return 0;
    };
    let dpy = dpy();
    if dpy.is_null() {
        return 0;
    }
    unsafe { create_shim_window(xlib(), dpy, parent as xlib::Window, spec) as u64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    static LIVE_X: Mutex<()> = Mutex::new(());
    static LAST_X_ERROR: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn empty_display_is_exit_16() {
        let err = display_env("").unwrap_err();
        assert_eq!(err, DisplayEnvError::Empty);
        assert_eq!(err.exit_code(), exit::NO_X11);
        assert_eq!(err.exit_code(), 16);
        assert_eq!(err.message(), "DISPLAY is empty");
        assert_eq!(
            display_from_var(Ok(String::new())),
            Err(DisplayEnvError::Empty)
        );
    }

    #[test]
    fn unset_and_non_unicode_display_are_exit_16() {
        assert_eq!(
            display_from_var(Err(VarError::NotPresent)),
            Err(DisplayEnvError::Empty)
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let raw = VarError::NotUnicode(OsString::from_vec(vec![0xff, 0xfe]));
            assert_eq!(display_from_var(Err(raw)), Err(DisplayEnvError::Empty));
        }
        assert_eq!(DisplayEnvError::Empty.exit_code(), 16);
    }

    #[test]
    fn wayland_display_alone_is_still_exit_16() {
        // A Wayland session without XWayland has WAYLAND_DISPLAY and no DISPLAY.
        // We do not read WAYLAND_DISPLAY and we do not port the embed.
        let wayland = Some("wayland-0");
        let display: Option<&str> = None;
        let _ = wayland;
        assert!(display_from_var(Err(VarError::NotPresent)).is_err());
        assert_eq!(
            display_env(display.unwrap_or("")),
            Err(DisplayEnvError::Empty)
        );
        assert_eq!(embed_protocol(), "x11");
        assert_ne!(embed_protocol(), "wayland");
    }

    #[test]
    fn interior_nul_display_is_exit_16() {
        let raw = ":\0.0";
        let err = display_env(raw).unwrap_err();
        assert_eq!(err, DisplayEnvError::InteriorNul);
        assert_eq!(err.exit_code(), 16);
        assert_eq!(err.message(), "invalid DISPLAY");
        assert!(std::ffi::CString::new(raw).is_err());
    }

    #[test]
    fn whitespace_display_is_not_classified_empty() {
        // Current host does not trim. " " goes to XOpenDisplay and fails there
        // with exit 16 (`XOpenDisplay failed`), not "DISPLAY is empty".
        assert_eq!(display_env(" "), Ok(()));
        assert_eq!(display_env("\t"), Ok(()));
        assert_eq!(display_env("\n"), Ok(()));
    }

    #[test]
    fn typical_display_names_pass_env_check() {
        for name in [":0", ":1", ":0.0", "localhost:0.0", "/tmp/.X11-unix/X1"] {
            assert_eq!(display_env(name), Ok(()), "{name}");
            assert_eq!(display_from_var(Ok(name.to_string())), Ok(name.to_string()));
        }
    }

    #[test]
    fn xopen_failure_is_exit_16() {
        assert_eq!(xopen_display_failed_message(), "XOpenDisplay failed");
        assert_eq!(exit::NO_X11, 16);
    }

    #[test]
    fn every_display_env_error_is_code_16() {
        for err in [DisplayEnvError::Empty, DisplayEnvError::InteriorNul] {
            assert_eq!(err.exit_code(), 16);
        }
    }

    #[test]
    fn xid_zero_skips_map_unmap_resize_focus() {
        assert!(!xid_usable(0));
        assert_eq!(plan_show(0), None);
        assert_eq!(plan_hide(0), None);
        assert_eq!(plan_focus(0), None);
        assert_eq!(plan_move_resize(0, 10, 20, 30, 40), None);
        assert!(!reparent_applies(0, 99));
        assert!(!reparent_applies(99, 0));
        assert!(!reparent_applies(0, 0));
    }

    #[test]
    fn show_maps_then_raises() {
        assert_eq!(plan_show(0x100), Some([WindowOp::Map, WindowOp::Raise]));
        assert_ne!(plan_show(0x100), Some([WindowOp::Raise, WindowOp::Map]));
        assert_ne!(plan_show(0x100), Some([WindowOp::Map, WindowOp::Map]));
    }

    #[test]
    fn hide_is_unmap_without_raise() {
        assert_eq!(plan_hide(7), Some(WindowOp::Unmap));
        assert_ne!(plan_hide(7), Some(WindowOp::Raise));
        assert_ne!(plan_hide(7), Some(WindowOp::Map));
    }

    #[test]
    fn focus_uses_revert_to_parent_and_current_time() {
        assert_eq!(
            plan_focus(3),
            Some(WindowOp::Focus {
                revert: RevertToParent,
                time: CurrentTime,
            })
        );
        assert_eq!(RevertToParent, 2);
        assert_eq!(CurrentTime, 0);
        // RevertToNone / a stale timestamp would be a different plan.
        assert_ne!(
            plan_focus(3),
            Some(WindowOp::Focus {
                revert: 0,
                time: CurrentTime,
            })
        );
    }

    #[test]
    fn resize_clamps_non_positive_extent() {
        assert_eq!(
            plan_move_resize(1, 8, 9, 0, 0),
            Some(WindowOp::MoveResize {
                x: 8,
                y: 9,
                w: 1,
                h: 1
            })
        );
        assert_eq!(
            plan_move_resize(1, 0, 0, -4, -9),
            Some(WindowOp::MoveResize {
                x: 0,
                y: 0,
                w: 1,
                h: 1
            })
        );
        assert_eq!(clamp_extent(0), 1);
        assert_eq!(clamp_extent(-1), 1);
        assert_eq!(clamp_extent(i32::MIN), 1);
        assert_eq!(clamp_extent(1), 1);
        assert_eq!(clamp_extent(1920), 1920);
        assert_eq!(clamp_extent(i32::MAX), i32::MAX as u32);
    }

    #[test]
    fn resize_keeps_negative_origin_for_offscreen_park() {
        let op = plan_move_resize(1, -10_000, -8_000, 400, 300).unwrap();
        assert_eq!(
            op,
            WindowOp::MoveResize {
                x: -10_000,
                y: -8_000,
                w: 400,
                h: 300
            }
        );
    }

    #[test]
    fn resize_mixed_zero_width_keeps_height() {
        assert_eq!(
            plan_move_resize(2, 1, 2, 0, 600),
            Some(WindowOp::MoveResize {
                x: 1,
                y: 2,
                w: 1,
                h: 600
            })
        );
    }

    #[test]
    fn reparent_to_shim_is_origin() {
        assert!(reparent_applies(10, 11));
        assert_eq!((SHIM_CHILD_X, SHIM_CHILD_Y), (0, 0));
    }

    #[test]
    fn shim_skipped_only_when_parent_xid_is_zero() {
        assert_eq!(plan_shim(0, 800, 600), None);
        assert!(!shim_required(0, false));
        assert!(!shim_required(0, true));
        assert!(plan_shim(0x50, 0, 0).is_some());
    }

    #[test]
    fn shim_kept_even_if_parent_visual_already_default() {
        assert!(shim_required(0x50, true));
        assert!(shim_required(0x50, false));
        assert!(copy_from_parent_colormap_badmatch(false));
        assert!(!copy_from_parent_colormap_badmatch(true));
    }

    #[test]
    fn shim_attrs_force_colormap_and_border_pixel() {
        let spec = plan_shim(1, 320, 200).unwrap();
        assert_eq!(spec.x, 0);
        assert_eq!(spec.y, 0);
        assert_eq!(spec.width, 320);
        assert_eq!(spec.height, 200);
        assert_eq!(spec.border_width, 0);
        assert_eq!(spec.border_pixel, 0);
        assert_eq!(spec.class, xlib::InputOutput as u32);
        assert_eq!(spec.visual, ShimVisualSource::Default);
        assert_eq!(spec.colormap, ShimColormapSource::Default);
        assert!(spec.map_after_create);
        assert_ne!(spec.value_mask & xlib::CWColormap, 0);
        assert_ne!(spec.value_mask & xlib::CWBorderPixel, 0);
        assert_ne!(spec.value_mask & xlib::CWBackPixel, 0);
        // CopyFromParent (mask 0) is the BadMatch path on a foreign visual.
        assert_ne!(spec.value_mask, 0);
        assert_ne!(spec.value_mask, xlib::CWBackPixel);
    }

    #[test]
    fn shim_clamps_zero_and_negative_size() {
        let spec = plan_shim(1, 0, -3).unwrap();
        assert_eq!((spec.width, spec.height), (1, 1));
    }

    #[test]
    fn xid_from_handle_is_identity_cast() {
        assert_eq!(xid_from_handle(0 as cef::sys::cef_window_handle_t), 0);
        assert_eq!(
            xid_from_handle(0x00ab_cdef as cef::sys::cef_window_handle_t),
            0x00ab_cdef
        );
    }

    #[test]
    fn embed_is_x11_not_native_wayland() {
        assert_eq!(embed_protocol(), "x11");
        assert_ne!(embed_protocol(), "wayland");
        assert_ne!(embed_protocol(), "headless");
    }

    unsafe extern "C" fn swallow_x_error(
        _dpy: *mut XDisplay,
        ev: *mut xlib::XErrorEvent,
    ) -> libc::c_int {
        if !ev.is_null() {
            LAST_X_ERROR.store(unsafe { (*ev).error_code as u32 }, Ordering::SeqCst);
        }
        0
    }

    struct LiveX {
        x: &'static Xlib,
        dpy: *mut XDisplay,
        windows: Vec<xlib::Window>,
        colormaps: Vec<xlib::Colormap>,
    }

    impl LiveX {
        fn connect() -> Option<Self> {
            let name = std::env::var("DISPLAY").ok()?;
            display_env(&name).ok()?;
            init_threads();
            let x = xlib();
            let c_name = CString::new(name).ok()?;
            let dpy = unsafe { (x.XOpenDisplay)(c_name.as_ptr()) };
            if dpy.is_null() {
                return None;
            }
            unsafe {
                (x.XSetErrorHandler)(Some(swallow_x_error));
                (x.XSynchronize)(dpy, 1);
            }
            Some(Self {
                x,
                dpy,
                windows: Vec::new(),
                colormaps: Vec::new(),
            })
        }

        fn reset_error(&self) {
            LAST_X_ERROR.store(0, Ordering::SeqCst);
            unsafe {
                (self.x.XSync)(self.dpy, 0);
            }
            LAST_X_ERROR.store(0, Ordering::SeqCst);
        }

        fn error(&self) -> u32 {
            unsafe {
                (self.x.XSync)(self.dpy, 0);
            }
            LAST_X_ERROR.load(Ordering::SeqCst)
        }

        fn root(&self) -> xlib::Window {
            unsafe { (self.x.XDefaultRootWindow)(self.dpy) }
        }

        fn create_override_parent(&mut self, w: i32, h: i32) -> u64 {
            unsafe {
                let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
                attrs.override_redirect = 1;
                attrs.border_pixel = 0;
                attrs.background_pixel =
                    (self.x.XBlackPixel)(self.dpy, (self.x.XDefaultScreen)(self.dpy));
                let (width, height) = (clamp_extent(w), clamp_extent(h));
                let window = (self.x.XCreateWindow)(
                    self.dpy,
                    self.root(),
                    0,
                    0,
                    width,
                    height,
                    0,
                    0,
                    xlib::CopyFromParent as u32,
                    std::ptr::null_mut(),
                    xlib::CWOverrideRedirect | xlib::CWBorderPixel | xlib::CWBackPixel,
                    &mut attrs,
                );
                self.windows.push(window);
                (self.x.XMapWindow)(self.dpy, window);
                (self.x.XFlush)(self.dpy);
                window as u64
            }
        }

        fn create_shim(&mut self, parent: u64, w: i32, h: i32) -> u64 {
            let spec = plan_shim(parent, w, h).expect("parent xid");
            let window =
                unsafe { create_shim_window(self.x, self.dpy, parent as xlib::Window, spec) };
            if window != 0 {
                self.windows.push(window);
            }
            window as u64
        }

        fn geometry(&self, xid: u64) -> (i32, i32, u32, u32) {
            let mut root = 0;
            let mut x = 0;
            let mut y = 0;
            let mut w = 0;
            let mut h = 0;
            let mut border = 0;
            let mut depth = 0;
            unsafe {
                (self.x.XGetGeometry)(
                    self.dpy,
                    xid as xlib::Window,
                    &mut root,
                    &mut x,
                    &mut y,
                    &mut w,
                    &mut h,
                    &mut border,
                    &mut depth,
                );
            }
            (x, y, w, h)
        }

        fn map_state(&self, xid: u64) -> i32 {
            let mut attrs: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
            unsafe {
                (self.x.XGetWindowAttributes)(self.dpy, xid as xlib::Window, &mut attrs);
            }
            attrs.map_state
        }

        fn apply_show(&self, xid: u64) {
            let Some(ops) = plan_show(xid) else {
                return;
            };
            for op in ops {
                self.apply(xid, op);
            }
        }

        fn apply(&self, xid: u64, op: WindowOp) {
            let w = xid as xlib::Window;
            unsafe {
                match op {
                    WindowOp::Map => {
                        (self.x.XMapWindow)(self.dpy, w);
                    }
                    WindowOp::Unmap => {
                        (self.x.XUnmapWindow)(self.dpy, w);
                    }
                    WindowOp::Raise => {
                        (self.x.XRaiseWindow)(self.dpy, w);
                    }
                    WindowOp::Focus { revert, time } => {
                        (self.x.XSetInputFocus)(self.dpy, w, revert, time);
                    }
                    WindowOp::MoveResize {
                        x,
                        y,
                        w: width,
                        h: height,
                    } => {
                        (self.x.XMoveResizeWindow)(self.dpy, w, x, y, width, height);
                    }
                }
                (self.x.XFlush)(self.dpy);
            }
        }

        fn foreign_visual(&self) -> Option<(*mut xlib::Visual, i32)> {
            unsafe {
                let screen = (self.x.XDefaultScreen)(self.dpy);
                let default = (self.x.XDefaultVisual)(self.dpy, screen);
                let mut n = 0;
                let mut tmpl: xlib::XVisualInfo = std::mem::zeroed();
                let list = (self.x.XGetVisualInfo)(self.dpy, xlib::VisualNoMask, &mut tmpl, &mut n);
                if list.is_null() || n <= 0 {
                    return None;
                }
                let infos = std::slice::from_raw_parts(list, n as usize);
                let found = infos.iter().find(|info| {
                    info.screen == screen && !info.visual.is_null() && info.visual != default
                });
                let out = found.map(|info| (info.visual, info.depth));
                (self.x.XFree)(list as *mut _);
                out
            }
        }

        fn create_foreign_visual_parent(&mut self, w: i32, h: i32) -> Option<u64> {
            let (visual, depth) = self.foreign_visual()?;
            unsafe {
                let screen = (self.x.XDefaultScreen)(self.dpy);
                let cmap = (self.x.XCreateColormap)(self.dpy, self.root(), visual, xlib::AllocNone);
                self.colormaps.push(cmap);
                let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
                attrs.colormap = cmap;
                attrs.border_pixel = 0;
                attrs.background_pixel = (self.x.XBlackPixel)(self.dpy, screen);
                attrs.override_redirect = 1;
                let window = (self.x.XCreateWindow)(
                    self.dpy,
                    self.root(),
                    0,
                    0,
                    clamp_extent(w),
                    clamp_extent(h),
                    0,
                    depth,
                    xlib::InputOutput as u32,
                    visual,
                    xlib::CWColormap
                        | xlib::CWBorderPixel
                        | xlib::CWBackPixel
                        | xlib::CWOverrideRedirect,
                    &mut attrs,
                );
                if window == 0 {
                    return None;
                }
                self.windows.push(window);
                (self.x.XMapWindow)(self.dpy, window);
                (self.x.XFlush)(self.dpy);
                Some(window as u64)
            }
        }
    }

    /// Empty `DISPLAY` → skip (no X). Set `DISPLAY` + failed open → fail the test.
    fn require_live() -> Option<LiveX> {
        match std::env::var("DISPLAY") {
            Ok(name) if !name.is_empty() => Some(
                LiveX::connect()
                    .unwrap_or_else(|| panic!("DISPLAY={name} but XOpenDisplay failed")),
            ),
            _ => None,
        }
    }

    impl Drop for LiveX {
        fn drop(&mut self) {
            unsafe {
                for window in self.windows.drain(..).rev() {
                    if window != 0 {
                        (self.x.XDestroyWindow)(self.dpy, window);
                    }
                }
                for cmap in self.colormaps.drain(..) {
                    (self.x.XFreeColormap)(self.dpy, cmap);
                }
                (self.x.XCloseDisplay)(self.dpy);
            }
        }
    }

    #[test]
    fn live_shim_map_unmap_resize_focus() {
        let _guard = LIVE_X.lock().expect("live x11 lock");
        let Some(mut live) = require_live() else {
            return;
        };
        live.reset_error();
        let parent = live.create_override_parent(400, 300);
        assert_ne!(parent, 0);
        let shim = live.create_shim(parent, 400, 300);
        assert_ne!(shim, 0, "default-visual shim must be creatable");
        assert_eq!(
            live.error(),
            0,
            "shim create must not BadMatch on default parent"
        );

        live.apply_show(shim);
        assert_eq!(live.map_state(shim), xlib::IsViewable);
        assert_eq!(live.error(), 0);

        live.apply(shim, plan_focus(shim).unwrap());
        assert_eq!(live.error(), 0, "focus on a mapped shim must not BadMatch");

        live.apply(shim, plan_move_resize(shim, 4, 6, 220, 140).unwrap());
        assert_eq!(live.geometry(shim), (4, 6, 220, 140));

        live.apply(shim, plan_hide(shim).unwrap());
        assert_eq!(live.map_state(shim), xlib::IsUnmapped);
        assert_eq!(live.error(), 0);

        live.apply_show(shim);
        assert_eq!(live.map_state(shim), xlib::IsViewable);
    }

    #[test]
    fn live_zero_size_shim_becomes_1x1() {
        let _guard = LIVE_X.lock().expect("live x11 lock");
        let Some(mut live) = require_live() else {
            return;
        };
        live.reset_error();
        let parent = live.create_override_parent(64, 64);
        let shim = live.create_shim(parent, 0, 0);
        assert_ne!(shim, 0);
        let (_x, _y, w, h) = live.geometry(shim);
        assert_eq!((w, h), (1, 1));
        assert_eq!(live.error(), 0);
    }

    #[test]
    fn live_copy_from_parent_on_foreign_visual_is_badmatch_shim_is_not() {
        let _guard = LIVE_X.lock().expect("live x11 lock");
        let Some(mut live) = require_live() else {
            return;
        };
        let Some(parent) = live.create_foreign_visual_parent(200, 120) else {
            // Single-visual servers cannot demonstrate the mismatch. The
            // unit test `shim_kept_even_if_parent_visual_already_default`
            // still forbids dropping the shim.
            eprintln!("won't-test live BadMatch: server has only the default visual");
            return;
        };

        live.reset_error();
        unsafe {
            let screen = (live.x.XDefaultScreen)(live.dpy);
            let default_visual = (live.x.XDefaultVisual)(live.dpy, screen);
            let default_depth = (live.x.XDefaultDepth)(live.dpy, screen);
            let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
            // Same request Chromium makes: default visual, CopyFromParent colormap.
            let bad = (live.x.XCreateWindow)(
                live.dpy,
                parent as xlib::Window,
                0,
                0,
                80,
                60,
                0,
                default_depth,
                xlib::InputOutput as u32,
                default_visual,
                0,
                &mut attrs,
            );
            let _ = bad;
        }
        assert_eq!(
            live.error(),
            xlib::BadMatch as u32,
            "CopyFromParent onto a non-default parent visual must BadMatch"
        );

        live.reset_error();
        let shim = live.create_shim(parent, 80, 60);
        assert_ne!(shim, 0, "shim with explicit colormap must be creatable");
        assert_eq!(
            live.error(),
            0,
            "removing CWColormap/CWBorderPixel would BadMatch; keep the shim"
        );
    }
}
