mod app_config;
mod cef;
mod fonts;
mod git;
mod pty;
mod workspace;

use tauri::{Emitter, Manager};

/// Recuperación del runtime CEF al arrancar (ver CONTRACT 8). Si promueve un
/// candidate verificado, el aviso sale cuando el frontend ya escucha.
fn recover_cef_runtime(app: &tauri::AppHandle) {
    let paths = match cef::paths::CefPaths::from_app(app) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("[cef] sin runtime: {error}");
            return;
        }
    };
    if let Err(error) = paths.ensure_dirs() {
        eprintln!("[cef] {error}");
    }
    match cef::promote::recover_at_startup(&paths, false) {
        Ok(Some(promoted)) => {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let _ = app.emit(
                    "cef-update",
                    cef::updater::UpdateEvent::Updated {
                        chromium: promoted.chromium_version,
                        cef: promoted.cef_version,
                    },
                );
            });
        }
        Ok(None) => {}
        Err(error) => eprintln!("[cef] recuperación al arrancar: {error}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(pty::PtyState::default())
        .manage(workspace::WatchState::default())
        .manage(cef::host::CefState::default())
        .setup(|app| {
            recover_cef_runtime(app.handle());
            cef::updater::start_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_config::load_app_config,
            app_config::record_recent_folder,
            app_config::remove_recent_folder,
            app_config::update_terminal_settings,
            app_config::update_appearance_settings,
            app_config::update_layout_settings,
            app_config::update_workspace_view,
            fonts::list_system_fonts,
            workspace::list_workspace_dirs,
            workspace::list_context_tree,
            workspace::read_markdown,
            workspace::write_markdown,
            workspace::create_markdown,
            workspace::create_directory,
            workspace::delete_markdown,
            workspace::rename_markdown,
            workspace::rename_directory,
            workspace::move_markdown,
            workspace::move_directory,
            workspace::delete_directory,
            workspace::watch_workspace,
            workspace::unwatch_workspace,
            git::git_probe,
            git::git_status,
            git::git_refs,
            git::git_graph,
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            pty::pty_kill_all,
            cef::host::browser_spawn,
            cef::host::browser_command,
            cef::host::browser_set_bounds,
            cef::host::browser_set_visible,
            cef::host::browser_focus_app,
            cef::host::browser_kill,
            cef::state::cef_runtime_info,
            cef::updater::cef_check_updates
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                cef::host::kill_on_exit(&app.state::<cef::host::CefState>());
            }
        });
}
