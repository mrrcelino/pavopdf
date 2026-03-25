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

    // Attempt 1: Try running Tesseract directly on the PDF.
    // Some Tesseract builds (with Leptonica PDF support) can handle PDF input.
    emit_progress(&app, &op_id, 10, "Running OCR (direct PDF attempt)\u{2026}");
    let tess_bin = tesseract.clone();
    let input_clone = input_path.clone();
    let output_base_clone = output_base.clone();
    let language = opts.language.clone();
    let direct_status = tokio::task::spawn_blocking(move || {
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

    let direct_ok = direct_status.success() && out_path.exists();

    // Attempt 2: If direct PDF failed, try pdftoppm -> Tesseract pipeline.
    if !direct_ok {
        emit_progress(&app, &op_id, 20, "Direct PDF failed, trying pdftoppm fallback\u{2026}");

        let temp_dir = out_dir.join(format!(".{stem}_ocr_tmp"));
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_prefix = temp_dir.join("page");

        let input_clone = input_path.clone();
        let temp_prefix_clone = temp_prefix.clone();
        let pdftoppm_result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("pdftoppm")
                .args(["-png"])
                .arg(&input_clone)
                .arg(&temp_prefix_clone)
                .status()
        })
        .await
        .map_err(|e| AppError::Pdf(format!("Task join error: {e}")))?;

        let pdftoppm_ok = pdftoppm_result.as_ref().map_or(false, |s| s.success());

        if pdftoppm_ok {
            // Gather page images and run Tesseract on each
            let mut page_images: Vec<PathBuf> = std::fs::read_dir(&temp_dir)
                .map_err(|e| AppError::Pdf(format!("Failed to read temp dir: {e}")))?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
                .collect();
            page_images.sort();

            if page_images.is_empty() {
                let _ = std::fs::remove_dir_all(&temp_dir);
                return emit_and_return(
                    "pdftoppm produced no page images. Cannot perform OCR.".into(),
                );
            }

            let mut page_pdfs: Vec<PathBuf> = Vec::new();
            let total_pages = page_images.len();

            for (i, page_img) in page_images.iter().enumerate() {
                let pct = (30 + ((i as u32 * 50) / total_pages as u32)).min(255) as u8;
                emit_progress(
                    &app,
                    &op_id,
                    pct,
                    &format!("OCR page {}/{}\u{2026}", i + 1, total_pages),
                );

                let page_out_base = temp_dir.join(format!("ocr_page_{i:04}"));
                let page_out_pdf = temp_dir.join(format!("ocr_page_{i:04}.pdf"));

                let tess_bin = tesseract.clone();
                let page_img_clone = page_img.clone();
                let page_out_base_clone = page_out_base.clone();
                let lang = opts.language.clone();

                let tess_status = tokio::task::spawn_blocking(move || {
                    std::process::Command::new(&tess_bin)
                        .arg(&page_img_clone)
                        .arg(&page_out_base_clone)
                        .args(["-l", &lang, "pdf"])
                        .status()
                })
                .await
                .map_err(|e| AppError::Pdf(format!("Task join error: {e}")))?
                .map_err(|e| {
                    let msg = format!("Tesseract failed on page {}: {e}", i + 1);
                    emit_error(&app, &op_id, &msg);
                    AppError::Pdf(msg)
                })?;

                if !tess_status.success() || !page_out_pdf.exists() {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return emit_and_return(format!(
                        "Tesseract failed on page {} with status {}",
                        i + 1,
                        tess_status.code().unwrap_or(-1)
                    ));
                }

                page_pdfs.push(page_out_pdf);
            }

            // Merge page PDFs into one using the existing merge_documents utility
            emit_progress(&app, &op_id, 85, "Merging OCR pages\u{2026}");
            if page_pdfs.len() == 1 {
                std::fs::copy(&page_pdfs[0], &out_path)
                    .map_err(|e| AppError::Pdf(format!("Failed to copy OCR result: {e}")))?;
            } else {
                let docs: Vec<lopdf::Document> = page_pdfs
                    .iter()
                    .map(|p| {
                        lopdf::Document::load(p)
                            .map_err(|e| AppError::Pdf(format!("Failed to load OCR page: {e}")))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut merged = crate::tools::organise::merge::merge_documents(docs)?;
                merged
                    .save(&out_path)
                    .map_err(|e| AppError::Pdf(format!("Failed to save merged OCR PDF: {e}")))?;
            }

            let _ = std::fs::remove_dir_all(&temp_dir);
        } else {
            // Both direct and pdftoppm approaches failed
            return emit_and_return(
                "Tesseract OCR failed. Tesseract cannot process PDF files directly unless \
                 built with Leptonica PDF support. The fallback (pdftoppm) is either not \
                 installed or also failed. To fix this:\n\
                 1. Install poppler-utils (provides pdftoppm) and retry, or\n\
                 2. Convert the PDF to PNG/TIFF images manually, then run OCR on those.\n\
                 Error code: "
                    .to_string()
                    + &direct_status
                        .code()
                        .map_or("-1".to_string(), |c| c.to_string()),
            );
        }
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
