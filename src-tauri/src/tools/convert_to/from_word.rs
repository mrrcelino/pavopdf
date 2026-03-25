use std::path::{Path, PathBuf};

use docx_rs::{
    DocumentChild, ParagraphChild, RunChild, TableCellContent, TableChild, TableRowChild,
};
use printpdf::{BuiltinFont, Mm, PdfDocument};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::tools::ProcessRequest;

// ── Constants ────────────────────────────────────────────────────────────────

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_LEFT_MM: f32 = 20.0;
const MARGIN_TOP_MM: f32 = 20.0;
const MARGIN_BOTTOM_MM: f32 = 20.0;
const FONT_SIZE: f32 = 12.0;
const LINE_HEIGHT_MM: f32 = 5.0;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Returns the file stem (without extension) for the given path.
pub fn output_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string()
}

/// Extract all paragraph text strings from a slice of `DocumentChild`.
///
/// Each paragraph becomes one `String` in the returned vec.
/// Tables are flattened: each cell's paragraphs are included in order.
pub fn extract_paragraphs_text(children: &[DocumentChild]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    for child in children {
        match child {
            DocumentChild::Paragraph(p) => {
                lines.push(extract_paragraph_text(&p.children));
            }
            DocumentChild::Table(t) => {
                extract_table_text(&t.rows, &mut lines);
            }
            _ => {}
        }
    }

    lines
}

fn extract_paragraph_text(children: &[ParagraphChild]) -> String {
    let mut text = String::new();

    for child in children {
        if let ParagraphChild::Run(run) = child {
            for rc in &run.children {
                match rc {
                    RunChild::Text(t) => text.push_str(&t.text),
                    RunChild::Tab(_) => text.push('\t'),
                    RunChild::Break(_) => text.push('\n'),
                    _ => {}
                }
            }
        }
    }

    text
}

fn extract_table_text(rows: &[TableChild], lines: &mut Vec<String>) {
    for row_child in rows {
        let TableChild::TableRow(row) = row_child;
        let mut row_texts: Vec<String> = Vec::new();

        for cell_child in &row.cells {
            let TableRowChild::TableCell(cell) = cell_child;
            let mut cell_text = String::new();

            for content in &cell.children {
                if let TableCellContent::Paragraph(p) = content {
                    let t = extract_paragraph_text(&p.children);
                    if !cell_text.is_empty() && !t.is_empty() {
                        cell_text.push(' ');
                    }
                    cell_text.push_str(&t);
                }
            }

            row_texts.push(cell_text);
        }

        lines.push(row_texts.join(" | "));
    }
}

