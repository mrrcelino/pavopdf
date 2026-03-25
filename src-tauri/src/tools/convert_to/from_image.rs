use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{GenericImageView, ImageFormat};
use lopdf::dictionary;
use lopdf::{Document, Object, Stream};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::tools::ProcessRequest;

/// A4 dimensions in mm.
const A4_WIDTH_MM: f64 = 210.0;
const A4_HEIGHT_MM: f64 = 297.0;

/// Convert mm to PDF points (1 pt = 25.4/72 mm).
fn mm_to_pt(mm: f64) -> f64 {
    mm * 72.0 / 25.4
}

/// Convert pixel dimensions to mm at 72 DPI (1 px = 1 pt = 25.4/72 mm).
pub fn px_to_mm(width_px: u32, height_px: u32) -> (f64, f64) {
    let w_mm = width_px as f64 * 25.4 / 72.0;
    let h_mm = height_px as f64 * 25.4 / 72.0;
    (w_mm, h_mm)
}

/// Extract the file stem from an input path, falling back to "image".
pub fn output_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_string()
}

/// Encode a `DynamicImage` to JPEG bytes.
fn encode_jpeg(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let rgb = img.to_rgb8();
    let mut buf = Vec::new();
    rgb.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
        .map_err(|e| AppError::Pdf(format!("Failed to encode image as JPEG: {e}")))?;
    Ok(buf)
}

