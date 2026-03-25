use std::path::{Path, PathBuf};

use printpdf::{BuiltinFont, Mm, PdfDocument};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::tools::ProcessRequest;

/// Font size in pt.
const FONT_SIZE: f32 = 10.0;
/// Line height in mm.
const LINE_HEIGHT: f32 = 5.0;
/// Top margin in mm.
const TOP_Y: f32 = 280.0;
/// Bottom margin in mm.
const BOTTOM_Y: f32 = 20.0;
/// Left margin in mm.
const LEFT_X: f32 = 15.0;
/// Maximum characters per line before wrapping.
const MAX_LINE_CHARS: usize = 90;

/// Derive an output file stem from the input path.
fn output_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string()
}

/// Strip HTML tags, inserting newlines at tag boundaries.
/// This is a simple best-effort approach for v0.1.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                result.push('\n');
            }
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Split long text into lines that fit the page width.
fn wrap_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() <= MAX_LINE_CHARS {
            lines.push(trimmed.to_string());
        } else {
            // Word-wrap long lines
            let mut current = String::new();
            for word in trimmed.split_whitespace() {
                if current.len() + word.len() + 1 > MAX_LINE_CHARS {
                    if !current.is_empty() {
                        lines.push(current);
                    }
                    current = word.to_string();
                } else {
                    if !current.is_empty() {
                        current.push(' ');
                    }
                    current.push_str(word);
                }
            }
            if !current.is_empty() {
                lines.push(current);
            }
        }
    }

    lines
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };
    let input_path = req
        .input_paths
        .first()
        .ok_or_else(|| AppError::Validation("No input file".into()))
        .map_err(|err| emit_and_return(err))?;

    let ext = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "html" && ext != "htm" {
        return Err(emit_and_return(AppError::Validation(format!(
            "Unsupported file format: .{ext} (expected .html or .htm)"
        ))));
    }

    emit_progress(&app, &op_id, 5, "Reading HTML file...");

    let html_content = tokio::fs::read_to_string(input_path).await
        .map_err(|e| AppError::Io(format!("Failed to read HTML file: {e}")))
        .map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 20, "Extracting text...");

    let plain_text = strip_html_tags(&html_content);
    let lines = wrap_lines(&plain_text);

    if lines.is_empty() {
        return Err(emit_and_return(AppError::Validation(
            "HTML file contains no text content".into(),
        )));
    }

    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))
        .map_err(|err| emit_and_return(err))?;
    let out_path = out_dir.join(format!("{stem}.pdf"));

    emit_progress(&app, &op_id, 40, "Creating PDF...");

    // Block scope so PdfDocumentReference (!Send) is dropped before .await
    let pdf_bytes = {
        let (doc, first_page, first_layer) =
            PdfDocument::new("HTML to PDF", Mm(210.0), Mm(297.0), "Page 1");
        let font = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| AppError::Pdf(format!("Failed to add font: {e}")))
            .map_err(|err| emit_and_return(err))?;

        let rows_per_page = ((TOP_Y - BOTTOM_Y) / LINE_HEIGHT) as usize;
        let chunks: Vec<&[String]> = lines.chunks(rows_per_page).collect();
        let total_pages = chunks.len();

        for (page_idx, chunk) in chunks.iter().enumerate() {
            let percent = (40 + (page_idx * 50) / total_pages.max(1)).min(100);
            emit_progress(
                &app,
                &op_id,
                percent as u8,
                &format!("Rendering page {}...", page_idx + 1),
            );

            if page_idx == 0 {
                let layer_ref = doc.get_page(first_page).get_layer(first_layer);
                let mut y = TOP_Y;
                for line in *chunk {
                    layer_ref.use_text(line, FONT_SIZE, Mm(LEFT_X), Mm(y), &font);
                    y -= LINE_HEIGHT;
                }
            } else {
                let label = format!("Page {}", page_idx + 1);
                let (page, layer) = doc.add_page(Mm(210.0), Mm(297.0), &label);
                let layer_ref = doc.get_page(page).get_layer(layer);
                let mut y = TOP_Y;
                for line in *chunk {
                    layer_ref.use_text(line, FONT_SIZE, Mm(LEFT_X), Mm(y), &font);
                    y -= LINE_HEIGHT;
                }
            }
        }

        emit_progress(&app, &op_id, 90, "Saving PDF...");

        doc.save_to_bytes()
            .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))
            .map_err(|err| emit_and_return(err))?
    };

    tokio::fs::write(&out_path, pdf_bytes).await
        .map_err(|e| AppError::Io(format!("Failed to write PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_html() {
        assert_eq!(output_stem(&PathBuf::from("/tmp/page.html")), "page");
    }

    #[test]
    fn strip_html_tags_basic() {
        assert_eq!(strip_html_tags("<p>Hello</p>").trim(), "Hello");
    }

    #[test]
    fn strip_html_tags_nested() {
        let result = strip_html_tags("<div><b>Bold</b> text</div>");
        assert!(result.contains("Bold"));
        assert!(result.contains("text"));
    }

    #[test]
    fn strip_html_tags_empty() {
        assert_eq!(strip_html_tags("").trim(), "");
    }

    #[test]
    fn wrap_lines_short() {
        let lines = wrap_lines("Hello world");
        assert_eq!(lines, vec!["Hello world"]);
    }

    #[test]
    fn wrap_lines_long() {
        let long = "word ".repeat(30); // 150 chars
        let lines = wrap_lines(&long);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.len() <= MAX_LINE_CHARS + 10); // some slack for word boundaries
        }
    }

    #[test]
    fn wrap_lines_skips_empty() {
        let text = "line1\n\n\nline2";
        let lines = wrap_lines(text);
        assert_eq!(lines, vec!["line1", "line2"]);
    }
}
