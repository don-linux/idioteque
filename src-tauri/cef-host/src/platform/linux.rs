//! Display y visibilidad de la ventana Alloy en Linux.

use crate::exit::{self, fatal};

pub const WINDOW_NAME: &str = "idioteque-browser";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayEnvError {
    Empty,
}

impl DisplayEnvError {
    pub fn message(self) -> &'static str {
        "sin compositor Wayland"
    }
}

/// Visible Alloy needs a Wayland compositor. Health never calls this.
pub fn compositor_present(wayland_display: Option<&str>) -> Result<(), DisplayEnvError> {
    match wayland_display {
        Some(value) if !value.is_empty() => Ok(()),
        _ => Err(DisplayEnvError::Empty),
    }
}

pub fn ensure_display() {
    let value = std::env::var("WAYLAND_DISPLAY").ok();
    if let Err(error) = compositor_present(value.as_deref()) {
        fatal(exit::NO_DISPLAY, error.message());
    }
}

/// CEF Alloy creates the toplevel. No extra native threads.
pub fn init_threads() {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityOp {
    Show,
    Hide,
}

pub fn plan_visibility(visible: bool) -> VisibilityOp {
    if visible {
        VisibilityOp::Show
    } else {
        VisibilityOp::Hide
    }
}

pub fn hidden_flag(visible: bool) -> i32 {
    i32::from(!visible)
}

pub fn clamp_extent(value: i32) -> i32 {
    value.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_display_is_required_for_visible() {
        assert!(compositor_present(Some("wayland-0")).is_ok());
        assert!(compositor_present(Some("wayland-1")).is_ok());
        assert_eq!(compositor_present(None), Err(DisplayEnvError::Empty));
        assert_eq!(compositor_present(Some("")), Err(DisplayEnvError::Empty));
        assert_eq!(DisplayEnvError::Empty.message(), "sin compositor Wayland");
        assert_eq!(exit::NO_DISPLAY, 16);
    }

    #[test]
    fn show_maps_hide_unmaps() {
        assert_eq!(plan_visibility(true), VisibilityOp::Show);
        assert_eq!(plan_visibility(false), VisibilityOp::Hide);
        assert_eq!(hidden_flag(true), 0);
        assert_eq!(hidden_flag(false), 1);
    }

    #[test]
    fn extents_are_at_least_one() {
        assert_eq!(clamp_extent(0), 1);
        assert_eq!(clamp_extent(-4), 1);
        assert_eq!(clamp_extent(1200), 1200);
    }

    #[test]
    fn window_name_is_stable() {
        assert_eq!(WINDOW_NAME, "idioteque-browser");
    }
}
