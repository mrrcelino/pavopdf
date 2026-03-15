use std::path::PathBuf;
use tauri::AppHandle;
use crate::{
    error::Result,
    storage::recent_files::{self, RecentEntry},
};

#[tauri::command]
pub async fn get_recent_files(app: AppHandle) -> Result<Vec<RecentEntry>> {
    recent_files::load(&app)
}

#[tauri::command]
pub async fn remove_recent_file(app: AppHandle, path: PathBuf) -> Result<()> {
    recent_files::remove(&app, &path)
}
