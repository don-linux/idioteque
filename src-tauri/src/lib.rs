mod app_config;
mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_config::load_app_config,
            app_config::record_recent_folder,
            app_config::remove_recent_folder,
            workspace::list_context_tree,
            workspace::read_markdown,
            workspace::write_markdown
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
