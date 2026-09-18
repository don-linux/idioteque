use std::ffi::OsStr;

fn main() {
    #[cfg(target_os = "linux")]
    apply_gdk_backend_for_cef_hole();

    idioteque_lib::run()
}

/// El embed de CEF es un hijo X11. En GNOME Wayland no hay sesión Xorg, pero
/// Mutter levanta XWayland (`DISPLAY`). Hay que fijar el backend antes de que
/// GTK arranque; si el entorno trae `wayland`, el hueco GDK no existe. Sin
/// DISPLAY no se pisa: la app abre y el navegador avisa. No se porta a Wayland
/// nativo: Ozone/X11 + XWayland es el embed que ya arranca.
#[cfg(target_os = "linux")]
fn apply_gdk_backend_for_cef_hole() {
    if let Some(backend) = gdk_backend_override(std::env::var_os("DISPLAY").as_deref()) {
        std::env::set_var("GDK_BACKEND", backend);
    }
}

/// `Some("x11")` only when `DISPLAY` is a non-empty X11/XWayland address.
/// Never `"wayland"`: a native Wayland GDK backend has no XID hole.
fn gdk_backend_override(display: Option<&OsStr>) -> Option<&'static str> {
    match display {
        Some(value) if !value.is_empty() => Some("x11"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn display_set_forces_x11_not_wayland() {
        assert_eq!(gdk_backend_override(Some(OsStr::new(":0"))), Some("x11"));
        assert_eq!(gdk_backend_override(Some(OsStr::new(":1"))), Some("x11"));
        assert_eq!(
            gdk_backend_override(Some(OsStr::new("localhost:10.0"))),
            Some("x11")
        );
    }

    #[test]
    fn empty_or_missing_display_does_not_override() {
        assert_eq!(gdk_backend_override(None), None);
        assert_eq!(gdk_backend_override(Some(OsStr::new(""))), None);
    }

    #[test]
    fn never_selects_native_wayland_backend() {
        // Even a DISPLAY string that mentions wayland is still an X11 address
        // (or junk): the override is x11 or nothing, never a Wayland port.
        for value in [":0", "wayland-0", "wayland-1"] {
            let out = gdk_backend_override(Some(OsStr::new(value)));
            assert_ne!(out, Some("wayland"));
            assert!(out == Some("x11") || out.is_none());
        }
        assert_eq!(
            gdk_backend_override(Some(OsStr::new("wayland-0"))),
            Some("x11"),
            "non-empty DISPLAY still forces X11 so the hole exists on XWayland"
        );
    }

    #[test]
    fn os_string_non_utf8_display_still_forces_x11() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let display = OsString::from_vec(vec![0xff, b':', b'0']);
            assert_eq!(gdk_backend_override(Some(display.as_os_str())), Some("x11"));
        }
        #[cfg(not(unix))]
        {
            let _ = OsString::new();
        }
    }
}
