use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("PDF error: {0}")]
    Pdf(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Cancelled")]
    Cancelled,
    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { AppError::Io(e.to_string()) }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self { AppError::Pdf(e.to_string()) }
}

// Tauri 2 provides a blanket `impl<T: Serialize> From<T> for InvokeError`
// (tauri-2.10.3/src/ipc/mod.rs:240) which automatically covers AppError
// since it derives Serialize. A manual impl here would conflict (E0119).
// Verified stable in Tauri 2.x — the blanket impl serializes via serde_json::to_value.

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_to_json() {
        let e = AppError::Validation("file too large".into());
        let json = serde_json::to_string(&e).unwrap();
        // Assert the adjacently-tagged wire format the frontend depends on
        assert!(json.contains("\"kind\""), "must have 'kind' key, got: {json}");
        assert!(json.contains("\"Validation\""), "must have variant name, got: {json}");
        assert!(json.contains("\"message\""), "must have 'message' key, got: {json}");
        assert!(json.contains("\"file too large\""), "must have message content, got: {json}");
    }

    #[test]
    fn cancelled_serializes_without_message() {
        let e = AppError::Cancelled;
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"kind\""), "got: {json}");
        assert!(json.contains("\"Cancelled\""), "got: {json}");
        // Unit variant must NOT have a message field
        assert!(!json.contains("\"message\""), "unit variant should not have message, got: {json}");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }
}