/// Simple word-wrap: splits `text` into lines that fit within `max_chars`.
fn wrap_line(text: &str, max_chars: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut result: Vec<String> = Vec::new();

    for segment in text.split('\n') {
        if segment.is_empty() {
            result.push(String::new());
            continue;
        }

        let words: Vec<&str> = segment.split_whitespace().collect();
        if words.is_empty() {
            result.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                result.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            result.push(current_line);
        }
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

/// Generate PDF bytes from extracted paragraph text (sync, not Send).
fn build_pdf(paragraphs: &[String]) -> Result<Vec<u8>> {
    let (pdf_doc, page1, layer1) = PdfDocument::new(
        "Converted from Word",
        Mm(PAGE_WIDTH_MM),
        Mm(PAGE_HEIGHT_MM),
        "Layer 1",
    );

    let font = pdf_doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(format!("Failed to add font: {e}")))?;

    let usable_width_mm = PAGE_WIDTH_MM - 2.0 * MARGIN_LEFT_MM;
    let approx_char_width_mm = FONT_SIZE * 0.35;
    let max_chars = (usable_width_mm / approx_char_width_mm) as usize;

    let mut current_page = page1;
    let mut current_layer = layer1;
    let mut y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;

    for para in paragraphs {
        let wrapped = wrap_line(para, max_chars);

        for line in &wrapped {
            if y_pos < MARGIN_BOTTOM_MM {
                let (new_page, new_layer) = pdf_doc.add_page(
                    Mm(PAGE_WIDTH_MM),
                    Mm(PAGE_HEIGHT_MM),
                    "Layer 1",
                );
                current_page = new_page;
                current_layer = new_layer;
                y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
            }

            let layer_ref = pdf_doc.get_page(current_page).get_layer(current_layer);
            layer_ref.use_text(line.as_str(), FONT_SIZE, Mm(MARGIN_LEFT_MM), Mm(y_pos), &font);
            y_pos -= LINE_HEIGHT_MM;
        }
    }

    pdf_doc
        .save_to_bytes()
        .map_err(|e| AppError::Pdf(format!("Failed to generate PDF bytes: {e}")))
}

// ── Entry point ──────────────────────────────────────────────────────────────

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    // Validate input
    if req.input_paths.is_empty() {
        let msg = "Word to PDF requires at least one input file".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }
    let input_path = &req.input_paths[0];

    let ext = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "docx" {
        let msg = format!(
            "Expected a .docx file, got '.{ext}'. Only .docx format is supported."
        );
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    if !input_path.exists() {
        let msg = format!("Input file not found: {}", input_path.display());
        emit_error(&app, &op_id, &msg);
        return Err(AppError::NotFound(msg));
    }

    // Read .docx bytes
    emit_progress(&app, &op_id, 10, "Reading Word document...");
    let bytes = tokio::fs::read(input_path)
        .await
        .map_err(|e| AppError::Io(format!("Failed to read .docx file: {e}")))
        .map_err(|err| emit_and_return(err))?;

    // Parse docx
    emit_progress(&app, &op_id, 25, "Parsing document structure...");
    let docx = docx_rs::read_docx(&bytes)
        .map_err(|e| AppError::Pdf(format!("Failed to parse .docx: {e}")))
        .map_err(|err| emit_and_return(err))?;

    // Extract text paragraphs
    emit_progress(&app, &op_id, 40, "Extracting text...");
    let paragraphs = extract_paragraphs_text(&docx.document.children);

    // Build PDF (sync — PdfDocumentReference is !Send so must not cross await)
    emit_progress(&app, &op_id, 55, "Generating PDF...");
    let pdf_bytes = build_pdf(&paragraphs)
        .map_err(|err| emit_and_return(err))?;

    // Determine output path (save next to input)
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))
        .map_err(|err| emit_and_return(err))?;

    let stem = if req.output_stem.trim().is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.trim().to_string()
    };
    let out_path = out_dir.join(format!("{stem}.pdf"));

    // Write PDF to disk
    emit_progress(&app, &op_id, 90, "Writing PDF...");
    tokio::fs::write(&out_path, pdf_bytes)
        .await
        .map_err(|e| AppError::Io(format!("Failed to write PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_from_word() {
        let p = PathBuf::from("/tmp/report.docx");
        assert_eq!(output_stem(&p), "report");
    }

    #[test]
    fn output_stem_no_extension() {
        let p = PathBuf::from("/tmp/readme");
        assert_eq!(output_stem(&p), "readme");
    }

    #[test]
    fn extract_text_from_empty() {
        let text = extract_paragraphs_text(&[]);
        assert!(text.is_empty());
    }

    #[test]
    fn wrap_line_short_text() {
        let lines = wrap_line("hello world", 80);
        assert_eq!(lines, vec!["hello world"]);
    }

    #[test]
    fn wrap_line_long_text() {
        let text = "word ".repeat(20).trim().to_string();
        let lines = wrap_line(&text, 20);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.len() <= 20, "Line too long: '{line}'");
        }
    }

    #[test]
    fn wrap_line_empty() {
        let lines = wrap_line("", 80);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn wrap_line_preserves_newlines() {
        let lines = wrap_line("line one\nline two", 80);
        assert_eq!(lines, vec!["line one", "line two"]);
    }
}
