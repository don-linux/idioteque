//! macOS stubs (not implemented in this delivery).

#![allow(dead_code)]

use crate::exit::{self, fatal};

/// CONTRACT.md §2: macosx64/macosarm64 are prepared, not implemented.
/// Same exit as Linux without X11 (16). Not an embed.
pub fn unimplemented_contract() -> (i32, &'static str) {
    (exit::NO_X11, "macos host is not implemented")
}

pub fn init_threads() {}

pub fn ensure_display() {}

pub fn map_window(_xid: u64) {}
pub fn unmap_window(_xid: u64) {}
pub fn raise_window(_xid: u64) {}
pub fn move_resize(_xid: u64, _x: i32, _y: i32, _w: i32, _h: i32) {}
pub fn focus_window(_xid: u64) {}

pub fn reparent(_child: u64, _parent: u64, _x: i32, _y: i32) {}

pub fn create_default_visual_child(_parent: u64, _w: i32, _h: i32) -> u64 {
    0
}

pub fn xid_from_handle(_handle: cef::sys::cef_window_handle_t) -> u64 {
    0
}

pub fn unimplemented_platform() -> ! {
    let (code, message) = unimplemented_contract();
    fatal(code, message)
}
