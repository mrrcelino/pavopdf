pub mod error;
pub mod storage;
pub mod pipeline;
pub mod commands;
pub mod tools;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::recent_files::get_recent_files,
            commands::recent_files::remove_recent_file,
            commands::process::process_pdf,
            commands::process::open_file_dialog,
            commands::process::save_file_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
