use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use lopdf::{Document, Object, ObjectId};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

pub fn output_stem(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_reordered")
}

pub fn validate_order(new_order: &[usize], total_pages: usize) -> Result<()> {
    if new_order.len() != total_pages {
        return Err(AppError::Validation(format!(
            "Expected {total_pages} pages in order, got {}",
            new_order.len()
        )));
    }

    let mut seen = HashSet::new();
    for &page in new_order {
        if page == 0 || page > total_pages {
            return Err(AppError::Validation(format!(
                "Page {page} is out of range (1-{total_pages})"
            )));
        }
        if !seen.insert(page) {
            return Err(AppError::Validation(format!(
                "Duplicate page {page} in reorder list"
            )));
        }
    }

    Ok(())
}

fn parse_new_order(req: &ProcessRequest) -> Result<Vec<usize>> {
    req.options
        .get("pages")
        .or_else(|| req.options.get("page_order"))
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::Validation("'pages' array required in options".into()))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .map(|page| page as usize)
                .ok_or_else(|| AppError::Validation("All page entries must be integers".into()))
        })
        .collect()
}

fn apply_reorder(doc: &mut Document, new_order: &[usize]) -> Result<()> {
    let original_pages: BTreeMap<u32, ObjectId> = doc.get_pages();

    let new_kids: Vec<Object> = new_order
        .iter()
        .map(|&page| {
            original_pages
                .get(&(page as u32))
                .copied()
                .map(Object::Reference)
                .ok_or_else(|| AppError::Pdf(format!("Page {page} not found in document")))
        })
        .collect::<Result<Vec<_>>>()?;

    let pages_id = doc
        .catalog()
        .map_err(|e| AppError::Pdf(format!("Catalog error: {e}")))?
        .get(b"Pages")
        .and_then(|value| value.as_reference())
        .map_err(|e| AppError::Pdf(format!("Missing /Pages in catalog: {e}")))?;

    let pages = doc
        .get_dictionary_mut(pages_id)
        .map_err(|e| AppError::Pdf(format!("Pages tree error: {e}")))?;
    pages.set("Kids", Object::Array(new_kids));
    pages.set("Count", Object::Integer(new_order.len() as i64));

    for &page_number in new_order {
        let page_id = original_pages
            .get(&(page_number as u32))
            .copied()
            .ok_or_else(|| AppError::Pdf(format!("Page {page_number} not found in document")))?;
        let page = doc
            .get_dictionary_mut(page_id)
            .map_err(|e| AppError::Pdf(format!("Page object error: {e}")))?;
        page.set("Parent", Object::Reference(pages_id));
    }

    Ok(())
}

#[cfg(test)]
pub fn apply_reorder_direct(doc: &mut Document, new_order: &[usize]) -> Result<()> {
    apply_reorder(doc, new_order)
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    if req.input_paths.len() != 1 {
        let msg = "Reorder requires exactly one input file".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    let input_path = &req.input_paths[0];
    let new_order = parse_new_order(&req).map_err(|err| emit_and_return(err))?;

    validate_pdf(input_path, "reorder").map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 5, "Loading document...");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    validate_order(&new_order, doc.get_pages().len()).map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 35, "Reordering pages...");
    apply_reorder(&mut doc, &new_order).map_err(|err| emit_and_return(err))?;

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

    emit_progress(&app, &op_id, 80, "Writing output...");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ProcessRequest, Tool};
    use serde_json::json;

    #[test]
    fn reorder_reverses_pages() {
        let new_order = vec![4usize, 3, 2, 1];
        assert!(validate_order(&new_order, 4).is_ok());
    }

    #[test]
    fn reorder_validates_duplicate_pages() {
        let new_order = vec![1usize, 1, 2, 3];
        assert!(validate_order(&new_order, 4).is_err());
    }

    #[test]
    fn reorder_validates_missing_pages() {
        let new_order = vec![1usize, 2, 4];
        assert!(validate_order(&new_order, 4).is_err());
    }

    #[test]
    fn reorder_validates_out_of_range() {
        let new_order = vec![1usize, 2, 3, 9];
        assert!(validate_order(&new_order, 4).is_err());
    }

    #[test]
    fn output_stem_reorder() {
        let path = PathBuf::from("/tmp/doc.pdf");
        assert_eq!(output_stem(&path), "doc_reordered");
    }

    fn make_request(options: serde_json::Value) -> ProcessRequest {
        ProcessRequest {
            operation_id: "op-test".into(),
            tool: Tool::Reorder,
            input_paths: vec![PathBuf::from("/tmp/input.pdf")],
            output_stem: "output".into(),
            options,
        }
    }

    #[test]
    fn parse_new_order_reads_pages_array() {
        let req = make_request(json!({ "pages": [3, 1, 2] }));

        assert_eq!(parse_new_order(&req).unwrap(), vec![3, 1, 2]);
    }

    #[test]
    fn parse_new_order_supports_page_order_alias() {
        let req = make_request(json!({ "page_order": [2, 1, 3] }));

        assert_eq!(parse_new_order(&req).unwrap(), vec![2, 1, 3]);
    }
}
