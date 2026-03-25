use std::path::{Path, PathBuf};

use lopdf::{dictionary, Document, Object, Stream};
use serde_json::Value;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_error, emit_progress};
use crate::tools::ProcessRequest;

/// Derive the output filename stem for PDF/A conversion.
/// E.g. `/tmp/report.pdf` → `"report_pdfa"`.
pub fn output_stem(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    format!("{stem}_pdfa")
}

/// Parse the conformance level from options. Defaults to "1b".
pub fn parse_conformance(opts: &Value) -> &str {
    opts.get("conformance")
        .and_then(|v| v.as_str())
        .unwrap_or("1b")
}

/// Build XMP metadata XML with PDF/A identification.
pub fn build_xmp_metadata(title: &str, creator: &str, conformance: &str) -> String {
    let part = conformance.chars().next().unwrap_or('1');
    let level = conformance.chars().nth(1).unwrap_or('b').to_ascii_uppercase();
    format!(
        r#"<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
      xmlns:dc="http://purl.org/dc/elements/1.1/"
      xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
      <dc:title><rdf:Alt><rdf:li xml:lang="x-default">{title}</rdf:li></rdf:Alt></dc:title>
      <dc:creator><rdf:Seq><rdf:li>{creator}</rdf:li></rdf:Seq></dc:creator>
      <pdfaid:part>{part}</pdfaid:part>
      <pdfaid:conformance>{level}</pdfaid:conformance>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#
    )
}

/// Add an sRGB OutputIntent to the document catalog if not already present.
pub fn add_output_intent(doc: &mut Document) -> Result<()> {
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|obj| obj.as_reference().ok())
        .ok_or_else(|| AppError::Pdf("Cannot find document catalog reference".into()))?;

    // Check if OutputIntents already exist
    let has_output_intents = doc
        .get_object(catalog_id)
        .ok()
        .and_then(|obj| obj.as_dict().ok())
        .and_then(|dict| dict.get(b"OutputIntents").ok())
        .is_some();

    if has_output_intents {
        return Ok(());
    }

    let output_intent = dictionary! {
        "Type" => Object::Name(b"OutputIntent".to_vec()),
        "S" => Object::Name(b"GTS_PDFA1".to_vec()),
        "OutputConditionIdentifier" => Object::string_literal("sRGB IEC61966-2.1"),
        "RegistryName" => Object::string_literal("http://www.color.org"),
        "Info" => Object::string_literal("sRGB IEC61966-2.1"),
    };

    let intent_id = doc.add_object(Object::Dictionary(output_intent));
    let intents_array = Object::Array(vec![Object::Reference(intent_id)]);

    // Get mutable catalog and add OutputIntents
    let catalog = doc
        .get_object_mut(catalog_id)
        .map_err(|e| AppError::Pdf(format!("Cannot access catalog: {e}")))?;

    if let Object::Dictionary(ref mut dict) = catalog {
        dict.set("OutputIntents", intents_array);
    } else {
        return Err(AppError::Pdf("Catalog is not a dictionary".into()));
    }

    Ok(())
}

/// Add XMP metadata stream to the document catalog.
fn add_xmp_metadata(doc: &mut Document, xmp: &str) -> Result<()> {
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|obj| obj.as_reference().ok())
        .ok_or_else(|| AppError::Pdf("Cannot find document catalog reference".into()))?;

    let xmp_bytes = xmp.as_bytes().to_vec();
    let xmp_stream = Stream::new(
        dictionary! {
            "Type" => Object::Name(b"Metadata".to_vec()),
            "Subtype" => Object::Name(b"XML".to_vec()),
            "Length" => Object::Integer(xmp_bytes.len() as i64),
        },
        xmp_bytes,
    );

    let metadata_id = doc.add_object(Object::Stream(xmp_stream));

    let catalog = doc
        .get_object_mut(catalog_id)
        .map_err(|e| AppError::Pdf(format!("Cannot access catalog: {e}")))?;

    if let Object::Dictionary(ref mut dict) = catalog {
        dict.set("Metadata", Object::Reference(metadata_id));
    } else {
        return Err(AppError::Pdf("Catalog is not a dictionary".into()));
    }

    Ok(())
}

