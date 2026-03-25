use std::io::Read;
use std::path::{Path, PathBuf};

use printpdf::{BuiltinFont, Mm, PdfDocument};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tauri::AppHandle;
use zip::ZipArchive;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_progress};
use crate::tools::ProcessRequest;

/// Font size in pt.
const FONT_SIZE: f32 = 12.0;
/// Line height in mm.
const LINE_HEIGHT: f32 = 6.0;
/// Top margin in mm.
const TOP_Y: f32 = 195.0;
/// Bottom margin in mm.
const BOTTOM_Y: f32 = 15.0;
/// Left margin in mm.
const LEFT_X: f32 = 15.0;

/// Derive an output file stem from the input path.
fn output_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string()
}

/// Extract all text from `<a:t>` elements in a slide XML fragment.
pub fn extract_slide_text(xml_bytes: &[u8]) -> Vec<String> {
    let mut reader = Reader::from_reader(xml_bytes);
    let mut buf = Vec::new();
    let mut texts = Vec::new();
    let mut inside_at = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"t" {
                    inside_at = true;
                }
            }
            Ok(Event::Text(ref e)) if inside_at => {
                if let Ok(text) = e.unescape() {
                    texts.push(text.into_owned());
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"t" {
                    inside_at = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    texts
}

/// Collect sorted slide entry names from the ZIP archive.
fn collect_slide_names<R: Read + std::io::Seek>(archive: &ZipArchive<R>) -> Vec<String> {
    let mut slide_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let name = archive.name_for_index(i)?.to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    // Sort by slide number (slide1.xml, slide2.xml, ...)
    slide_names.sort_by(|a, b| {
        let num_a = extract_slide_number(a);
        let num_b = extract_slide_number(b);
        num_a.cmp(&num_b)
    });

    slide_names
}

/// Extract the numeric part from a slide filename like "ppt/slides/slide3.xml".
fn extract_slide_number(name: &str) -> usize {
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse::<usize>()
        .unwrap_or(0)
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let input_path = req
        .input_paths
        .first()
        .ok_or_else(|| AppError::Validation("No input file".into()))?;

    let ext = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "pptx" {
        return Err(AppError::Validation(format!(
            "Unsupported presentation format: .{ext} (only .pptx supported)"
        )));
    }

    emit_progress(&app, &op_id, 5, "Opening presentation...");

    let file = std::fs::File::open(input_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| AppError::Pdf(format!("Failed to open PPTX as ZIP: {e}")))?;

    let slide_names = collect_slide_names(&archive);
    if slide_names.is_empty() {
        return Err(AppError::Validation(
            "No slides found in presentation".into(),
        ));
    }

    let total_slides = slide_names.len();

    // Create PDF — landscape A4
    let (doc, first_page, first_layer) =
        PdfDocument::new("PowerPoint to PDF", Mm(297.0), Mm(210.0), "Slide 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(format!("Failed to add font: {e}")))?;

    for (idx, slide_name) in slide_names.iter().enumerate() {
        let percent = 10 + (idx * 80) / total_slides.max(1);
        emit_progress(
            &app,
            &op_id,
            percent as u8,
            &format!("Processing slide {}...", idx + 1),
        );

        let xml_data = {
            let mut entry = archive
                .by_name(slide_name)
                .map_err(|e| AppError::Pdf(format!("Failed to read {slide_name}: {e}")))?;
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| AppError::Pdf(format!("Failed to read slide data: {e}")))?;
            buf
        };

        let texts = extract_slide_text(&xml_data);
        let lines: Vec<String> = texts
            .iter()
            .flat_map(|t| t.lines().map(String::from))
            .filter(|l| !l.trim().is_empty())
            .collect();

        if idx == 0 {
            // Use the already-created first page
            let layer_ref = doc.get_page(first_page).get_layer(first_layer);
            let mut y = TOP_Y;
            for line in &lines {
                if y < BOTTOM_Y {
                    break;
                }
                layer_ref.use_text(line, FONT_SIZE, Mm(LEFT_X), Mm(y), &font);
                y -= LINE_HEIGHT;
            }
        } else {
            let label = format!("Slide {}", idx + 1);
            let (page, layer) = doc.add_page(Mm(297.0), Mm(210.0), &label);
            let layer_ref = doc.get_page(page).get_layer(layer);
            let mut y = TOP_Y;
            for line in &lines {
                if y < BOTTOM_Y {
                    break;
                }
                layer_ref.use_text(line, FONT_SIZE, Mm(LEFT_X), Mm(y), &font);
                y -= LINE_HEIGHT;
            }
        }
    }

    emit_progress(&app, &op_id, 90, "Saving PDF...");

    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };
    let out_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Validation("Cannot determine output directory".into()))?;
    let out_path = out_dir.join(format!("{stem}.pdf"));

    let pdf_bytes = doc
        .save_to_bytes()
        .map_err(|e| AppError::Pdf(format!("Failed to save PDF: {e}")))?;
    std::fs::write(&out_path, pdf_bytes)?;

    emit_complete(&app, &op_id);
    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_ppt() {
        assert_eq!(output_stem(&PathBuf::from("/tmp/deck.pptx")), "deck");
    }

    #[test]
    fn extract_text_from_xml() {
        let xml = r#"<p:sp><a:t>Hello</a:t><a:t> World</a:t></p:sp>"#;
        let texts = extract_slide_text(xml.as_bytes());
        assert_eq!(texts.join(""), "Hello World");
    }

    #[test]
    fn extract_text_empty_xml() {
        let xml = r#"<p:sp></p:sp>"#;
        let texts = extract_slide_text(xml.as_bytes());
        assert!(texts.is_empty());
    }

    #[test]
    fn extract_slide_number_works() {
        assert_eq!(extract_slide_number("ppt/slides/slide1.xml"), 1);
        assert_eq!(extract_slide_number("ppt/slides/slide12.xml"), 12);
    }
}
