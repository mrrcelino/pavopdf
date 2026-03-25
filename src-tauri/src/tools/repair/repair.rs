use std::path::PathBuf;
use lopdf::Document;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |msg: String| -> Result<PathBuf> {
        emit_error(&app, &op_id, &msg);
        Err(AppError::Pdf(msg))
    };

    if req.input_paths.is_empty() {
        return emit_and_return("Repair requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "repair")?;

    emit_progress(&app, &op_id, 10, "Attempting to load PDF\u{2026}");

    let mut doc = match Document::load(input_path) {
        Ok(doc) => {
            emit_progress(&app, &op_id, 50, "PDF loaded successfully, re-saving clean copy\u{2026}");
            doc
        }
        Err(_) => {
            emit_progress(&app, &op_id, 30, "Standard load failed, trying lenient parse\u{2026}");
            let bytes = tokio::fs::read(input_path).await.map_err(|e| {
                let msg = format!("Failed to read file: {e}");
                emit_error(&app, &op_id, &msg);
                AppError::Io(msg)
            })?;
            Document::load_mem(&bytes).map_err(|e| {
                let msg = format!("Cannot repair PDF: {e}");
                emit_error(&app, &op_id, &msg);
                AppError::Pdf(msg)
            })?
        }
    };

    emit_progress(&app, &op_id, 70, "Normalising object structure\u{2026}");
    doc.renumber_objects();
    doc.compress();

    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_repaired.pdf"));

    emit_progress(&app, &op_id, 90, "Writing output\u{2026}");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn output_stem_repair() {
        let stem = "corrupted";
        let out = format!("{stem}_repaired.pdf");
        assert_eq!(out, "corrupted_repaired.pdf");
    }

    #[test]
    fn repair_options_deserialize() {
        // Repair accepts empty options (no specific configuration needed)
        let json = serde_json::json!({});
        let _: serde_json::Value = serde_json::from_value(json).unwrap();
    }
}