/// Remove transparency groups (Group entries with /S /Transparency) from page dictionaries.
/// PDF/A-1 forbids transparency.
fn remove_transparency_groups(doc: &mut Document) {
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().values().copied().collect();

    for page_id in page_ids {
        if let Ok(Object::Dictionary(ref mut dict)) = doc.get_object_mut(page_id) {
            if let Ok(group) = dict.get(b"Group") {
                let is_transparency = match group {
                    Object::Dictionary(ref g) => g
                        .get(b"S")
                        .ok()
                        .and_then(|s| {
                            if let Object::Name(name) = s {
                                Some(name.as_slice() == b"Transparency")
                            } else {
                                None
                            }
                        })
                        .unwrap_or(false),
                    _ => false,
                };
                if is_transparency {
                    dict.remove(b"Group");
                }
            }
        }
    }
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let emit_and_return = |err: AppError| {
        emit_error(&app, &op_id, &err.to_string());
        err
    };

    if req.input_paths.len() != 1 {
        let msg = "PDF to PDF/A requires exactly one input file".to_string();
        emit_error(&app, &op_id, &msg);
        return Err(AppError::Validation(msg));
    }

    let input_path = &req.input_paths[0];

    crate::pipeline::validate::validate_pdf(input_path, "pdf_to_pdfa")
        .map_err(|err| emit_and_return(err))?;

    emit_progress(&app, &op_id, 5, "Loading document\u{2026}");

    let mut doc = Document::load(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to load {:?}: {e}", input_path)))
        .map_err(|err| emit_and_return(err))?;

    let conformance = parse_conformance(&req.options);

    // Step 1: Set PDF version to 1.4 (PDF/A-1 requirement)
    emit_progress(&app, &op_id, 20, "Setting PDF version to 1.4\u{2026}");
    doc.version = "1.4".to_string();

    // Step 2: Add XMP metadata with PDF/A identification
    emit_progress(&app, &op_id, 40, "Adding PDF/A metadata\u{2026}");
    let title = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled");
    let xmp = build_xmp_metadata(title, "PavoPDF", conformance);
    add_xmp_metadata(&mut doc, &xmp).map_err(|err| emit_and_return(err))?;

    // Step 3: Add sRGB OutputIntent
    emit_progress(&app, &op_id, 60, "Adding output intent\u{2026}");
    add_output_intent(&mut doc).map_err(|err| emit_and_return(err))?;

    // Step 4: Remove transparency groups
    emit_progress(&app, &op_id, 75, "Removing transparency groups\u{2026}");
    remove_transparency_groups(&mut doc);

    // Step 5: Save output next to input file
    emit_progress(&app, &op_id, 85, "Saving PDF/A document\u{2026}");

    let out_dir = input_path
        .parent()
        .ok_or_else(|| {
            let msg = "Cannot determine output directory from input path".to_string();
            emit_error(&app, &op_id, &msg);
            AppError::Validation(msg)
        })?;

    let stem = if req.output_stem.trim().is_empty() {
        output_stem(input_path)
    } else {
        let trimmed = req.output_stem.trim();
        if trimmed.to_ascii_lowercase().ends_with(".pdf") {
            trimmed[..trimmed.len() - 4].to_string()
        } else {
            trimmed.to_string()
        }
    };

    let out_path = out_dir.join(format!("{stem}.pdf"));

    doc.save(&out_path)
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF/A {:?}: {e}", out_path)))
        .map_err(|err| emit_and_return(err))?;

    emit_complete(&app, &op_id);

    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_pdfa() {
        let p = std::path::PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report_pdfa");
    }

    #[test]
    fn xmp_metadata_contains_pdfa_identification() {
        let xmp = build_xmp_metadata("Test Doc", "PavoPDF", "1b");
        assert!(xmp.contains("pdfaid:part"));
        assert!(xmp.contains("pdfaid:conformance"));
    }

    #[test]
    fn parse_conformance_level_defaults_to_1b() {
        let opts = serde_json::json!({});
        assert_eq!(parse_conformance(&opts), "1b");
    }

    #[test]
    fn parse_conformance_level_reads_option() {
        let opts = serde_json::json!({"conformance": "2b"});
        assert_eq!(parse_conformance(&opts), "2b");
    }
}
