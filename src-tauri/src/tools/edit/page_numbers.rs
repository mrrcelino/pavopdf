use std::path::PathBuf;
use lopdf::{Document, Object, Stream, dictionary, Dictionary};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_progress, emit_complete, emit_error};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct PageNumberOptions {
    #[serde(default = "default_pn_format")]
    pub format: String,
    #[serde(default = "default_pn_position")]
    pub position: String,
    #[serde(default = "default_pn_font_size")]
    pub font_size: f32,
}

fn default_pn_format() -> String { "Page {n} of {total}".into() }
fn default_pn_position() -> String { "bottom_center".into() }
fn default_pn_font_size() -> f32 { 10.0 }

/// Replace `{n}` with page number and `{total}` with total pages.
pub fn format_page_number(format: &str, page: usize, total: usize) -> String {
    format
        .replace("{n}", &page.to_string())
        .replace("{total}", &total.to_string())
}

/// Convert a position name to (x, y) coordinates given a page width.
pub fn position_to_xy(position: &str, page_width: f32) -> (f32, f32) {
    match position {
        "bottom_center" => (page_width / 2.0, 20.0),
        "bottom_right" => (page_width - 50.0, 20.0),
        "bottom_left" => (50.0, 20.0),
        _ => (page_width / 2.0, 20.0), // default to bottom_center
    }
}

/// Build a PDF content stream that renders page number text at (x, y).
pub fn build_page_number_content(text: &str, x: f32, y: f32, font_size: f32) -> Vec<u8> {
    format!(
        "q BT /F1 {font_size:.1} Tf {x:.2} {y:.2} Td ({text}) Tj ET Q",
        font_size = font_size,
        x = x,
        y = y,
        text = escape_pdf_text(text),
    )
    .into_bytes()
}

/// Escape parentheses and backslashes in PDF text strings.
fn escape_pdf_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Get the MediaBox width from a page, defaulting to 595.0 (A4).
fn get_page_width(doc: &Document, page_id: lopdf::ObjectId) -> f32 {
    let dict = match doc.get_dictionary(page_id) {
        Ok(d) => d,
        Err(_) => return 595.0,
    };
    match dict.get(b"MediaBox") {
        Ok(Object::Array(arr)) if arr.len() >= 4 => {
            match &arr[2] {
                Object::Integer(w) => *w as f32,
                Object::Real(w) => *w,
                _ => 595.0,
            }
        }
        _ => 595.0,
    }
}

/// Ensure the page's Resources dictionary has /F1 as Helvetica.
fn ensure_font_f1(doc: &mut Document, page_id: lopdf::ObjectId) -> Result<()> {
    let page_dict = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Failed to get page dictionary: {e}")))?;

    let has_resources = page_dict.get(b"Resources").is_ok();
    if !has_resources {
        page_dict.set("Resources", Object::Dictionary(Dictionary::new()));
    }

    let page_dict = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Failed to get page dictionary: {e}")))?;

    let resources = page_dict
        .get_mut(b"Resources")
        .map_err(|e| AppError::Pdf(format!("Failed to get Resources: {e}")))?;

    if let Object::Dictionary(ref mut res_dict) = resources {
        let has_font = res_dict.get(b"Font").is_ok();
        if !has_font {
            res_dict.set("Font", Object::Dictionary(Dictionary::new()));
        }
        if let Ok(Object::Dictionary(ref mut font_dict)) = res_dict.get_mut(b"Font") {
            if font_dict.get(b"F1").is_err() {
                let helvetica = dictionary! {
                    "Type" => Object::Name(b"Font".to_vec()),
                    "Subtype" => Object::Name(b"Type1".to_vec()),
                    "BaseFont" => Object::Name(b"Helvetica".to_vec()),
                };
                font_dict.set("F1", Object::Dictionary(helvetica));
            }
        }
    }

    Ok(())
}

/// Append a content stream to a page's Contents entry.
fn append_content_stream(doc: &mut Document, page_id: lopdf::ObjectId, content_bytes: Vec<u8>) -> Result<()> {
    let new_stream = Stream::new(Dictionary::new(), content_bytes);
    let new_stream_id = doc.add_object(Object::Stream(new_stream));

    let page_dict = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Failed to get page dictionary: {e}")))?;

    match page_dict.get(b"Contents") {
        Ok(Object::Reference(old_ref)) => {
            let old_ref = *old_ref;
            page_dict.set(
                "Contents",
                Object::Array(vec![
                    Object::Reference(old_ref),
                    Object::Reference(new_stream_id),
                ]),
            );
        }
        Ok(Object::Array(arr)) => {
            let mut new_arr = arr.clone();
            new_arr.push(Object::Reference(new_stream_id));
            page_dict.set("Contents", Object::Array(new_arr));
        }
        _ => {
            page_dict.set("Contents", Object::Reference(new_stream_id));
        }
    }

    Ok(())
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |msg: String| -> Result<PathBuf> {
        emit_error(&app, &op_id, &msg);
        Err(AppError::Pdf(msg))
    };

    if req.input_paths.is_empty() {
        return emit_and_return("Page Numbers requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "page_numbers")?;

    let opts: PageNumberOptions = serde_json::from_value(req.options.clone())
        .unwrap_or(PageNumberOptions {
            format: default_pn_format(),
            position: default_pn_position(),
            font_size: default_pn_font_size(),
        });

    emit_progress(&app, &op_id, 10, "Loading PDF\u{2026}");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    // Collect page ids in order. lopdf's get_pages() returns BTreeMap<u32, ObjectId>
    let pages_map = doc.get_pages();
    let total = pages_map.len();
    let page_ids: Vec<lopdf::ObjectId> = {
        let mut entries: Vec<(u32, lopdf::ObjectId)> = pages_map.into_iter().collect();
        entries.sort_by_key(|(num, _)| *num);
        entries.into_iter().map(|(_, id)| id).collect()
    };

    for (i, &page_id) in page_ids.iter().enumerate() {
        let page_num = i + 1;
        emit_progress(
            &app,
            &op_id,
            (20 + i * 60 / total.max(1)) as u8,
            &format!("Adding page number {}/{}", page_num, total),
        );

        let page_width = get_page_width(&doc, page_id);
        let (x, y) = position_to_xy(&opts.position, page_width);
        let text = format_page_number(&opts.format, page_num, total);
        let content = build_page_number_content(&text, x, y, opts.font_size);

        ensure_font_f1(&mut doc, page_id)?;
        append_content_stream(&mut doc, page_id, content)?;
    }

    // Save next to input file
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_numbered.pdf"));

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
    fn format_page_number_basic() {
        let result = format_page_number("Page {n} of {total}", 3, 10);
        assert_eq!(result, "Page 3 of 10");
    }

    #[test]
    fn format_page_number_simple() {
        let result = format_page_number("{n}", 5, 10);
        assert_eq!(result, "5");
    }

    #[test]
    fn position_bottom_center() {
        let (x, y) = position_to_xy("bottom_center", 595.0);
        assert!((x - 297.5).abs() < 0.01, "x should be ~297.5, got {x}");
        assert!((y - 20.0).abs() < 0.01, "y should be 20.0, got {y}");
    }

    #[test]
    fn position_bottom_right() {
        let (x, y) = position_to_xy("bottom_right", 595.0);
        assert!((x - 545.0).abs() < 0.01, "x should be ~545.0, got {x}");
        assert!((y - 20.0).abs() < 0.01, "y should be 20.0, got {y}");
    }
}
