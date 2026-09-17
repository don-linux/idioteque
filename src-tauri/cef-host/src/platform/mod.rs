//! Platform window embedding. Linux is the complete implementation.
//!
//! macOS and Windows stay compile-ready stubs (CONTRACT.md §2). Tests compile
//! both modules on every host so the unimplemented contract cannot drift into
//! a fake HWND/NSView embed.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(any(target_os = "windows", test))]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(any(target_os = "macos", test))]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{macos, windows};

    /// Tokens that would mean someone started a native embed in a stub.
    const FORBIDDEN_EMBED_APIS: &[&str] = &[
        "CreateWindow",
        "SetParent(",
        "SetWindowPos(",
        "NSView",
        "NSWindow",
        "objc::",
        "cocoa::",
        "appkit::",
        "windows_sys",
        "winapi::",
        "x11_dl",
        "XCreateWindow",
        "XReparentWindow",
        "XOpenDisplay",
        "XMapWindow",
        "XUnmapWindow",
        "wl_egl",
        "wayland_client",
        "HWND",
        "DISPLAY",
        "raw-window-handle",
    ];

    static DISPLAY_LOCK: Mutex<()> = Mutex::new(());

    fn assert_diverging(_f: fn() -> !) {}

    fn handle(bits: usize) -> cef::sys::cef_window_handle_t {
        bits as cef::sys::cef_window_handle_t
    }

    fn without_display(body: impl FnOnce()) {
        let _guard = DISPLAY_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os("DISPLAY");
        std::env::remove_var("DISPLAY");
        body();
        match previous {
            Some(value) => std::env::set_var("DISPLAY", value),
            None => std::env::remove_var("DISPLAY"),
        }
    }

    fn assert_source_is_stable_stub(src: &str, message: &str) {
        assert!(
            src.contains(message),
            "stub lost its stable unimplemented message: {message}"
        );
        assert!(
            src.contains("NO_X11"),
            "unimplemented must keep exit 16 (NO_X11), not a new mac/win code"
        );
        assert!(
            src.contains("pub fn unimplemented_platform() -> !"),
            "unimplemented_platform must stay diverging"
        );
        let platform_fn = src
            .split("pub fn unimplemented_platform() -> !")
            .nth(1)
            .expect("unimplemented_platform body");
        assert!(
            platform_fn.contains("unimplemented_contract()"),
            "unimplemented_platform must fatal unimplemented_contract, not a different payload"
        );
        for api in FORBIDDEN_EMBED_APIS {
            assert!(!src.contains(api), "stub invented native embed ({api})");
        }
    }

    fn assert_window_ops_are_nops(
        map: fn(u64),
        unmap: fn(u64),
        raise: fn(u64),
        move_resize: fn(u64, i32, i32, i32, i32),
        focus: fn(u64),
        reparent: fn(u64, u64, i32, i32),
    ) {
        for xid in [0_u64, 1, 0x2a00_0001, u64::MAX] {
            map(xid);
            unmap(xid);
            raise(xid);
            focus(xid);
            move_resize(xid, 0, 0, 0, 0);
            move_resize(xid, -8, -8, i32::MIN, i32::MAX);
        }
        reparent(0, 0, 0, 0);
        reparent(1, 2, 3, 4);
        reparent(u64::MAX, u64::MAX, i32::MIN, i32::MAX);
    }

    fn assert_create_never_invents_a_window(create: fn(u64, i32, i32) -> u64) {
        let cases = [
            (0, 0, 0),
            (1, 800, 600),
            (0x2a00_0001, 1, 1),
            (u64::MAX, -1, -1),
            (42, i32::MIN, i32::MAX),
        ];
        for (parent, width, height) in cases {
            let first = create(parent, width, height);
            let second = create(parent, width, height);
            assert_eq!(first, 0, "stub must not invent a child for parent={parent}");
            assert_eq!(
                second, 0,
                "second create must stay 0 (no fake hwnd allocator)"
            );
            if parent != 0 {
                assert_ne!(
                    first, parent,
                    "must not echo the parent xid/hwnd as a fake embed"
                );
            }
        }
    }

    #[test]
    fn macos_unimplemented_contract_is_exit_16() {
        let (code, message) = macos::unimplemented_contract();
        assert_eq!(code, crate::exit::NO_X11);
        assert_eq!(code, 16);
        assert_eq!(message, "macos host is not implemented");
        assert_diverging(macos::unimplemented_platform);
    }

    #[test]
    fn windows_unimplemented_contract_is_exit_16() {
        let (code, message) = windows::unimplemented_contract();
        assert_eq!(code, crate::exit::NO_X11);
        assert_eq!(code, 16);
        assert_eq!(message, "windows host is not implemented");
        assert_diverging(windows::unimplemented_platform);
    }

    #[test]
    fn macos_and_windows_messages_stay_distinct() {
        let (_, mac) = macos::unimplemented_contract();
        let (_, win) = windows::unimplemented_contract();
        assert_ne!(mac, win);
        assert!(mac.contains("macos"));
        assert!(win.contains("windows"));
        assert!(!mac.contains("windows"));
        assert!(!win.contains("macos"));
    }

    #[test]
    fn stub_sources_do_not_invent_native_embed() {
        assert_source_is_stable_stub(include_str!("macos.rs"), "macos host is not implemented");
        assert_source_is_stable_stub(
            include_str!("windows.rs"),
            "windows host is not implemented",
        );
    }

    #[test]
    fn macos_create_child_stays_zero_and_window_ops_are_nops() {
        macos::init_threads();
        assert_create_never_invents_a_window(macos::create_default_visual_child);
        assert_window_ops_are_nops(
            macos::map_window,
            macos::unmap_window,
            macos::raise_window,
            macos::move_resize,
            macos::focus_window,
            macos::reparent,
        );
    }

    #[test]
    fn windows_create_child_stays_zero_and_window_ops_are_nops() {
        windows::init_threads();
        assert_create_never_invents_a_window(windows::create_default_visual_child);
        assert_window_ops_are_nops(
            windows::map_window,
            windows::unmap_window,
            windows::raise_window,
            windows::move_resize,
            windows::focus_window,
            windows::reparent,
        );
    }

    #[test]
    fn macos_xid_from_handle_is_always_zero() {
        assert_eq!(macos::xid_from_handle(handle(0)), 0);
        assert_eq!(macos::xid_from_handle(handle(1)), 0);
        assert_eq!(macos::xid_from_handle(handle(0xDEAD_BEEF)), 0);
        assert_eq!(macos::xid_from_handle(handle(usize::MAX)), 0);
    }

    #[test]
    fn windows_xid_from_handle_is_the_bitcast_not_a_window() {
        assert_eq!(windows::xid_from_handle(handle(0)), 0);
        assert_eq!(windows::xid_from_handle(handle(0x1122_3344)), 0x1122_3344);
        assert_eq!(
            windows::xid_from_handle(handle(0xDEAD_BEEF)),
            0xDEAD_BEEF as u64
        );
        // Identity only. Must not mint a non-zero id from a null handle.
        assert_eq!(windows::xid_from_handle(handle(0)), 0);
    }

    #[test]
    fn stub_ensure_display_does_not_require_x11() {
        without_display(|| {
            macos::ensure_display();
            windows::ensure_display();
            macos::init_threads();
            windows::init_threads();
        });
        macos::ensure_display();
        windows::ensure_display();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_module_stays_the_public_embed_on_this_host() {
        let _linux_xid = super::linux::xid_from_handle as fn(cef::sys::cef_window_handle_t) -> u64;
        let _linux_create = super::linux::create_default_visual_child as fn(u64, i32, i32) -> u64;
        let _stub_create = macos::create_default_visual_child as fn(u64, i32, i32) -> u64;
        assert!(
            !std::ptr::fn_addr_eq(_linux_create, _stub_create),
            "Linux must not re-export the mac/win stub create"
        );
    }
}
