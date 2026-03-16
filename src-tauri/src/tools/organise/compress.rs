use std::io::Cursor;
use std::path::{Path, PathBuf};
use image::{DynamicImage, GrayImage, RgbImage};
use lopdf::{Dictionary, Document, Object, Stream};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

// ── Preset ────────────────────────────────────────────────────────────────────

/// Compression preset that controls downsampling DPI and JPEG quality.
pub struct CompressPreset {
    pub dpi: u32,
    pub quality: u8,
}

impl CompressPreset {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "small" => Ok(Self { dpi: 72, quality: 55 }),
            "balanced" => Ok(Self { dpi: 150, quality: 75 }),
            "high_quality" => Ok(Self { dpi: 220, quality: 90 }),
            other => Err(AppError::Validation(format!(
                "Unknown compression preset: '{other}'. Use small, balanced, or high_quality."
            ))),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `"{stem}_compressed"` for the given input path.
pub fn output_stem(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_compressed")
}

/// Remove embedded thumbnail images from every page to shrink file size.
fn strip_thumbnails(doc: &mut Document) {
    let page_ids: Vec<lopdf::ObjectId> =
        doc.get_pages().values().copied().collect();

    for page_id in page_ids {
        if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(page_id) {
            dict.remove(b"Thumb");
        }
    }
}

/// Resample and re-encode images above the preset DPI threshold.
fn compress_images(doc: &mut Document, preset: &CompressPreset) -> Result<()> {
    // Collect IDs of image XObjects first to avoid a borrow conflict.
    let image_ids: Vec<lopdf::ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(&id, obj)| {
            if let Object::Stream(stream) = obj {
                let subtype = stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|o| {
                        if let Object::Name(bytes) = o {
                            std::str::from_utf8(bytes).ok().map(|s| s.to_owned())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if subtype == "Image" {
                    return Some(id);
                }
            }
            None
        })
        .collect();

    let max_dim = preset.dpi * 8;

    for id in image_ids {
        // --- read dimensions and raw data ---------------------------------
        let (width, height, color_space, bits, raw) = {
            let stream = match doc.get_object(id) {
                Ok(Object::Stream(s)) => s,
                _ => continue,
            };
            let w = stream
                .dict
                .get(b"Width")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0) as u32;
            let h = stream
                .dict
                .get(b"Height")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0) as u32;

            if w <= max_dim && h <= max_dim {
                continue;
            }

            let cs = stream
                .dict
                .get(b"ColorSpace")
                .ok()
                .and_then(|o| {
                    if let Object::Name(bytes) = o {
                        std::str::from_utf8(bytes).ok().map(|s| s.to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "DeviceRGB".into());

            let bpc = stream
                .dict
                .get(b"BitsPerComponent")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(8) as u8;

            let content = match stream.decompressed_content() {
                Ok(c) => c,
                Err(_) => continue,
            };

            (w, h, cs, bpc, content)
        };

        // --- build DynamicImage -------------------------------------------
        let img: DynamicImage = if color_space == "DeviceGray" && bits == 8 {
            match GrayImage::from_raw(width, height, raw) {
                Some(g) => DynamicImage::ImageLuma8(g),
                None => continue,
            }
        } else {
            match RgbImage::from_raw(width, height, raw) {
                Some(r) => DynamicImage::ImageRgb8(r),
                None => continue,
            }
        };

        // --- compute new size ---------------------------------------------
        let max_side = width.max(height);
        let scale = max_dim as f32 / max_side as f32;
        let new_w = (width as f32 * scale).round() as u32;
        let new_h = (height as f32 * scale).round() as u32;

        let resized = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);

        // --- JPEG encode --------------------------------------------------
        let mut jpeg_buf = Vec::new();
        resized
            .write_to(
                &mut Cursor::new(&mut jpeg_buf),
                image::ImageFormat::Jpeg,
            )
            .map_err(|e| AppError::Pdf(format!("JPEG encode failed: {e}")))?;

        // --- build replacement stream -------------------------------------
        let mut new_dict = Dictionary::new();
        new_dict.set("Type", Object::Name(b"XObject".to_vec()));
        new_dict.set("Subtype", Object::Name(b"Image".to_vec()));
        new_dict.set("Width", Object::Integer(new_w as i64));
        new_dict.set("Height", Object::Integer(new_h as i64));
        new_dict.set(
            "ColorSpace",
            Object::Name(if color_space == "DeviceGray" {
                b"DeviceGray".to_vec()
            } else {
                b"DeviceRGB".to_vec()
            }),
        );
        new_dict.set("BitsPerComponent", Object::Integer(8));
        new_dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        new_dict.set("Length", Object::Integer(jpeg_buf.len() as i64));
        // DecodeParms intentionally omitted (not needed for DCTDecode)

        let new_stream = Stream::new(new_dict, jpeg_buf);
        doc.objects.insert(id, Object::Stream(new_stream));
    }

    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    if req.input_paths.is_empty() {
        return Err(AppError::Validation(
            "Compress requires exactly one input file".into(),
        ));
    }
    let input_path = &req.input_paths[0];

    let preset_name = req
        .options
        .get("preset")
        .and_then(|v| v.as_str())
        .unwrap_or("balanced");
    let preset = CompressPreset::from_str(preset_name)?;

    validate_pdf(input_path, "compress")?;

    emit_progress(&app, &op_id, 10, "Loading document…");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    emit_progress(&app, &op_id, 30, "Stripping thumbnails…");
    strip_thumbnails(&mut doc);

    emit_progress(&app, &op_id, 50, "Compressing images…");
    compress_images(&mut doc, &preset)?;

    // Determine output path (save next to input)
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };
    let out_path = out_dir.join(format!("{stem}.pdf"));

    emit_progress(&app, &op_id, 85, "Writing output…");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save compressed PDF: {e}")))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_from_str_small() {
        let p = CompressPreset::from_str("small").unwrap();
        assert_eq!(p.dpi, 72);
        assert_eq!(p.quality, 55);
    }

    #[test]
    fn preset_from_str_balanced() {
        let p = CompressPreset::from_str("balanced").unwrap();
        assert_eq!(p.dpi, 150);
    }

    #[test]
    fn preset_from_str_high_quality() {
        let p = CompressPreset::from_str("high_quality").unwrap();
        assert_eq!(p.dpi, 220);
    }

    #[test]
    fn preset_from_str_invalid_returns_error() {
        assert!(CompressPreset::from_str("ultra").is_err());
    }

    #[test]
    fn output_stem_compress() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report_compressed");
    }
}
