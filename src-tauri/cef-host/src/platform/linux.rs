//! Display y visibilidad de la ventana Alloy en Linux.

use crate::exit::{self, fatal};

pub const WINDOW_NAME: &str = "idioteque-browser";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowIdentity {
    pub title: &'static str,
    pub wayland_app_id: &'static str,
    pub wm_class_class: &'static str,
    pub wm_class_name: &'static str,
    pub wm_role_name: &'static str,
}

/// Views Alloy identity on Wayland. All fields are the CEF window, not the editor.
pub fn window_identity() -> WindowIdentity {
    WindowIdentity {
        title: WINDOW_NAME,
        wayland_app_id: WINDOW_NAME,
        wm_class_class: WINDOW_NAME,
        wm_class_name: WINDOW_NAME,
        wm_role_name: WINDOW_NAME,
    }
}

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

/// CEF Views creates the toplevel. No extra native threads.
pub fn init_threads() {}

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
    fn hidden_flag_inverts_visibility() {
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
        assert_ne!(WINDOW_NAME, "idioteque");
    }

    #[test]
    fn window_identity_is_browser_not_editor() {
        let identity = window_identity();
        const BROWSER: &str = "idioteque-browser";
        assert_eq!(WINDOW_NAME, BROWSER);
        assert_eq!(identity.title, BROWSER);
        assert_eq!(identity.wayland_app_id, BROWSER);
        assert_eq!(identity.wm_class_class, BROWSER);
        assert_eq!(identity.wm_class_name, BROWSER);
        assert_eq!(identity.wm_role_name, BROWSER);
        for field in [
            identity.title,
            identity.wayland_app_id,
            identity.wm_class_class,
            identity.wm_class_name,
            identity.wm_role_name,
        ] {
            assert_eq!(field, BROWSER);
            assert_ne!(field, "idioteque");
            assert_ne!(field, "chromium");
            assert_ne!(field, "cef");
        }
    }
}
