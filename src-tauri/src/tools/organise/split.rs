use std::path::{Path, PathBuf};
use lopdf::Document;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_progress, emit_complete, emit_error};
use crate::tools::ProcessRequest;

/// Parse a comma-separated range string like "1-3,5,7-9" into a sorted, deduplicated
/// list of 1-based page numbers. Returns an error if any page exceeds total_pages or
/// if the syntax is invalid.
pub fn parse_range(range_str: &str, total_pages: usize) -> Result<Vec<usize>> {
    let mut pages: Vec<usize> = Vec::new();

    for segment in range_str.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        if let Some((start_str, end_str)) = segment.split_once('-') {
            let start = start_str.trim().parse::<usize>().map_err(|_| {
                AppError::Validation(format!("Invalid page number: '{}'", start_str.trim()))
            })?;
            let end = end_str.trim().parse::<usize>().map_err(|_| {
                AppError::Validation(format!("Invalid page number: '{}'", end_str.trim()))
            })?;

            if start == 0 || end == 0 {
                return Err(AppError::Validation(format!(
                    "Page numbers must be 1 or greater in range '{segment}'"
                )));
            }

            if start > total_pages || end > total_pages {
                return Err(AppError::Validation(format!(
                    "Page range {}-{} exceeds total page count {}",
                    start, end, total_pages
                )));
            }
            if start > end {
                return Err(AppError::Validation(format!(
                    "Invalid range: {} > {}",
                    start, end
                )));
            }

            for p in start..=end {
                pages.push(p);
            }
        } else {
            let page = segment.parse::<usize>().map_err(|_| {
                AppError::Validation(format!("Invalid page number: '{}'", segment))
            })?;

            if page == 0 {
                return Err(AppError::Validation("Page number must be 1 or greater".into()));
            }

            if page > total_pages {
                return Err(AppError::Validation(format!(
                    "Page {} exceeds total page count {}",
                    page, total_pages
                )));
            }

            pages.push(page);
        }
    }

    // Sort and deduplicate
    pages.sort_unstable();
    pages.dedup();

    Ok(pages)
}

/// Split a slice of page numbers into chunks of size n.
/// The last chunk may be smaller than n.
pub fn chunk_by_n(pages: &[usize], n: usize) -> Vec<Vec<usize>> {
    pages.chunks(n).map(|c| c.to_vec()).collect()
}

/// Derive the output filename stem for a split chunk.
/// E.g. `/tmp/report.pdf` with chunk_index 1 → `"report_split_1"`.
pub fn output_stem_for_chunk(input: &Path, chunk_index: usize) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_split_{chunk_index}")
}

/// Extract a subset of pages from a document by cloning it and removing all pages
/// that are NOT in page_numbers (1-based).
fn extract_pages(doc: &Document, page_numbers: &[usize]) -> Result<Document> {
    let mut new_doc = doc.clone();
    let total = new_doc.get_pages().len();

    // Compute which pages to delete (pages NOT in the target set)
    let keep: std::collections::HashSet<usize> = page_numbers.iter().copied().collect();
    let to_delete: Vec<u32> = (1..=total)
        .filter(|p| !keep.contains(p))
        .map(|p| p as u32)
        .collect();

    if !to_delete.is_empty() {
        new_doc.delete_pages(&to_delete);
    }

    Ok(new_doc)
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let input_path = req.input_paths.first().ok_or_else(|| {
        let msg = "Split requires at least one input file".to_string();
        emit_error(&app, &op_id, &msg);
        AppError::Validation(msg)
    })?;

    crate::pipeline::validate::validate_pdf(input_path, "split")?;

    emit_progress(&app, &op_id, 5, "Loading document\u{2026}");

    let doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load {:?}: {e}", input_path)))?;

    let total_pages = doc.get_pages().len();

    // Determine chunks from options
    let chunks: Vec<Vec<usize>> =
        if let Some(range_str) = req.options.get("range").and_then(|v| v.as_str()) {
            vec![parse_range(range_str, total_pages)?]
        } else if let Some(n) = req
            .options
            .get("every_n_pages")
            .and_then(|v| v.as_u64())
        {
            let n = n as usize;
            if n == 0 {
                let msg = "every_n_pages must be at least 1".to_string();
                emit_error(&app, &op_id, &msg);
                return Err(AppError::Validation(msg));
            }
            chunk_by_n(&(1..=total_pages).collect::<Vec<_>>(), n)
        } else {
            let msg = "Split requires 'range' or 'every_n_pages' option".to_string();
            emit_error(&app, &op_id, &msg);
            return Err(AppError::Validation(msg));
        };

    let out_dir = input_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    let chunk_count = chunks.len();
    let mut out_paths: Vec<PathBuf> = Vec::with_capacity(chunk_count);

    for (i, chunk) in chunks.into_iter().enumerate() {
        // Scale progress from 20% to 90%
        let percent = 20u8 + ((i as u32 * 70 / chunk_count.max(1) as u32) as u8);
        emit_progress(
            &app,
            &op_id,
            percent,
            &format!("Writing chunk {}/{}\u{2026}", i + 1, chunk_count),
        );

        let mut new_doc = extract_pages(&doc, &chunk)?;
        let stem = output_stem_for_chunk(input_path, i + 1);
        let out_path = out_dir.join(format!("{stem}.pdf"));

        new_doc
            .save(&out_path)
            .map_err(|e| AppError::Pdf(format!("Failed to save split PDF {:?}: {e}", out_path)))?;

        out_paths.push(out_path);
    }

    emit_complete(&app, &op_id);

    out_paths
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Validation("No chunks were produced".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_single_page() {
        assert_eq!(parse_range("5", 10).unwrap(), vec![5usize]);
    }

    #[test]
    fn parse_range_span() {
        assert_eq!(parse_range("2-4", 10).unwrap(), vec![2, 3, 4]);
    }

    #[test]
    fn parse_range_mixed() {
        assert_eq!(parse_range("1-3,5,7-9", 10).unwrap(), vec![1, 2, 3, 5, 7, 8, 9]);
    }

    #[test]
    fn parse_range_out_of_bounds_returns_error() {
        assert!(parse_range("1-15", 10).is_err());
    }

    #[test]
    fn parse_range_invalid_syntax_returns_error() {
        assert!(parse_range("a-b", 10).is_err());
    }

    #[test]
    fn chunks_by_n_splits_correctly() {
        let pages = vec![1, 2, 3, 4, 5];
        let chunks = chunk_by_n(&pages, 2);
        assert_eq!(chunks, vec![vec![1, 2], vec![3, 4], vec![5]]);
    }

    #[test]
    fn output_stem_for_split() {
        use std::path::PathBuf;
        let p = PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem_for_chunk(&p, 1), "report_split_1");
        assert_eq!(output_stem_for_chunk(&p, 2), "report_split_2");
    }
}
