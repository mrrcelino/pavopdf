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

// Note: Tauri's blanket impl `From<T> for InvokeError where T: Serialize`
// automatically covers AppError since it derives Serialize, so no manual
// impl is needed here.

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_to_json() {
        let e = AppError::Validation("file too large".into());
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("Validation"));
        assert!(json.contains("file too large"));
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }
}
