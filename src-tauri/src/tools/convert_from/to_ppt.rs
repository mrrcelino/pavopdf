use std::io::{Cursor, Write as _};
use std::path::{Path, PathBuf};

use image::ImageFormat;
use tauri::AppHandle;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_progress};
use crate::pipeline::validate::validate_pdf;
use crate::tools::ProcessRequest;

use super::pdfium_helper::{load_pdfium, open_pdf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the file stem from an input path (e.g. `"report"` from `"report.pdf"`).
fn output_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_owned()
}

/// Convert PDF points to EMU (English Metric Units). 1 point = 12700 EMU.
fn emu_from_points(points: f32) -> i64 {
    (points as f64 * 12700.0).round() as i64
}

/// Return slide dimensions in EMU for the given page size in PDF points.
fn slide_dimensions_emu(width_pts: f32, height_pts: f32) -> (i64, i64) {
    (emu_from_points(width_pts), emu_from_points(height_pts))
}

// ---------------------------------------------------------------------------
// Per-page data
// ---------------------------------------------------------------------------

struct PageImage {
    png_data: Vec<u8>,
    width_pts: f32,
    height_pts: f32,
}

// ---------------------------------------------------------------------------
// PPTX XML templates
// ---------------------------------------------------------------------------

fn content_types_xml(page_count: usize) -> String {
    let mut slide_overrides = String::new();
    for i in 1..=page_count {
        slide_overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    let mut image_defaults = String::new();
    // PNG default
    image_defaults.push_str(r#"<Default Extension="png" ContentType="image/png"/>"#);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
{image_defaults}
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
{slide_overrides}
</Types>"#
    )
}

fn root_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#
}

fn presentation_xml(page_count: usize, cx: i64, cy: i64) -> String {
    let mut slide_list = String::new();
    for i in 1..=page_count {
        slide_list.push_str(&format!(
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            255 + i,
            i + 2 // rId1 = slideMaster, rId2 = slideLayout, rId3+ = slides
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:sldMasterIdLst>
<p:sldMasterId id="2147483648" r:id="rId1"/>
</p:sldMasterIdLst>
<p:sldIdLst>
{slide_list}
</p:sldIdLst>
<p:sldSz cx="{cx}" cy="{cy}"/>
<p:notesSz cx="{cy}" cy="{cx}"/>
</p:presentation>"#
    )
}

fn presentation_rels_xml(page_count: usize) -> String {
    let mut rels = String::new();
    rels.push_str(r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#);
    rels.push_str(r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="slideLayouts/slideLayout1.xml"/>"#);
    for i in 1..=page_count {
        rels.push_str(&format!(
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#,
            i + 2
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{rels}
</Relationships>"#
    )
}

fn slide_master_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
</p:sldMaster>"#
}

fn slide_master_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#
}

fn slide_layout_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
</p:sldLayout>"#
}

fn slide_layout_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#
}

fn slide_xml(cx: i64, cy: i64) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr/>
<p:pic>
<p:nvPicPr><p:cNvPr id="2" name="Page Image"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
<p:blipFill>
<a:blip r:embed="rId2"/>
<a:stretch><a:fillRect/></a:stretch>
</p:blipFill>
<p:spPr>
<a:xfrm>
<a:off x="0" y="0"/>
<a:ext cx="{cx}" cy="{cy}"/>
</a:xfrm>
<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
</p:spPr>
</p:pic>
</p:spTree>
</p:cSld>
</p:sld>"#
    )
}

fn slide_rels_xml(slide_idx: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{slide_idx}.png"/>
</Relationships>"#
    )
}

// ---------------------------------------------------------------------------
// PPTX builder
// ---------------------------------------------------------------------------

