mod app_config;
mod cef;
mod fonts;
mod git;
mod pty;
mod workspace;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(pty::PtyState::default())
        .manage(workspace::WatchState::default())
        .manage(cef::host::CefState::default())
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
            cef::host::browser_kill
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                cef::host::kill_on_exit(&app.state::<cef::host::CefState>());
            }
        });
}
