use std::path::PathBuf;
use lopdf::{Document, Object, dictionary};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_progress, emit_complete, emit_error};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

#[derive(Debug, serde::Deserialize, Default)]
pub struct MetadataOptions {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
}

/// Apply metadata fields to the document's Info dictionary.
/// - `None` = leave unchanged
/// - `Some("")` = remove the field
/// - `Some(text)` = set the field
pub fn apply_metadata(mut doc: Document, opts: &MetadataOptions) -> Result<Document> {
    // Get or create the Info dictionary object id
    let info_id = match doc.trailer.get(b"Info") {
        Ok(obj) => obj.as_reference().ok(),
        Err(_) => None,
    };

    let info_id = match info_id {
        Some(id) => id,
        None => {
            // Create a new Info dictionary
            let new_info = dictionary! {};
            let id = doc.add_object(Object::Dictionary(new_info));
            doc.trailer.set("Info", Object::Reference(id));
            id
        }
    };

    let dict = doc
        .get_dictionary_mut(info_id)
        .map_err(|e| AppError::Pdf(format!("Failed to access Info dictionary: {e}")))?;

    apply_field(dict, b"Title", &opts.title);
    apply_field(dict, b"Author", &opts.author);
    apply_field(dict, b"Subject", &opts.subject);
    apply_field(dict, b"Keywords", &opts.keywords);
    apply_field(dict, b"Creator", &opts.creator);

    Ok(doc)
}

fn apply_field(dict: &mut lopdf::Dictionary, key: &[u8], value: &Option<String>) {
    match value {
        None => { /* leave unchanged */ }
        Some(v) if v.is_empty() => {
            dict.remove(key);
        }
        Some(v) => {
            dict.set(key, Object::string_literal(v.as_str()));
        }
    }
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    let emit_and_return = |msg: String| -> Result<PathBuf> {
        emit_error(&app, &op_id, &msg);
        Err(AppError::Pdf(msg))
    };

    if req.input_paths.is_empty() {
        return emit_and_return("Edit Metadata requires at least one input file".into());
    }

    let input_path = &req.input_paths[0];
    validate_pdf(input_path, "edit")?;

    emit_progress(&app, &op_id, 10, "Loading PDF\u{2026}");
    let doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    let opts: MetadataOptions =
        serde_json::from_value(req.options.clone()).unwrap_or_default();

    emit_progress(&app, &op_id, 50, "Applying metadata\u{2026}");
    let mut doc = apply_metadata(doc, &opts)?;

    // Save next to input file
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let out_path = out_dir.join(format!("{stem}_metadata.pdf"));

    emit_progress(&app, &op_id, 80, "Writing output\u{2026}");
    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Dictionary, Object, Stream};

    fn make_doc() -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = Stream::new(Dictionary::new(), b"BT /F1 12 Tf (test) Tj ET".to_vec());
        let content_id = doc.add_object(content);
        let page = dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
            "Contents" => Object::Reference(content_id),
        };
        let page_id = doc.add_object(page);
        let pages = dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => Object::Array(vec![Object::Reference(page_id)]),
            "Count" => Object::Integer(1),
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

    fn make_doc_with_info(title: &str) -> Document {
        let mut doc = make_doc();
        let info = dictionary! {
            "Title" => Object::string_literal(title),
            "Author" => Object::string_literal("Original Author"),
        };
        let info_id = doc.add_object(Object::Dictionary(info));
        doc.trailer.set("Info", Object::Reference(info_id));
        doc
    }

    #[test]
    fn apply_metadata_sets_title() {
        let doc = make_doc();
        let opts = MetadataOptions {
            title: Some("My Title".into()),
            ..Default::default()
        };
        let doc = apply_metadata(doc, &opts).unwrap();
        let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let dict = doc.get_dictionary(info_id).unwrap();
        let title = dict.get(b"Title").unwrap().as_str().unwrap();
        assert_eq!(std::str::from_utf8(title).unwrap(), "My Title");
    }

    #[test]
    fn apply_metadata_updates_existing_title() {
        let doc = make_doc_with_info("Old Title");
        let opts = MetadataOptions {
            title: Some("New Title".into()),
            ..Default::default()
        };
        let doc = apply_metadata(doc, &opts).unwrap();
        let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let dict = doc.get_dictionary(info_id).unwrap();
        let title = dict.get(b"Title").unwrap().as_str().unwrap();
        assert_eq!(std::str::from_utf8(title).unwrap(), "New Title");
    }

    #[test]
    fn apply_metadata_none_fields_leave_existing_values() {
        let doc = make_doc_with_info("Keep Me");
        let opts = MetadataOptions::default(); // all None
        let doc = apply_metadata(doc, &opts).unwrap();
        let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let dict = doc.get_dictionary(info_id).unwrap();
        let title = dict.get(b"Title").unwrap().as_str().unwrap();
        assert_eq!(std::str::from_utf8(title).unwrap(), "Keep Me");
        let author = dict.get(b"Author").unwrap().as_str().unwrap();
        assert_eq!(std::str::from_utf8(author).unwrap(), "Original Author");
    }

    #[test]
    fn apply_metadata_empty_string_clears_field() {
        let doc = make_doc_with_info("Remove Me");
        let opts = MetadataOptions {
            title: Some(String::new()), // empty string = remove
            ..Default::default()
        };
        let doc = apply_metadata(doc, &opts).unwrap();
        let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let dict = doc.get_dictionary(info_id).unwrap();
        assert!(dict.get(b"Title").is_err(), "Title should be removed");
    }
}
