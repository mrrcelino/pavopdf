use std::path::PathBuf;
use lopdf::Document;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct UnlockOptions {
    pub password: String,
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |msg: String| -> Result<PathBuf> {
        emit_error(&app, &op_id, &msg);
        Err(AppError::Pdf(msg))
    };

    if req.input_paths.is_empty() {
        return emit_and_return("Unlock requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "unlock")?;

    let opts: UnlockOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| {
            let msg = format!("Invalid unlock options: {e}");
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    if opts.password.is_empty() {
        return emit_and_return("Password must not be empty".into());
    }

    emit_progress(&app, &op_id, 10, "Loading PDF\u{2026}");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    emit_progress(&app, &op_id, 40, "Attempting to decrypt\u{2026}");
    if doc.is_encrypted() {
        doc.decrypt(&opts.password)
            .map_err(|e| {
                let msg = format!("Failed to decrypt PDF (wrong password?): {e}");
                emit_error(&app, &op_id, &msg);
                AppError::Pdf(msg)
            })?;
        emit_progress(&app, &op_id, 60, "Decryption successful");
    } else {
        emit_progress(&app, &op_id, 60, "PDF is not encrypted, saving clean copy");
    }

    doc.renumber_objects();

    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_unlocked.pdf"));

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
    fn unlock_options_deserialize() {
        let json = serde_json::json!({ "password": "secret123" });
        let opts: UnlockOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.password, "secret123");
    }

    #[test]
    fn output_stem_unlock() {
        let stem = "report";
        let out = format!("{stem}_unlocked.pdf");
        assert_eq!(out, "report_unlocked.pdf");
    }
}
