pub mod error;
// pub mod storage;   // Task 4-5
// pub mod pipeline;  // Task 6
// pub mod commands;  // Task 7
// pub mod tools;     // Task 7

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
