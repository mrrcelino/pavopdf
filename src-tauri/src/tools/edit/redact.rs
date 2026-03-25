use std::path::PathBuf;
use lopdf::{Document, Object, Stream, Dictionary};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_progress, emit_complete, emit_error};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize)]
pub struct RedactOptions {
    pub regions: Vec<RedactRegion>,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct RedactRegion {
    pub page: usize,    // 1-based page number
    pub x: f32,         // PDF points from left
    pub y: f32,         // PDF points from bottom
    pub width: f32,     // width in points
    pub height: f32,    // height in points
}

/// Build a PDF content stream that draws filled black rectangles over the given regions.
pub fn build_redact_content(regions: &[RedactRegion]) -> Vec<u8> {
    let mut parts: Vec<String> = Vec::with_capacity(regions.len());
    for r in regions {
        parts.push(format!(
            "q 0 0 0 rg {x:.2} {y:.2} {w:.2} {h:.2} re f Q",
            x = r.x,
            y = r.y,
            w = r.width,
            h = r.height,
        ));
    }
    parts.join(" ").into_bytes()
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
        return emit_and_return("Redact requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "redact")?;

    let opts: RedactOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| {
            let msg = format!("Invalid redact options: {e}");
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    if opts.regions.is_empty() {
        return emit_and_return("Redact requires at least one region".into());
    }

    emit_progress(&app, &op_id, 10, "Loading PDF\u{2026}");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    // Validate page numbers
    let pages_map = doc.get_pages();
    let total_pages = pages_map.len();
    for region in &opts.regions {
        if region.page == 0 || region.page > total_pages {
            return emit_and_return(format!(
                "Invalid page number {}: document has {} pages",
                region.page, total_pages
            ));
        }
    }

    // Group regions by page number
    let mut regions_by_page: std::collections::HashMap<usize, Vec<RedactRegion>> =
        std::collections::HashMap::new();
    for region in &opts.regions {
        regions_by_page
            .entry(region.page)
            .or_default()
            .push(region.clone());
    }

    // Sorted page ids by page number
    let page_ids: std::collections::BTreeMap<u32, lopdf::ObjectId> = pages_map;

    let pages_to_process = regions_by_page.len();
    let mut processed = 0;

    for (page_num, page_regions) in &regions_by_page {
        emit_progress(
            &app,
            &op_id,
            (20 + processed * 60 / pages_to_process.max(1)) as u8,
            &format!("Redacting page {}", page_num),
        );

        let page_id = page_ids
            .get(&(*page_num as u32))
            .ok_or_else(|| AppError::Pdf(format!("Page {} not found in document", page_num)))?;

        let content = build_redact_content(page_regions);
        append_content_stream(&mut doc, *page_id, content)?;
        processed += 1;
    }

    // Save next to input file
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_redacted.pdf"));

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
    fn build_redact_content_single_region() {
        let regions = vec![RedactRegion {
            page: 1,
            x: 100.0,
            y: 200.0,
            width: 150.0,
            height: 20.0,
        }];
        let content = String::from_utf8(build_redact_content(&regions)).unwrap();
        assert!(content.contains("re f"), "should contain 're f' for rectangle fill, got: {content}");
        assert!(content.contains("0 0 0 rg"), "should set black fill color, got: {content}");
    }

    #[test]
    fn build_redact_content_multiple_regions() {
        let regions = vec![
            RedactRegion { page: 1, x: 10.0, y: 20.0, width: 100.0, height: 15.0 },
            RedactRegion { page: 1, x: 50.0, y: 80.0, width: 200.0, height: 25.0 },
        ];
        let content = String::from_utf8(build_redact_content(&regions)).unwrap();
        let count = content.matches("re f").count();
        assert_eq!(count, 2, "should contain 2 're f' entries, got {count} in: {content}");
    }

    #[test]
    fn redact_region_deserialize() {
        let json = serde_json::json!({
            "page": 1,
            "x": 100.0,
            "y": 200.0,
            "width": 150.0,
            "height": 20.0
        });
        let region: RedactRegion = serde_json::from_value(json).unwrap();
        assert_eq!(region.page, 1);
        assert!((region.x - 100.0).abs() < f32::EPSILON);
        assert!((region.width - 150.0).abs() < f32::EPSILON);
    }
}
