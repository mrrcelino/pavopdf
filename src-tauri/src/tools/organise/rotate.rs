use std::path::PathBuf;
use tauri::AppHandle;
use crate::error::{AppError, Result};
use crate::tools::ProcessRequest;

pub async fn run(_app: AppHandle, _req: ProcessRequest) -> Result<PathBuf> {
    Err(AppError::Pdf(format!("not yet implemented")))
}
