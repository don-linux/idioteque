//! Exit codes from CONTRACT.md §4.7 and a single fatal path that emits JSON first.

use std::io::Write;
use std::process;

use crate::protocol::{self, HostEvent};

pub const OK: i32 = 0;
pub const BAD_ARGS: i32 = 2;
pub const API_INCOMPAT: i32 = 10;
pub const INIT_FAILED: i32 = 11;
pub const HEALTH_TIMEOUT: i32 = 12;
pub const VERSION_MISMATCH: i32 = 13;
pub const BAD_SLOT: i32 = 14;
pub const SANDBOX: i32 = 15;
pub const NO_X11: i32 = 16;

/// Emit `{"event":"fatal",...}` on the protocol fd and terminate. Never returns.
pub fn fatal(code: i32, message: impl Into<String>) -> ! {
    let message = message.into();
    eprintln!("cef-host fatal {code}: {message}");
    protocol::emit(&HostEvent::Fatal {
        message,
        code,
    });
    // Best-effort flush of the original stderr as well.
    let _ = std::io::stderr().flush();
    process::exit(code);
}
