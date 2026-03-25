use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::ImageFormat;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

use super::pdfium_helper::{load_pdfium, open_pdf};

// ---------------------------------------------------------------------------
// Option parsers
// ---------------------------------------------------------------------------

/// Derive the output stem from the input filename: `"report"` → `"report_images"`.
fn output_stem(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    format!("{stem}_images")
}

/// Parse the image format from options. Only "png", "jpeg", "webp" are valid;
/// anything else defaults to "png".
fn parse_format(opts: &serde_json::Value) -> &str {
    opts.get("format")
        .and_then(|v| v.as_str())
        .filter(|f| matches!(*f, "png" | "jpeg" | "webp"))
        .unwrap_or("png")
}

/// Parse DPI from options (default 150).
fn parse_dpi(opts: &serde_json::Value) -> u32 {
    opts.get("dpi")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(1200).max(72) as u32)
        .unwrap_or(150)
}

/// Parse JPEG quality from options (default 90, clamped to 1..=100).
fn parse_quality(opts: &serde_json::Value) -> u8 {
    opts.get("quality")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100).max(1) as u8)
        .unwrap_or(90)
}

/// Parse an optional list of 1-based page numbers.
fn parse_pages(opts: &serde_json::Value) -> Option<Vec<usize>> {
    opts.get("pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
}

/// Map format string to the file extension used on disk.
fn format_ext(format: &str) -> &str {
    match format {
        "jpeg" => "jpg",
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Image saving
// ---------------------------------------------------------------------------

/// Save a `DynamicImage` to `path` in the requested format.
fn save_image(
    img: &image::DynamicImage,
    path: &Path,
    format: &str,
    quality: u8,
) -> Result<()> {
    match format {
        "jpeg" => {
            let file = std::fs::File::create(path)?;
            let mut writer = std::io::BufWriter::new(file);
            let encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| AppError::Pdf(format!("Failed to encode JPEG: {e}")))?;
        }
        "webp" => {
            img.save_with_format(path, ImageFormat::WebP)
                .map_err(|e| AppError::Pdf(format!("Failed to save WebP: {e}")))?;
        }
        _ => {
            // Default to PNG
            img.save_with_format(path, ImageFormat::Png)
                .map_err(|e| AppError::Pdf(format!("Failed to save PNG: {e}")))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    // --- Validate input -------------------------------------------------------
    let input_path = req
        .input_paths
        .first()
        .ok_or_else(|| AppError::Validation("No input file provided".into()))?;

    validate_pdf(input_path, "pdf_to_image")?;
    emit_progress(&app, &op_id, 10, "Loading PDF...");

    // --- Parse options --------------------------------------------------------
    let format = parse_format(&req.options).to_owned();
    let dpi = parse_dpi(&req.options);
    let quality = parse_quality(&req.options);
    let selected_pages = parse_pages(&req.options);
    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };

    let ext = format_ext(&format);

    // --- Load Pdfium & open document -----------------------------------------
    let pdfium = load_pdfium()?;
    let document = open_pdf(&pdfium, input_path, None)?;

    let total_pages = document.pages().len() as usize;
    if total_pages == 0 {
        return Err(AppError::Pdf("PDF contains no pages".into()));
    }

    // --- Determine which pages to render -------------------------------------
    let page_indices: Vec<usize> = match &selected_pages {
        Some(pages) => {
            let mut indices = Vec::new();
            for &p in pages {
                if p == 0 || p > total_pages {
                    return Err(AppError::Validation(format!(
                        "Page {p} is out of range (document has {total_pages} pages)"
                    )));
                }
                indices.push(p - 1); // convert to 0-based
            }
            indices
        }
        None => (0..total_pages).collect(),
    };

    let num_output = page_indices.len();
    let output_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Pdf("Cannot determine output directory".into()))?;

    let mut first_output: Option<PathBuf> = None;

    emit_progress(&app, &op_id, 20, "Rendering pages...");

    // --- Render each page ----------------------------------------------------
    for (i, &page_idx) in page_indices.iter().enumerate() {
        let page = document
            .pages()
            .get(page_idx as u16)
            .map_err(|e| AppError::Pdf(format!("Failed to get page {}: {e}", page_idx + 1)))?;

        // Calculate pixel dimensions from page points and target DPI.
        let width_points = page.width().value;
        let height_points = page.height().value;
        let width_px = ((width_points as f64) * (dpi as f64) / 72.0).round() as i32;
        let height_px = ((height_points as f64) * (dpi as f64) / 72.0).round() as i32;

        let config = pdfium_render::prelude::PdfRenderConfig::new()
            .set_target_size(width_px, height_px);

        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| AppError::Pdf(format!("Failed to render page {}: {e}", page_idx + 1)))?;

        let img = bitmap.as_image();

        // Build output filename
        let out_path = if num_output == 1 {
            output_dir.join(format!("{stem}.{ext}"))
        } else {
            output_dir.join(format!("{stem}_page_{}.{ext}", page_idx + 1))
        };

        save_image(&img, &out_path, &format, quality)?;

        if first_output.is_none() {
            first_output = Some(out_path);
        }

        // Progress: 20% → 95% spread across pages
        let percent = 20 + ((i + 1) as u8 * 75 / num_output as u8).min(75);
        emit_progress(
            &app,
            &op_id,
            percent,
            &format!("Rendered page {} of {}", i + 1, num_output),
        );
    }

    emit_complete(&app, &op_id);

    first_output.ok_or_else(|| AppError::Pdf("No pages were rendered".into()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_image() {
        let p = std::path::PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report_images");
    }

    #[test]
    fn parse_format_defaults_to_png() {
        let opts = serde_json::json!({});
        assert_eq!(parse_format(&opts), "png");
    }

    #[test]
    fn parse_format_reads_option() {
        let opts = serde_json::json!({"format": "jpeg"});
        assert_eq!(parse_format(&opts), "jpeg");
    }

    #[test]
    fn parse_format_invalid_defaults_to_png() {
        let opts = serde_json::json!({"format": "bmp"});
        assert_eq!(parse_format(&opts), "png");
    }

    #[test]
    fn parse_dpi_defaults_to_150() {
        let opts = serde_json::json!({});
        assert_eq!(parse_dpi(&opts), 150);
    }

    #[test]
    fn parse_dpi_reads_option() {
        let opts = serde_json::json!({"dpi": 300});
        assert_eq!(parse_dpi(&opts), 300);
    }

    #[test]
    fn parse_quality_defaults_to_90() {
        let opts = serde_json::json!({});
        assert_eq!(parse_quality(&opts), 90);
    }

    #[test]
    fn parse_quality_clamps_high() {
        let opts = serde_json::json!({"quality": 200});
        assert_eq!(parse_quality(&opts), 100);
    }

    #[test]
    fn parse_quality_clamps_low() {
        let opts = serde_json::json!({"quality": 0});
        assert_eq!(parse_quality(&opts), 1);
    }

    #[test]
    fn parse_pages_none_when_missing() {
        let opts = serde_json::json!({});
        assert!(parse_pages(&opts).is_none());
    }

    #[test]
    fn parse_pages_extracts_values() {
        let opts = serde_json::json!({"pages": [1, 3, 5]});
        assert_eq!(parse_pages(&opts), Some(vec![1, 3, 5]));
    }

    #[test]
    fn format_ext_jpeg_maps_to_jpg() {
        assert_eq!(format_ext("jpeg"), "jpg");
    }

    #[test]
    fn format_ext_png_unchanged() {
        assert_eq!(format_ext("png"), "png");
    }

    #[test]
    fn format_ext_webp_unchanged() {
        assert_eq!(format_ext("webp"), "webp");
    }
}
