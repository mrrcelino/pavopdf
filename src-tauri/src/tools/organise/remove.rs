use std::collections::HashSet;
use std::path::{Path, PathBuf};

use lopdf::Document;
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
    format!("{stem}_pages_removed")
}

pub fn validate_removal(pages_to_remove: &[usize], total_pages: usize) -> Result<()> {
    if pages_to_remove.is_empty() {
        return Err(AppError::Validation("No pages selected for removal".into()));
    }

    let unique: HashSet<usize> = pages_to_remove.iter().copied().collect();
    for &page in &unique {
        if page == 0 || page > total_pages {
            return Err(AppError::Validation(format!(
                "Page {page} is out of range (1-{total_pages})"
            )));
        }
    }

    if unique.len() >= total_pages {
        return Err(AppError::Validation(
            "Cannot remove all pages from a document".into(),
        ));
    }

    Ok(())
}

fn parse_pages_to_remove(req: &ProcessRequest) -> Result<Vec<usize>> {
    req.options
        .get("pages")
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

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    if req.input_paths.len() != 1 {
        let msg = "Remove requires exactly one input file".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    let input_path = &req.input_paths[0];
    let pages_to_remove = parse_pages_to_remove(&req).map_err(|err| emit_and_return(err))?;

    validate_pdf(input_path, "remove").map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 5, "Loading document...");
    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))
        .map_err(|err| emit_and_return(err))?;

    let total_pages = doc.get_pages().len();
    validate_removal(&pages_to_remove, total_pages).map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 35, "Removing pages...");
    let page_numbers: Vec<u32> = pages_to_remove.iter().map(|&page| page as u32).collect();
    doc.delete_pages(&page_numbers);

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
    fn validate_removal_rejects_empty_list() {
        assert!(validate_removal(&[], 5).is_err());
    }

    #[test]
    fn validate_removal_rejects_removing_all_pages() {
        assert!(validate_removal(&[1, 2, 3], 3).is_err());
    }

    #[test]
    fn validate_removal_rejects_out_of_range() {
        assert!(validate_removal(&[6], 5).is_err());
    }

    #[test]
    fn validate_removal_accepts_valid_subset() {
        assert!(validate_removal(&[1, 3], 5).is_ok());
    }

    #[test]
    fn output_stem_remove() {
        let path = PathBuf::from("/tmp/doc.pdf");
        assert_eq!(output_stem(&path), "doc_pages_removed");
    }

    fn make_request(options: serde_json::Value) -> ProcessRequest {
        ProcessRequest {
            operation_id: "op-test".into(),
            tool: Tool::Remove,
            input_paths: vec![PathBuf::from("/tmp/input.pdf")],
            output_stem: "output".into(),
            options,
        }
    }

    #[test]
    fn parse_pages_to_remove_reads_pages_array() {
        let req = make_request(json!({ "pages": [1, 3, 4] }));

        assert_eq!(parse_pages_to_remove(&req).unwrap(), vec![1, 3, 4]);
    }

    #[test]
    fn parse_pages_to_remove_rejects_non_integer_entries() {
        let req = make_request(json!({ "pages": [1, "two"] }));

        assert!(parse_pages_to_remove(&req).is_err());
    }
}
