use std::path::PathBuf;
use lopdf::{Document, Object};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::{
    temp::TempStage,
    progress::{emit_progress, emit_complete, emit_error},
};
use crate::tools::ProcessRequest;

/// Merge a list of PDF documents into a single document.
/// The pages from each document are appended in order.
/// Returns an error if the list is empty.
pub fn merge_documents(docs: Vec<Document>) -> Result<Document> {
    if docs.is_empty() {
        return Err(AppError::Validation(
            "Merge requires at least one document".into(),
        ));
    }
    if docs.len() == 1 {
        return Ok(docs.into_iter().next().unwrap());
    }

    let mut iter = docs.into_iter();
    let mut base = iter.next().unwrap();

    for mut doc in iter {
        // Renumber the incoming document's objects so they don't clash with base.
        let next_id = base.max_id + 1;
        doc.renumber_objects_with(next_id);

        // Find the pages node of the base document.
        let base_pages_id = get_pages_id(&base)?;
        // Find the pages node of the incoming document.
        let doc_pages_id = get_pages_id(&doc)?;

        // Move all objects from doc into base.
        for (id, obj) in doc.objects {
            base.objects.insert(id, obj);
        }
        base.max_id = base.max_id.max(doc.max_id);

        // Get Kids array and Count of base pages node.
        let base_kids: Vec<lopdf::ObjectId> = {
            let pages_dict = base
                .get_dictionary(base_pages_id)
                .map_err(|e| AppError::Pdf(e.to_string()))?;
            pages_dict
                .get(b"Kids")
                .and_then(|k| k.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|o| o.as_reference().ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        let base_count = base_kids.len() as i64;

        // Get Kids array and Count from doc pages node.
        let doc_kids: Vec<lopdf::ObjectId> = {
            let pages_dict = base
                .get_dictionary(doc_pages_id)
                .map_err(|e| AppError::Pdf(e.to_string()))?;
            pages_dict
                .get(b"Kids")
                .and_then(|k| k.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|o| o.as_reference().ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        let doc_count = doc_kids.len() as i64;

        // Update each incoming page's Parent to point to base's pages node.
        for &page_id in &doc_kids {
            let page = base
                .get_dictionary_mut(page_id)
                .map_err(|e| AppError::Pdf(format!("Failed to update Parent for page {page_id:?}: {e}")))?;
            page.set("Parent", Object::Reference(base_pages_id));
        }

        // Build new Kids array = base kids + doc kids.
        let mut new_kids: Vec<Object> = base_kids
            .iter()
            .map(|&id| Object::Reference(id))
            .collect();
        new_kids.extend(doc_kids.iter().map(|&id| Object::Reference(id)));

        // Update base pages node with merged Kids and Count.
        {
            let pages_dict = base
                .get_dictionary_mut(base_pages_id)
                .map_err(|e| AppError::Pdf(e.to_string()))?;
            pages_dict.set("Kids", Object::Array(new_kids));
            pages_dict.set("Count", Object::Integer(base_count + doc_count));
        }

        // Remove the now-redundant incoming pages node.
        base.objects.remove(&doc_pages_id);
    }

    Ok(base)
}

/// Extract the ObjectId of the /Pages node from a document's catalog.
fn get_pages_id(doc: &Document) -> Result<lopdf::ObjectId> {
    let catalog = doc.catalog().map_err(|e| AppError::Pdf(e.to_string()))?;
    let pages_id = catalog
        .get(b"Pages")
        .and_then(|o| o.as_reference())
        .map_err(|e| AppError::Pdf(format!("Catalog missing Pages: {e}")))?;
    Ok(pages_id)
}

/// Derive the output filename stem from the first input path.
/// E.g. `/tmp/report.pdf` → `"report_merged"`.
pub fn output_stem(first_input: &PathBuf) -> String {
    let stem = first_input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_merged")
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    if req.input_paths.is_empty() {
        let msg = "Merge requires at least one input file".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    let total = req.input_paths.len();
    let mut docs: Vec<Document> = Vec::with_capacity(total);

    for (i, path) in req.input_paths.iter().enumerate() {
        crate::pipeline::validate::validate_pdf(path, "merge")?;
        emit_progress(
            &app,
            &op_id,
            (i * 40 / total) as u8,
            &format!("Loading file {}/{}", i + 1, total),
        );
        let doc = Document::load(path)
            .map_err(|e| AppError::Pdf(format!("Failed to load {:?}: {e}", path)))?;
        docs.push(doc);
    }

    emit_progress(&app, &op_id, 50, "Merging documents\u{2026}");
    let mut merged = merge_documents(docs)?;

    let stem = if req.output_stem.is_empty() {
        output_stem(&req.input_paths[0])
    } else {
        req.output_stem.clone()
    };
    let out_filename = format!("{stem}.pdf");
    let stage = TempStage::new()?;
    let out_path = stage.output_path(&out_filename);

    emit_progress(&app, &op_id, 80, "Writing output\u{2026}");
    merged
        .save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save merged PDF: {e}")))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Dictionary, Object, Stream};
    use std::path::PathBuf;

    fn make_doc(page_count: usize) -> lopdf::Document {
        let mut doc = lopdf::Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut page_ids = Vec::new();
        for _ in 0..page_count {
            let content =
                Stream::new(Dictionary::new(), b"BT /F1 12 Tf (test) Tj ET".to_vec());
            let content_id = doc.add_object(content);
            let page = dictionary! {
                "Type" => Object::Name(b"Page".to_vec()),
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => Object::Array(vec![
                    0.into(), 0.into(), (595).into(), (842).into()
                ]),
                "Contents" => Object::Reference(content_id),
            };
            let page_id = doc.add_object(page);
            page_ids.push(page_id);
        }
        let pages = dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => Object::Array(
                page_ids.iter().map(|id| Object::Reference(*id)).collect()
            ),
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
    fn merge_two_docs_produces_combined_page_count() {
        let doc_a = make_doc(2);
        let doc_b = make_doc(3);
        let merged = merge_documents(vec![doc_a, doc_b]).expect("merge failed");
        assert_eq!(merged.get_pages().len(), 5);
    }

    #[test]
    fn merge_single_doc_is_identity() {
        let doc = make_doc(4);
        let merged = merge_documents(vec![doc]).expect("merge failed");
        assert_eq!(merged.get_pages().len(), 4);
    }

    #[test]
    fn merge_empty_list_returns_error() {
        let result = merge_documents(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn output_stem_is_correct() {
        let p = PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report_merged");
    }
}
