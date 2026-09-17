//! X11 helpers for the Linux host: map/unmap/resize/raise/focus the CEF child.

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

/// Fail with exit 16 if DISPLAY is unset or XOpenDisplay fails.
pub fn ensure_display() {
    let display = std::env::var("DISPLAY").unwrap_or_default();
    if display.is_empty() {
        fatal(exit::NO_X11, "DISPLAY is empty");
    }
    init_threads();
    let xlib = xlib();
    let c_display = CString::new(display).unwrap_or_else(|_| fatal(exit::NO_X11, "invalid DISPLAY"));
    unsafe {
        let dpy = (xlib.XOpenDisplay)(c_display.as_ptr());
        if dpy.is_null() {
            fatal(exit::NO_X11, "XOpenDisplay failed");
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
    if xid == 0 {
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

pub fn map_window(xid: u64) {
    with_display(xid, |x, dpy, w| unsafe {
        (x.XMapWindow)(dpy, w);
    });
    raise_window(xid);
}

pub fn unmap_window(xid: u64) {
    with_display(xid, |x, dpy, w| unsafe {
        (x.XUnmapWindow)(dpy, w);
    });
}

pub fn raise_window(xid: u64) {
    with_display(xid, |x, dpy, w| unsafe {
        (x.XRaiseWindow)(dpy, w);
    });
}

pub fn move_resize(xid: u64, x: i32, y: i32, w: i32, h: i32) {
    let w = w.max(1) as u32;
    let h = h.max(1) as u32;
    with_display(xid, |xl, dpy, win| unsafe {
        (xl.XMoveResizeWindow)(dpy, win, x, y, w, h);
    });
}

pub fn focus_window(xid: u64) {
    with_display(xid, |x, dpy, w| unsafe {
        (x.XSetInputFocus)(dpy, w, RevertToParent, CurrentTime);
    });
}

pub fn reparent(child: u64, parent: u64, x: i32, y: i32) {
    if child == 0 || parent == 0 {
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

/// Ventana intermedia con el visual por defecto del servidor.
///
/// El hueco que crea GTK lleva el visual GL que GDK elige para su toplevel;
/// Chromium crea su ventana con el visual por defecto y colormap
/// `CopyFromParent`, y con visuales distintos `CreateWindow` falla con
/// `BadMatch`. Este hijo declara colormap y border pixel explícitos, así que
/// puede colgar del hueco, y CEF cuelga de él sin conflicto.
pub fn create_default_visual_child(parent: u64, w: i32, h: i32) -> u64 {
    if parent == 0 {
        return 0;
    }
    let dpy = dpy();
    if dpy.is_null() {
        return 0;
    }
    let x = xlib();
    unsafe {
        let screen = (x.XDefaultScreen)(dpy);
        let visual = (x.XDefaultVisual)(dpy, screen);
        let depth = (x.XDefaultDepth)(dpy, screen);
        let mut attrs: xlib::XSetWindowAttributes = std::mem::zeroed();
        attrs.colormap = (x.XDefaultColormap)(dpy, screen);
        attrs.border_pixel = 0;
        attrs.background_pixel = (x.XBlackPixel)(dpy, screen);
        let window = (x.XCreateWindow)(
            dpy,
            parent as xlib::Window,
            0,
            0,
            w.max(1) as u32,
            h.max(1) as u32,
            0,
            depth,
            xlib::InputOutput as u32,
            visual,
            xlib::CWColormap | xlib::CWBorderPixel | xlib::CWBackPixel,
            &mut attrs,
        );
        if window == 0 {
            return 0;
        }
        (x.XMapWindow)(dpy, window);
        (x.XFlush)(dpy);
        window as u64
    }
}