/// Build a single-page or multi-page PDF from images using lopdf.
///
/// Each image becomes one page. Page size is determined by `page_size`:
/// - `"fit"` (default): page matches image dimensions at 72 DPI.
/// - `"a4"`: A4 page with the image centered and scaled to fit.
fn build_pdf(
    images: &[(image::DynamicImage, PathBuf)],
    page_size: &str,
    app: &AppHandle,
    op_id: &str,
) -> Result<Document> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids: Vec<Object> = Vec::with_capacity(images.len());

    let total = images.len();

    for (i, (img, path)) in images.iter().enumerate() {
        // Progress: scale from 20% to 85%
        let pct = 20_usize + (i * 65 / total.max(1));
        emit_progress(
            app,
            op_id,
            pct.min(100) as u8,
            &format!(
                "Embedding image {}/{}\u{2026}",
                i + 1,
                total
            ),
        );

        let (w_px, h_px) = img.dimensions();

        // Image size in points (at 72 DPI, 1px = 1pt)
        let img_w_pt = w_px as f64;
        let img_h_pt = h_px as f64;

        // Determine page size and image placement
        let (page_w_pt, page_h_pt, draw_x, draw_y, draw_w, draw_h) = if page_size == "a4" {
            let pw = mm_to_pt(A4_WIDTH_MM);
            let ph = mm_to_pt(A4_HEIGHT_MM);
            // Scale image to fit within A4 with margins
            let scale = (pw / img_w_pt).min(ph / img_h_pt).min(1.0);
            let dw = img_w_pt * scale;
            let dh = img_h_pt * scale;
            let dx = (pw - dw) / 2.0;
            let dy = (ph - dh) / 2.0;
            (pw, ph, dx, dy, dw, dh)
        } else {
            // "fit" mode: page matches image
            (img_w_pt, img_h_pt, 0.0, 0.0, img_w_pt, img_h_pt)
        };

        // Encode to JPEG
        let jpeg_bytes = encode_jpeg(img).map_err(|e| {
            AppError::Pdf(format!("Failed to encode {:?}: {e}", path))
        })?;

        // Create image XObject stream with DCTDecode (JPEG)
        let img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w_px as i64,
                "Height" => h_px as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8_i64,
                "Filter" => "DCTDecode",
                "Length" => jpeg_bytes.len() as i64,
            },
            jpeg_bytes,
        );
        let img_id = doc.add_object(img_stream);

        // Resources dictionary referencing the image
        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Img0" => img_id,
            },
        });

        // Content stream: draw image scaled to (draw_w, draw_h) at (draw_x, draw_y)
        let content_str = format!(
            "q {draw_w:.2} 0 0 {draw_h:.2} {draw_x:.2} {draw_y:.2} cm /Img0 Do Q"
        );
        let content_stream = Stream::new(dictionary! {}, content_str.into_bytes());
        let content_id = doc.add_object(content_stream);

        // Page object
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(page_w_pt as f32), Object::Real(page_h_pt as f32)],
            "Contents" => content_id,
            "Resources" => resources_id,
        });

        page_ids.push(page_id.into());
    }

    // Pages node
    let count = page_ids.len() as i64;
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => count,
        "Kids" => page_ids,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    Ok(doc)
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    if req.input_paths.is_empty() {
        let msg = "Image to PDF requires at least one input image".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    emit_progress(&app, &op_id, 5, "Loading images\u{2026}");

    // Load all images
    let mut images: Vec<(image::DynamicImage, PathBuf)> = Vec::with_capacity(req.input_paths.len());
    let total = req.input_paths.len();

    for (i, path) in req.input_paths.iter().enumerate() {
        let pct = 5_usize + (i * 15 / total.max(1));
        emit_progress(
            &app,
            &op_id,
            pct.min(100) as u8,
            &format!("Loading image {}/{}\u{2026}", i + 1, total),
        );

        let img = image::open(path).map_err(|e| {
            emit_and_return(AppError::Pdf(format!(
                "Failed to open image {:?}: {e}",
                path
            )))
        })?;
        images.push((img, path.clone()));
    }

    // Determine page_size option
    let page_size = req
        .options
        .get("page_size")
        .and_then(|v| v.as_str())
        .unwrap_or("fit");

    if page_size != "fit" && page_size != "a4" {
        let msg = format!("Invalid page_size option: '{page_size}'. Expected 'fit' or 'a4'.");
        return Err(emit_and_return(AppError::Validation(msg)));
    }

    emit_progress(&app, &op_id, 20, "Building PDF\u{2026}");

    let mut doc = build_pdf(&images, page_size, &app, &op_id)
        .map_err(|e| emit_and_return(e))?;

    // Determine output path next to the first input file
    let first_input = &req.input_paths[0];
    let out_dir = first_input.parent().ok_or_else(|| {
        emit_and_return(AppError::Validation(
            "Cannot determine output directory from input path".to_string(),
        ))
    })?;

    let stem = if req.output_stem.trim().is_empty() {
        output_stem(first_input)
    } else {
        let s = req.output_stem.trim();
        if s.to_ascii_lowercase().ends_with(".pdf") {
            s[..s.len() - 4].to_string()
        } else {
            s.to_string()
        }
    };

    let out_path = out_dir.join(format!("{stem}.pdf"));

    emit_progress(&app, &op_id, 90, "Writing PDF\u{2026}");

    doc.save(&out_path)
        .map_err(|e| emit_and_return(AppError::Pdf(format!("Failed to save PDF {:?}: {e}", out_path))))?;

    emit_complete(&app, &op_id);

    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_from_image() {
        let p = PathBuf::from("/tmp/photo.jpg");
        assert_eq!(output_stem(&p), "photo");
    }

    #[test]
    fn output_stem_fallback() {
        let p = PathBuf::from("/");
        // No file stem — should fall back to "image"
        assert_eq!(output_stem(&p), "image");
    }

    #[test]
    fn dimensions_at_72dpi() {
        let (w_mm, h_mm) = px_to_mm(640, 480);
        assert!(w_mm > 0.0);
        assert!(h_mm > 0.0);
        // 640px at 72dpi = 640/72 inches = 8.889 inches = 225.78mm
        assert!((w_mm - 225.78).abs() < 1.0);
        // 480px at 72dpi = 480/72 inches = 6.667 inches = 169.33mm
        assert!((h_mm - 169.33).abs() < 1.0);
    }

    #[test]
    fn mm_to_pt_conversion() {
        // 25.4mm = 1 inch = 72pt
        let pt = mm_to_pt(25.4);
        assert!((pt - 72.0).abs() < 0.01);
    }

    #[test]
    fn px_to_mm_single_pixel() {
        let (w, h) = px_to_mm(1, 1);
        // 1px at 72dpi = 1/72 inch = 25.4/72 mm ≈ 0.3528mm
        assert!((w - 25.4 / 72.0).abs() < 0.001);
        assert!((h - 25.4 / 72.0).abs() < 0.001);
    }
}
