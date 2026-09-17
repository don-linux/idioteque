// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // El embed de CEF es un hijo X11. En GNOME Wayland (Ubuntu 26.04) no hay
    // sesión Xorg, pero Mutter levanta XWayland (`DISPLAY`). Hay que fijar el
    // backend antes de que GTK arranque; si el entorno trae `wayland`, el hueco
    // no existe. Sin DISPLAY no se pisa: la app abre y el navegador avisa.
    #[cfg(target_os = "linux")]
    {
        let display = std::env::var_os("DISPLAY");
        if display.as_ref().is_some_and(|value| !value.is_empty()) {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    idioteque_lib::run()
}
