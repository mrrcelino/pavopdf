use std::path::{Path, PathBuf};

use docx_rs::{BreakType, Docx, Paragraph, Run};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

use super::pdfium_helper::{load_pdfium, open_pdf};

/// Returns the file stem of the input path (without extension).
fn output_stem(input: &Path) -> String {
    input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Splits text into paragraphs by double-newline boundaries,
/// filtering out empty segments.
fn split_into_paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect()
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |err: AppError| -> AppError {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    // --- Validate input ------------------------------------------------
    if req.input_paths.is_empty() {
        return Err(emit_and_return(AppError::Validation(
            "No input file provided for pdf_to_word".into(),
        )));
    }
    let input_path = &req.input_paths[0];

    validate_pdf(input_path, "pdf_to_word").map_err(|e| emit_and_return(e))?;

    emit_progress(&app, &op_id, 5, "Loading PDF…");

    // --- Load pdfium & open PDF ----------------------------------------
    let pdfium = load_pdfium().map_err(|e| emit_and_return(e))?;
    let document = open_pdf(&pdfium, input_path, None).map_err(|e| emit_and_return(e))?;

    let page_count = document.pages().len();
    if page_count == 0 {
        return Err(emit_and_return(AppError::Pdf(
            "PDF has no pages".into(),
        )));
    }

    emit_progress(&app, &op_id, 10, "Extracting text…");

    // --- Build docx ----------------------------------------------------
    let mut docx = Docx::new();

    for (i, page) in document.pages().iter().enumerate() {
        let page_num = i + 1;

        // Insert a page break between pages (not before the first)
        if i > 0 {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_break(BreakType::Page)),
            );
        }

        let page_text = page
            .text()
            .map_err(|e| {
                emit_and_return(AppError::Pdf(format!(
                    "Failed to extract text from page {page_num}: {e}"
                )))
            })?
            .all();

        let paragraphs = split_into_paragraphs(&page_text);

        for para_text in paragraphs {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(para_text)),
            );
        }

        // Progress: 10-90% across pages
        let pct = 10 + ((i as u64 + 1) * 80 / page_count as u64) as u8;
        emit_progress(
            &app,
            &op_id,
            pct,
            &format!("Processed page {page_num}/{page_count}"),
        );
    }

    emit_progress(&app, &op_id, 90, "Writing .docx file…");

    // --- Write output --------------------------------------------------
    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };

    let out_dir = input_path
        .parent()
        .ok_or_else(|| emit_and_return(AppError::Io("Cannot determine parent directory".into())))?;
    let out_path = out_dir.join(format!("{stem}.docx"));

    let mut buf: Vec<u8> = Vec::new();
    docx.build()
        .pack(&mut std::io::Cursor::new(&mut buf))
        .map_err(|e| {
            emit_and_return(AppError::Io(format!("Failed to build .docx: {e}")))
        })?;

    std::fs::write(&out_path, &buf).map_err(|e| {
        emit_and_return(AppError::Io(format!("Failed to write .docx: {e}")))
    })?;

    emit_progress(&app, &op_id, 100, "Complete");
    emit_complete(&app, &op_id);

    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_word() {
        let p = std::path::PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report");
    }

    #[test]
    fn split_into_paragraphs_basic() {
        let text = "Hello world.\n\nSecond paragraph.\n\nThird.";
        let paras = split_into_paragraphs(text);
        assert_eq!(paras.len(), 3);
        assert_eq!(paras[0], "Hello world.");
    }

    #[test]
    fn split_into_paragraphs_empty() {
        let paras = split_into_paragraphs("");
        assert!(paras.is_empty());
    }

    #[test]
    fn split_into_paragraphs_single_line() {
        let paras = split_into_paragraphs("Just one line");
        assert_eq!(paras.len(), 1);
    }

    #[test]
    fn split_into_paragraphs_trims_whitespace() {
        let text = "  Leading spaces  \n\n  Trailing spaces  ";
        let paras = split_into_paragraphs(text);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0], "Leading spaces");
        assert_eq!(paras[1], "Trailing spaces");
    }

    #[test]
    fn split_into_paragraphs_multiple_newlines() {
        let text = "First\n\n\n\nSecond";
        let paras = split_into_paragraphs(text);
        assert_eq!(paras.len(), 2);
    }

    #[test]
    fn output_stem_no_extension() {
        let p = std::path::PathBuf::from("/tmp/readme");
        assert_eq!(output_stem(&p), "readme");
    }
}