/// Build a .pptx file from rendered page images.
fn write_pptx(pages: &[PageImage], out_path: &Path) -> Result<()> {
    if pages.is_empty() {
        return Err(AppError::Validation("No pages to write".into()));
    }

    let file = std::fs::File::create(out_path)?;
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Use the first page's dimensions for the presentation-level slide size
    let (pres_cx, pres_cy) = slide_dimensions_emu(pages[0].width_pts, pages[0].height_pts);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(content_types_xml(pages.len()).as_bytes())?;

    // _rels/.rels
    zip.start_file("_rels/.rels", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(root_rels_xml().as_bytes())?;

    // ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(presentation_xml(pages.len(), pres_cx, pres_cy).as_bytes())?;

    // ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(presentation_rels_xml(pages.len()).as_bytes())?;

    // ppt/slideMasters/slideMaster1.xml
    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(slide_master_xml().as_bytes())?;

    // ppt/slideMasters/_rels/slideMaster1.xml.rels
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(slide_master_rels_xml().as_bytes())?;

    // ppt/slideLayouts/slideLayout1.xml
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(slide_layout_xml().as_bytes())?;

    // ppt/slideLayouts/_rels/slideLayout1.xml.rels
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts)
        .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
    zip.write_all(slide_layout_rels_xml().as_bytes())?;

    // Per-slide files
    for (i, page) in pages.iter().enumerate() {
        let slide_num = i + 1;
        let (cx, cy) = slide_dimensions_emu(page.width_pts, page.height_pts);

        // ppt/slides/slide{N}.xml
        zip.start_file(format!("ppt/slides/slide{slide_num}.xml"), opts)
            .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
        zip.write_all(slide_xml(cx, cy).as_bytes())?;

        // ppt/slides/_rels/slide{N}.xml.rels
        zip.start_file(format!("ppt/slides/_rels/slide{slide_num}.xml.rels"), opts)
            .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
        zip.write_all(slide_rels_xml(slide_num).as_bytes())?;

        // ppt/media/image{N}.png
        zip.start_file(format!("ppt/media/image{slide_num}.png"), opts)
            .map_err(|e| AppError::Pdf(format!("ZIP error: {e}")))?;
        zip.write_all(&page.png_data)?;
    }

    zip.finish()
        .map_err(|e| AppError::Pdf(format!("Failed to finalise PPTX: {e}")))?;

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

    validate_pdf(input_path, "pdf_to_ppt")?;
    emit_progress(&app, &op_id, 5, "Loading PDF...");

    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };

    // --- Load Pdfium & open document -----------------------------------------
    let pdfium = load_pdfium()?;
    let document = open_pdf(&pdfium, input_path, None)?;

    let total_pages = document.pages().len() as usize;
    if total_pages == 0 {
        return Err(AppError::Pdf("PDF contains no pages".into()));
    }

    emit_progress(&app, &op_id, 10, "Rendering pages...");

    // --- Render each page to PNG ---------------------------------------------
    let mut pages: Vec<PageImage> = Vec::with_capacity(total_pages);

    for page_idx in 0..total_pages {
        let page = document
            .pages()
            .get(page_idx as u16)
            .map_err(|e| AppError::Pdf(format!("Failed to get page {}: {e}", page_idx + 1)))?;

        let width_pts = page.width().value;
        let height_pts = page.height().value;

        // Render at 150 DPI
        let width_px = (width_pts * 150.0 / 72.0) as i32;
        let height_px = (height_pts * 150.0 / 72.0) as i32;

        let config = pdfium_render::prelude::PdfRenderConfig::new()
            .set_target_size(width_px, height_px);

        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| AppError::Pdf(format!("Failed to render page {}: {e}", page_idx + 1)))?;

        let img = bitmap.as_image();

        // Encode to PNG in memory
        let mut png_buf = Cursor::new(Vec::new());
        img.write_to(&mut png_buf, ImageFormat::Png)
            .map_err(|e| AppError::Pdf(format!("Failed to encode page {} as PNG: {e}", page_idx + 1)))?;

        pages.push(PageImage {
            png_data: png_buf.into_inner(),
            width_pts,
            height_pts,
        });

        // Progress: 10% -> 80% across pages
        let percent = 10 + (((page_idx + 1) * 70 / total_pages.max(1)) as u8).min(70);
        emit_progress(
            &app,
            &op_id,
            percent,
            &format!("Rendered page {} of {}", page_idx + 1, total_pages),
        );
    }

    // --- Build PPTX ----------------------------------------------------------
    emit_progress(&app, &op_id, 85, "Building PowerPoint file...");

    let output_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Pdf("Cannot determine output directory".into()))?;
    let out_path = output_dir.join(format!("{stem}.pptx"));

    write_pptx(&pages, &out_path)?;

    emit_progress(&app, &op_id, 100, "Done");
    emit_complete(&app, &op_id);

    Ok(out_path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_ppt() {
        let p = std::path::PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report");
    }

    #[test]
    fn output_stem_no_extension() {
        let p = std::path::PathBuf::from("/tmp/document");
        assert_eq!(output_stem(&p), "document");
    }

    #[test]
    fn output_stem_fallback() {
        let p = std::path::PathBuf::from("/");
        assert_eq!(output_stem(&p), "output");
    }

    #[test]
    fn emu_from_points_converts_correctly() {
        // 1 point = 12700 EMU
        assert_eq!(emu_from_points(72.0), 914400); // 1 inch
        assert_eq!(emu_from_points(1.0), 12700);
    }

    #[test]
    fn emu_from_points_zero() {
        assert_eq!(emu_from_points(0.0), 0);
    }

    #[test]
    fn slide_dimensions_standard() {
        let (cx, cy) = slide_dimensions_emu(612.0, 792.0); // US Letter
        assert!(cx > 0);
        assert!(cy > 0);
        assert_eq!(cx, emu_from_points(612.0));
        assert_eq!(cy, emu_from_points(792.0));
    }

    #[test]
    fn slide_dimensions_a4() {
        let (cx, cy) = slide_dimensions_emu(595.0, 842.0); // A4
        assert!(cx > 0);
        assert!(cy > 0);
    }

    #[test]
    fn content_types_has_slide_entries() {
        let xml = content_types_xml(3);
        assert!(xml.contains("slide1.xml"));
        assert!(xml.contains("slide2.xml"));
        assert!(xml.contains("slide3.xml"));
        assert!(xml.contains("image/png"));
    }

    #[test]
    fn presentation_xml_has_slide_ids() {
        let xml = presentation_xml(2, 914400, 914400);
        assert!(xml.contains("rId3")); // first slide
        assert!(xml.contains("rId4")); // second slide
    }

    #[test]
    fn slide_xml_contains_dimensions() {
        let xml = slide_xml(914400, 1219200);
        assert!(xml.contains("914400"));
        assert!(xml.contains("1219200"));
        assert!(xml.contains("r:embed=\"rId2\""));
    }

    #[test]
    fn slide_rels_references_correct_image() {
        let xml = slide_rels_xml(3);
        assert!(xml.contains("image3.png"));
    }

    #[test]
    fn write_pptx_empty_pages_errors() {
        let dir = std::env::temp_dir();
        let out = dir.join("test_empty.pptx");
        let result = write_pptx(&[], &out);
        assert!(result.is_err());
    }

    #[test]
    fn write_pptx_creates_valid_zip() {
        // Create a minimal 1x1 PNG in memory
        let mut png_buf = Cursor::new(Vec::new());
        let img = image::DynamicImage::new_rgb8(1, 1);
        img.write_to(&mut png_buf, ImageFormat::Png).unwrap();

        let pages = vec![PageImage {
            png_data: png_buf.into_inner(),
            width_pts: 612.0,
            height_pts: 792.0,
        }];

        let dir = std::env::temp_dir();
        let out = dir.join("test_single_slide.pptx");
        write_pptx(&pages, &out).unwrap();

        // Verify it is a valid ZIP with expected entries
        let file = std::fs::File::open(&out).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_owned())
            .collect();

        assert!(names.contains(&"[Content_Types].xml".to_owned()));
        assert!(names.contains(&"ppt/presentation.xml".to_owned()));
        assert!(names.contains(&"ppt/slides/slide1.xml".to_owned()));
        assert!(names.contains(&"ppt/media/image1.png".to_owned()));
        assert!(names.contains(&"ppt/slides/_rels/slide1.xml.rels".to_owned()));

        // Clean up
        let _ = std::fs::remove_file(&out);
    }
}
