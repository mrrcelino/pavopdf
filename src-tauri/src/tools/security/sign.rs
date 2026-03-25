use std::path::PathBuf;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::ImageFormat;
use lopdf::{Document, Object, Stream, Dictionary, dictionary};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct SignOptions {
    pub signature_png_base64: String,
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Build the content stream that places the signature image on the page.
pub fn build_signature_content_stream(x: f32, y: f32, width: f32, height: f32) -> Vec<u8> {
    format!(
        "q {width:.4} 0 0 {height:.4} {x:.4} {y:.4} cm /Sig0 Do Q",
        width = width,
        height = height,
        x = x,
        y = y,
    )
    .into_bytes()
}

/// Decode a base64 PNG and re-encode as JPEG bytes, returning (jpeg_bytes, pixel_width, pixel_height).
fn decode_png_to_jpeg(base64_png: &str) -> Result<(Vec<u8>, u32, u32)> {
    let png_bytes = STANDARD
        .decode(base64_png)
        .map_err(|e| AppError::Validation(format!("Invalid base64 in signature: {e}")))?;

    let img = image::load_from_memory_with_format(&png_bytes, ImageFormat::Png)
        .map_err(|e| AppError::Validation(format!("Invalid PNG image: {e}")))?;

    let (pw, ph) = (img.width(), img.height());
    let rgb = img.to_rgb8();

    let mut jpeg_buf = std::io::Cursor::new(Vec::new());
    rgb.write_to(&mut jpeg_buf, ImageFormat::Jpeg)
        .map_err(|e| AppError::Pdf(format!("Failed to encode signature as JPEG: {e}")))?;

    Ok((jpeg_buf.into_inner(), pw, ph))
}

/// Add a JPEG image XObject to the document and return its ObjectId.
fn add_jpeg_xobject(doc: &mut Document, jpeg_bytes: Vec<u8>, pixel_w: u32, pixel_h: u32) -> lopdf::ObjectId {
    let img_dict = dictionary! {
        "Type" => Object::Name(b"XObject".to_vec()),
        "Subtype" => Object::Name(b"Image".to_vec()),
        "Width" => Object::Integer(pixel_w as i64),
        "Height" => Object::Integer(pixel_h as i64),
        "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()),
        "BitsPerComponent" => Object::Integer(8),
        "Filter" => Object::Name(b"DCTDecode".to_vec()),
    };
    let stream = Stream::new(img_dict, jpeg_bytes);
    doc.add_object(Object::Stream(stream))
}

/// Ensure the page's Resources dictionary has /XObject/Sig0 pointing to the given object.
fn ensure_xobject_sig0(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    img_obj_id: lopdf::ObjectId,
) -> Result<()> {
    // Resolve whether Resources is a reference or inline
    let resources_ref = {
        let page_dict = doc
            .get_dictionary(page_id)
            .map_err(|e| AppError::Pdf(format!("Failed to get page dictionary: {e}")))?;
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => Some(*id),
            Ok(Object::Dictionary(_)) => None,
            _ => {
                let page_dict_mut = doc
                    .get_dictionary_mut(page_id)
                    .map_err(|e| AppError::Pdf(format!("Failed to get page dict: {e}")))?;
                page_dict_mut.set("Resources", Object::Dictionary(Dictionary::new()));
                None
            }
        }
    };

    if let Some(res_id) = resources_ref {
        let res_dict = doc
            .get_dictionary_mut(res_id)
            .map_err(|e| AppError::Pdf(format!("Failed to resolve Resources: {e}")))?;
        inject_xobject_sig0(res_dict, img_obj_id);
        return Ok(());
    }

    let page_dict = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Failed to get page dictionary: {e}")))?;
    if let Ok(Object::Dictionary(ref mut res_dict)) = page_dict.get_mut(b"Resources") {
        inject_xobject_sig0(res_dict, img_obj_id);
    }

    Ok(())
}

fn inject_xobject_sig0(res_dict: &mut Dictionary, img_obj_id: lopdf::ObjectId) {
    let has_xobj = res_dict.get(b"XObject").is_ok();
    if !has_xobj {
        res_dict.set("XObject", Object::Dictionary(Dictionary::new()));
    }
    if let Ok(Object::Dictionary(ref mut xobj_dict)) = res_dict.get_mut(b"XObject") {
        xobj_dict.set("Sig0", Object::Reference(img_obj_id));
    }
}

/// Append a content stream to a page's Contents entry.
fn append_content_stream(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    content_bytes: Vec<u8>,
) -> Result<()> {
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
        return emit_and_return("Sign requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "sign")?;

    let opts: SignOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| {
            let msg = format!("Invalid sign options: {e}");
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    if opts.signature_png_base64.is_empty() {
        return emit_and_return("Signature image must not be empty".into());
    }
    if opts.page == 0 {
        return emit_and_return("Page number must be 1 or greater".into());
    }

    emit_progress(&app, &op_id, 10, "Decoding signature image\u{2026}");
    let (jpeg_bytes, pixel_w, pixel_h) = decode_png_to_jpeg(&opts.signature_png_base64)?;

    emit_progress(&app, &op_id, 30, "Loading PDF\u{2026}");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let pages = doc.get_pages();
    let page_id = pages
        .get(&(opts.page as u32))
        .copied()
        .ok_or_else(|| {
            let msg = format!("Page {} does not exist (PDF has {} pages)", opts.page, pages.len());
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    emit_progress(&app, &op_id, 50, "Embedding signature\u{2026}");
    let img_obj_id = add_jpeg_xobject(&mut doc, jpeg_bytes, pixel_w, pixel_h);
    ensure_xobject_sig0(&mut doc, page_id, img_obj_id)?;

    let content = build_signature_content_stream(opts.x, opts.y, opts.width, opts.height);
    append_content_stream(&mut doc, page_id, content)?;

    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_signed.pdf"));

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
    fn sign_options_deserialize() {
        let json = serde_json::json!({
            "signature_png_base64": "aWdub3Jl",
            "page": 1,
            "x": 100.0,
            "y": 200.0,
            "width": 150.0,
            "height": 50.0
        });
        let opts: SignOptions = serde_json::from_value(json).unwrap();
        assert_eq!(opts.page, 1);
        assert!((opts.x - 100.0).abs() < f32::EPSILON);
        assert!((opts.width - 150.0).abs() < f32::EPSILON);
    }

    #[test]
    fn output_stem_sign() {
        let stem = "contract";
        let out = format!("{stem}_signed.pdf");
        assert_eq!(out, "contract_signed.pdf");
    }

    #[test]
    fn build_signature_content_stream_format() {
        let content = build_signature_content_stream(100.0, 200.0, 150.0, 50.0);
        let text = String::from_utf8(content).unwrap();
        assert!(text.starts_with("q "), "should start with save graphics state");
        assert!(text.ends_with(" Do Q"), "should end with Do Q");
        assert!(text.contains("/Sig0"), "should reference /Sig0 XObject");
        assert!(text.contains("150.0000"), "should contain width");
        assert!(text.contains("50.0000"), "should contain height");
        assert!(text.contains("100.0000"), "should contain x position");
        assert!(text.contains("200.0000"), "should contain y position");
    }
}
