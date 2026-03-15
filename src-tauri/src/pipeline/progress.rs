use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub operation_id: String,
    pub percent: u8,
    pub message: String,
}

pub fn emit_progress(app: &AppHandle, operation_id: &str, percent: u8, message: &str) {
    let _ = app.emit("pdf-progress", ProgressEvent {
        operation_id: operation_id.into(),
        percent,
        message: message.into(),
    });
}

pub fn emit_complete(app: &AppHandle, operation_id: &str) {
    let _ = app.emit("pdf-complete", serde_json::json!({
        "operation_id": operation_id,
    }));
}

pub fn emit_error(app: &AppHandle, operation_id: &str, message: &str) {
    let _ = app.emit("pdf-error", serde_json::json!({
        "operation_id": operation_id,
        "message": message,
    }));
}
