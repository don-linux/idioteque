mod app_config;
mod fonts;
mod git;
mod pty;
mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(pty::PtyState::default())
        .manage(workspace::WatchState::default())
        .invoke_handler(tauri::generate_handler![
            app_config::load_app_config,
            app_config::record_recent_folder,
            app_config::remove_recent_folder,
            app_config::update_terminal_settings,
            app_config::update_footer_settings,
            app_config::update_appearance_settings,
            fonts::list_system_fonts,
            workspace::list_context_tree,
            workspace::read_markdown,
            workspace::write_markdown,
            workspace::delete_markdown,
            workspace::watch_workspace,
            workspace::unwatch_workspace,
            git::git_probe,
            git::git_status,
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            pty::pty_kill_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
