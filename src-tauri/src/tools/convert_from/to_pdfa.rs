use std::path::PathBuf;
use tauri::AppHandle;
use crate::error::Result;
use crate::tools::ProcessRequest;

pub async fn run(_app: AppHandle, _req: ProcessRequest) -> Result<PathBuf> {
    Err(crate::error::AppError::Pdf("PDF to PDF/A not yet implemented".into()))
}
