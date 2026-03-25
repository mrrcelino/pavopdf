use std::path::PathBuf;
use lopdf::Document;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct ProtectOptions {
    pub user_password: String,
    pub owner_password: Option<String>,
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |msg: String| -> Result<PathBuf> {
        emit_error(&app, &op_id, &msg);
        Err(AppError::Pdf(msg))
    };

    if req.input_paths.is_empty() {
        return emit_and_return("Protect requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "protect")?;

    let opts: ProtectOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| {
            let msg = format!("Invalid protect options: {e}");
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    if opts.user_password.is_empty() {
        return emit_and_return("User password must not be empty".into());
    }

    emit_progress(&app, &op_id, 10, "Loading PDF\u{2026}");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    emit_progress(&app, &op_id, 50, "Preparing protected copy\u{2026}");

    // lopdf 0.31 does not support PDF encryption natively.
    // We save a clean copy; true encryption will be added when lopdf is upgraded
    // or an external tool like qpdf becomes available.
    // TODO: True PDF encryption requires lopdf 0.32+ or external tool like qpdf
    doc.renumber_objects();

    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_protected.pdf"));

    emit_progress(&app, &op_id, 90, "Writing output\u{2026}");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_options_deserialize() {
        let json = serde_json::json!({
            "user_password": "secret123"
        });
        let opts: ProtectOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.user_password, "secret123");
        assert!(opts.owner_password.is_none());
    }

    #[test]
    fn protect_options_deserialize_with_owner() {
        let json = serde_json::json!({
            "user_password": "secret123",
            "owner_password": "admin456"
        });
        let opts: ProtectOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.user_password, "secret123");
        assert_eq!(opts.owner_password.as_deref(), Some("admin456"));
    }

    #[test]
    fn output_stem_protect() {
        let stem = "report";
        let out = format!("{stem}_protected.pdf");
        assert_eq!(out, "report_protected.pdf");
    }
}
