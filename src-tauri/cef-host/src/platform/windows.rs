//! Windows stubs (not implemented in this delivery).

#![allow(dead_code)]

use crate::exit::{self, fatal};

pub fn init_threads() {}

pub fn ensure_display() {
    // No X11 on Windows.
}

pub fn map_window(_xid: u64) {}
pub fn unmap_window(_xid: u64) {}
pub fn raise_window(_xid: u64) {}
pub fn move_resize(_xid: u64, _x: i32, _y: i32, _w: i32, _h: i32) {}
pub fn focus_window(_xid: u64) {}

pub fn reparent(_child: u64, _parent: u64, _x: i32, _y: i32) {}

pub fn xid_from_handle(handle: cef::sys::cef_window_handle_t) -> u64 {
    handle as usize as u64
}

pub fn unimplemented_platform() -> ! {
    fatal(exit::NO_X11, "windows host is not implemented")
}
