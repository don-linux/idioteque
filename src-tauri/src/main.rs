// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // El navegador CEF se embebe como ventana hija X11: en Wayland la app corre
    // sobre XWayland. Debe fijarse antes de que GTK se inicialice.
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    idioteque_lib::run()
}
