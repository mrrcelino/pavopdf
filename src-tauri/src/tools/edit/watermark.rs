use std::path::PathBuf;
use lopdf::{Document, Object, Stream, dictionary, Dictionary};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_progress, emit_complete, emit_error};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct WatermarkOptions {
    pub text: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_rotation")]
    pub rotation: f32,
}

fn default_font_size() -> f32 { 48.0 }
fn default_opacity() -> f32 { 0.3 }
fn default_rotation() -> f32 { 45.0 }

/// Build a PDF content stream that renders a centered, rotated watermark text.
pub fn build_watermark_content(text: &str, font_size: f32, opacity: f32, rotation: f32) -> Vec<u8> {
    let rad = rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    // A4 center in points: 595/2 = 297.5, 841/2 = 420.5
    format!(
        "q {opacity:.4} g BT /F1 {font_size:.1} Tf {cos:.6} {sin:.6} {neg_sin:.6} {cos2:.6} 297.5 420.5 Tm ({text}) Tj ET Q",
        opacity = opacity,
        font_size = font_size,
        cos = cos,
        sin = sin,
        neg_sin = -sin,
        cos2 = cos,
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

/// Ensure the page's Resources dictionary has /F1 as Helvetica.
fn ensure_font_f1(doc: &mut Document, page_id: lopdf::ObjectId) -> Result<()> {
    let page_dict = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Failed to get page dictionary: {e}")))?;

    // Get or create Resources
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
        return emit_and_return("Watermark requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "watermark")?;

    let opts: WatermarkOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| {
            let msg = format!("Invalid watermark options: {e}");
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    if opts.text.trim().is_empty() {
        return emit_and_return("Watermark text must not be empty".into());
    }

    emit_progress(&app, &op_id, 10, "Loading PDF\u{2026}");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let content_bytes = build_watermark_content(&opts.text, opts.font_size, opts.opacity, opts.rotation);

    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().values().copied().collect();
    let total = page_ids.len();

    for (i, &page_id) in page_ids.iter().enumerate() {
        emit_progress(
            &app,
            &op_id,
            (20 + i * 60 / total.max(1)) as u8,
            &format!("Watermarking page {}/{}", i + 1, total),
        );
        ensure_font_f1(&mut doc, page_id)?;
        append_content_stream(&mut doc, page_id, content_bytes.clone())?;
    }

    // Save next to input file
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_watermarked.pdf"));

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
    fn build_watermark_content_contains_text() {
        let content = build_watermark_content("DRAFT", 48.0, 0.3, 45.0);
        let text = String::from_utf8(content).unwrap();
        assert!(text.contains("DRAFT"), "content should contain watermark text, got: {text}");
    }

    #[test]
    fn build_watermark_content_sets_opacity() {
        let content = build_watermark_content("TEST", 48.0, 0.3, 45.0);
        let text = String::from_utf8(content).unwrap();
        assert!(text.contains("0.3"), "content should contain opacity value, got: {text}");
    }

    #[test]
    fn watermark_options_deserialize() {
        let json = serde_json::json!({"text": "DRAFT"});
        let opts: WatermarkOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.text, "DRAFT");
        assert!((opts.font_size - 48.0).abs() < f32::EPSILON);
        assert!((opts.opacity - 0.3).abs() < f32::EPSILON);
        assert!((opts.rotation - 45.0).abs() < f32::EPSILON);
    }
}
