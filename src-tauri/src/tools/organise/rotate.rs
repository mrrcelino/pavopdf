use std::path::{Path, PathBuf};

use lopdf::{Document, Object};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

/// Normalize any rotation angle into the 0-359 range.
pub fn normalize_rotation(degrees: i32) -> i32 {
    ((degrees % 360) + 360) % 360
}

/// Parse "all", a comma-separated list, or a range like "2-4" into 1-based page numbers.
pub fn parse_page_selection(selection: &str, total: usize) -> Vec<usize> {
    if selection.trim().eq_ignore_ascii_case("all") {
        return (1..=total).collect();
    }

    let mut pages = Vec::new();
    for part in selection.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start, end)) = part.split_once('-') {
            let start = start.trim().parse::<usize>().ok().unwrap_or(0);
            let end = end.trim().parse::<usize>().ok().unwrap_or(0);
            if start > 0 && end >= start && end <= total {
                pages.extend(start..=end);
            }
        } else {
            let page = part.parse::<usize>().ok().unwrap_or(0);
            if page > 0 && page <= total {
                pages.push(page);
            }
        }
    }

    pages.sort_unstable();
    pages.dedup();
    pages
}

pub fn output_stem(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_rotated")
}

fn selected_pages(req: &ProcessRequest, total: usize) -> Result<Vec<usize>> {
    if let Some(array) = req.options.get("pages").and_then(|value| value.as_array()) {
        let mut pages = Vec::new();
        for value in array {
            let Some(page) = value.as_u64() else {
                return Err(AppError::Validation(
                    "Selected pages must be integers".into(),
                ));
            };
            let page = page as usize;
            if page == 0 || page > total {
                return Err(AppError::Validation(format!(
                    "Page {page} is out of range (1-{total})"
                )));
            }
            pages.push(page);
        }
        pages.sort_unstable();
        pages.dedup();
        return Ok(pages);
    }

    let selection = req
        .options
        .get("pages")
        .and_then(|value| value.as_str())
        .unwrap_or("all");
    Ok(parse_page_selection(selection, total))
}

fn validated_rotation(req: &ProcessRequest) -> Result<i32> {
    let raw = req
        .options
        .get("degrees")
        .and_then(|value| value.as_i64())
        .unwrap_or(90) as i32;

    if raw % 90 != 0 {
        return Err(AppError::Validation(
            "Rotation must be a multiple of 90 degrees".into(),
        ));
    }

    Ok(normalize_rotation(raw))
}

fn apply_rotations(doc: &mut Document, page_numbers: &[usize], degrees: i32) -> Result<()> {
    let page_map = doc.get_pages();
    for page_number in page_numbers {
        if let Some(&page_id) = page_map.get(&(*page_number as u32)) {
            rotate_page(doc, page_id, degrees)?;
        } else {
            return Err(AppError::Validation(format!(
                "Page {} not found in document",
                page_number
            )));
        }
    }
    Ok(())
}

fn rotate_page(doc: &mut Document, page_id: lopdf::ObjectId, additional: i32) -> Result<()> {
    let page = doc
        .get_dictionary_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Page object error: {e}")))?;

    let current = page
        .get(b"Rotate")
        .ok()
        .and_then(|value| value.as_i64().ok())
        .unwrap_or(0) as i32;

    page.set(
        "Rotate",
        Object::Integer(normalize_rotation(current + additional) as i64),
    );
    Ok(())
}

#[cfg(test)]
pub fn rotate_page_direct(doc: &mut Document, page_id: lopdf::ObjectId, degrees: i32) -> Result<()> {
    rotate_page(doc, page_id, degrees)
}

