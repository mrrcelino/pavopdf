use std::path::PathBuf;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct OcrOptions {
    #[serde(default = "default_language")]
    pub language: String,
    pub tesseract_path: Option<String>,
}

fn default_language() -> String {
    "eng".into()
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |msg: String| -> Result<PathBuf> {
        emit_error(&app, &op_id, &msg);
        Err(AppError::Pdf(msg))
    };

    if req.input_paths.is_empty() {
        return emit_and_return("OCR requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "ocr")?;

    let opts: OcrOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| {
            let msg = format!("Invalid OCR options: {e}");
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    let tesseract = opts
        .tesseract_path
        .unwrap_or_else(|| "tesseract".into());

    // Verify tesseract is available
    emit_progress(&app, &op_id, 5, "Checking Tesseract availability\u{2026}");
    let tess_bin = tesseract.clone();
    let version_check = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&tess_bin)
            .arg("--version")
            .output()
    })
    .await
    .map_err(|e| AppError::Pdf(format!("Task join error: {e}")))?;

    if version_check.is_err() {
        return emit_and_return(format!(
            "Tesseract not found at '{}'. Please install Tesseract OCR or provide a custom path.",
            tesseract
        ));
    }

    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    // Tesseract appends .pdf to the output base path
    let output_base = out_dir.join(format!("{stem}_ocr"));
    let out_path = out_dir.join(format!("{stem}_ocr.pdf"));

    emit_progress(&app, &op_id, 10, "Running OCR\u{2026}");
    let tess_bin = tesseract.clone();
    let input_clone = input_path.clone();
    let output_base_clone = output_base.clone();
    let language = opts.language.clone();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&tess_bin)
            .arg(&input_clone)
            .arg(&output_base_clone)
            .args(["-l", &language, "pdf"])
            .status()
    })
    .await
    .map_err(|e| AppError::Pdf(format!("Task join error: {e}")))?
    .map_err(|e| {
        let msg = format!("Failed to run Tesseract: {e}");
        emit_error(&app, &op_id, &msg);
        AppError::Pdf(msg)
    })?;

    if !status.success() {
        return emit_and_return(format!(
            "Tesseract exited with status {}",
            status.code().unwrap_or(-1)
        ));
    }

    if !out_path.exists() {
        return emit_and_return("Tesseract did not produce output file".into());
    }

    emit_progress(&app, &op_id, 95, "OCR complete");
    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_options_deserialize() {
        let json = serde_json::json!({});
        let opts: OcrOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.language, "eng");
        assert!(opts.tesseract_path.is_none());
    }

    #[test]
    fn ocr_options_deserialize_custom() {
        let json = serde_json::json!({
            "language": "deu",
            "tesseract_path": "/usr/local/bin/tesseract"
        });
        let opts: OcrOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.language, "deu");
        assert_eq!(opts.tesseract_path.as_deref(), Some("/usr/local/bin/tesseract"));
    }

    #[test]
    fn output_stem_ocr() {
        let stem = "scanned";
        let out = format!("{stem}_ocr.pdf");
        assert_eq!(out, "scanned_ocr.pdf");
    }
}
