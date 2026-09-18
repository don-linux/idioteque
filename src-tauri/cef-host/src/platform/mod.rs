//! Platform window embedding. Linux/X11 only (CONTRACT.md §2).

#[cfg(not(target_os = "linux"))]
compile_error!("cef-host solo implementa Linux/X11");

mod linux;
pub use linux::*;