fn build_output_path(input_path: &Path, stem_override: &str) -> Result<PathBuf> {
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory from input path".into()))?;
    let stem = if stem_override.trim().is_empty() {
        output_stem(input_path)
    } else {
        stem_override.trim().to_string()
    };
    Ok(out_dir.join(format!("{stem}.pdf")))
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    if req.input_paths.len() != 1 {
        let msg = "Rotate requires exactly one input file".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    let input_path = &req.input_paths[0];
    let degrees = validated_rotation(&req).map_err(|err| emit_and_return(err))?;

    validate_pdf(input_path, "rotate").map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 5, "Loading document...");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    let total = doc.get_pages().len();
    let pages = selected_pages(&req, total).map_err(|err| emit_and_return(err))?;
    if pages.is_empty() {
        let msg = "No pages selected for rotation".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    emit_progress(&app, &op_id, 20, "Rotating pages...");
    apply_rotations(&mut doc, &pages, degrees).map_err(|err| emit_and_return(err))?;
    emit_progress(&app, &op_id, 80, &format!("Rotated {} pages", pages.len()));

    let out_path = build_output_path(input_path, &req.output_stem)?;

    emit_progress(&app, &op_id, 85, "Writing output...");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save rotated PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ProcessRequest, Tool};
    use lopdf::{dictionary, Dictionary, Stream};
    use serde_json::json;

    fn make_doc(page_count: usize) -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut page_ids = Vec::new();

        for _ in 0..page_count {
            let content = Stream::new(Dictionary::new(), b"BT ET".to_vec());
            let content_id = doc.add_object(content);
            let page = dictionary! {
                "Type" => Object::Name(b"Page".to_vec()),
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
                "Contents" => Object::Reference(content_id),
            };
            page_ids.push(doc.add_object(page));
        }

        let pages = dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => Object::Array(page_ids.iter().copied().map(Object::Reference).collect()),
            "Count" => Object::Integer(page_count as i64),
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog = dictionary! {
            "Type" => Object::Name(b"Catalog".to_vec()),
            "Pages" => Object::Reference(pages_id),
        };
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", Object::Reference(catalog_id));

        doc
    }

    #[test]
    fn normalize_rotation_zero() {
        assert_eq!(normalize_rotation(0), 0);
    }

    #[test]
    fn normalize_rotation_360_wraps() {
        assert_eq!(normalize_rotation(360), 0);
    }

    #[test]
    fn normalize_rotation_450_wraps() {
        assert_eq!(normalize_rotation(450), 90);
    }

    #[test]
    fn normalize_rotation_negative() {
        assert_eq!(normalize_rotation(-90), 270);
    }

    #[test]
    fn parse_page_selection_all() {
        assert_eq!(parse_page_selection("all", 5), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn parse_page_selection_list() {
        assert_eq!(parse_page_selection("1,3,5", 5), vec![1, 3, 5]);
    }

    #[test]
    fn parse_page_selection_range() {
        assert_eq!(parse_page_selection("2-4", 5), vec![2, 3, 4]);
    }

    #[test]
    fn output_stem_rotate() {
        let p = PathBuf::from("/tmp/doc.pdf");
        assert_eq!(output_stem(&p), "doc_rotated");
    }

    #[test]
    fn rotate_page_updates_existing_rotation() {
        let mut doc = make_doc(1);
        let page_id = *doc.get_pages().values().next().unwrap();

        rotate_page(&mut doc, page_id, 90).unwrap();
        rotate_page(&mut doc, page_id, 180).unwrap();

        let page = doc.get_dictionary(page_id).unwrap();
        let rotation = page.get(b"Rotate").unwrap().as_i64().unwrap();
        assert_eq!(rotation, 270);
    }

    fn make_request(options: serde_json::Value) -> ProcessRequest {
        ProcessRequest {
            operation_id: "op-test".into(),
            tool: Tool::Rotate,
            input_paths: vec![PathBuf::from("/tmp/input.pdf")],
            output_stem: "output".into(),
            options,
        }
    }

    #[test]
    fn selected_pages_reads_array_from_options() {
        let req = make_request(json!({ "pages": [3, 1, 3, 2] }));

        assert_eq!(selected_pages(&req, 5).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn validated_rotation_rejects_non_right_angles() {
        let req = make_request(json!({ "degrees": 45 }));

        assert!(validated_rotation(&req).is_err());
    }
}
