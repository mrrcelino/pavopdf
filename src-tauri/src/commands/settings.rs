use tauri::AppHandle;
use crate::{error::Result, storage::settings::{self, Settings}};

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Settings> {
    settings::load(&app)
}

#[tauri::command]
pub async fn set_settings(app: AppHandle, settings: Settings) -> Result<()> {
    settings::save(&app, &settings)
}
