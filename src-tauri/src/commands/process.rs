use std::path::PathBuf;
use tauri::AppHandle;
use crate::{error::Result, tools::{self, ProcessRequest}};

#[tauri::command]
pub async fn process_pdf(app: AppHandle, request: ProcessRequest) -> Result<PathBuf> {
    tools::run(app, request).await
}

#[tauri::command]
pub async fn open_file_dialog(app: AppHandle, multiple: bool) -> Result<Vec<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    let result = if multiple {
        app.dialog()
            .file()
            .blocking_pick_files()
            .map(|files| {
                files
                    .into_iter()
                    .filter_map(|f| f.into_path().ok())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        app.dialog()
            .file()
            .blocking_pick_file()
            .and_then(|f| f.into_path().ok())
            .map(|p| vec![p])
            .unwrap_or_default()
    };
    Ok(result)
}

#[tauri::command]
pub async fn save_file_dialog(
    app: AppHandle,
    suggested_name: String,
) -> Result<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    let path = app
        .dialog()
        .file()
        .set_file_name(&suggested_name)
        .blocking_save_file()
        .and_then(|f| f.into_path().ok());
    Ok(path)
}
