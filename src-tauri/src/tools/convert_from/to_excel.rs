use std::io::{Cursor, Write as IoWrite};
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
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

/// Derive the output stem from the input filename.
fn output_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_owned()
}

/// Split a single line of text into cells.
///
/// Tabs are the primary delimiter. If no tabs are present, sequences of 2+
/// spaces are used. If neither is found, the whole line is returned as a
/// single cell.
fn split_line_into_cells(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Prefer tab-separated
    if trimmed.contains('\t') {
        return trimmed.split('\t').map(|s| s.trim()).collect();
    }

    // Try splitting on 2+ spaces
    let parts: Vec<&str> = trimmed
        .split("  ")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.len() > 1 {
        return parts;
    }

    // Single cell
    vec![trimmed]
}

/// Convert a 0-based column index to an Excel column letter (A, B, ..., Z, AA, AB, ...).
fn col_letter(index: usize) -> String {
    let mut result = String::new();
    let mut n = index;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}

/// Escape characters that are invalid in XML text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// XLSX writer
// ---------------------------------------------------------------------------

/// Build a minimal .xlsx file from sheet data.
///
/// Each element of `sheets` is `(sheet_name, rows)` where each row is a `Vec<String>` of cell values.
/// Map a `zip::result::ZipError` into our `AppError`.
fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::Pdf(format!("ZIP write error: {e}"))
}

fn write_xlsx(sheets: &[(String, Vec<Vec<String>>)], path: &Path) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", opts).map_err(zip_err)?;
    zip.write_all(content_types_xml(sheets.len()).as_bytes())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", opts).map_err(zip_err)?;
    zip.write_all(rels_xml().as_bytes())?;

    // 3. xl/workbook.xml
    zip.start_file("xl/workbook.xml", opts).map_err(zip_err)?;
    zip.write_all(workbook_xml(sheets).as_bytes())?;

    // 4. xl/_rels/workbook.xml.rels
    zip.start_file("xl/_rels/workbook.xml.rels", opts).map_err(zip_err)?;
    zip.write_all(workbook_rels_xml(sheets.len()).as_bytes())?;

    // 5. xl/styles.xml (minimal)
    zip.start_file("xl/styles.xml", opts).map_err(zip_err)?;
    zip.write_all(styles_xml().as_bytes())?;

    // 6. xl/worksheets/sheet{n}.xml
    for (i, (_name, rows)) in sheets.iter().enumerate() {
        let filename = format!("xl/worksheets/sheet{}.xml", i + 1);
        zip.start_file(&filename, opts).map_err(zip_err)?;
        zip.write_all(sheet_xml(rows)?.as_bytes())?;
    }

    zip.finish().map_err(zip_err)?;
    Ok(())
}

fn content_types_xml(sheet_count: usize) -> String {
    let mut buf = Cursor::new(Vec::new());
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))
        .unwrap();

    let mut types = BytesStart::new("Types");
    types.push_attribute((
        "xmlns",
        "http://schemas.openxmlformats.org/package/2006/content-types",
    ));
    w.write_event(Event::Start(types)).unwrap();

    // rels
    let mut def = BytesStart::new("Default");
    def.push_attribute(("Extension", "rels"));
    def.push_attribute((
        "ContentType",
        "application/vnd.openxmlformats-package.relationships+xml",
    ));
    w.write_event(Event::Empty(def)).unwrap();

    // xml
    let mut def = BytesStart::new("Default");
    def.push_attribute(("Extension", "xml"));
    def.push_attribute(("ContentType", "application/xml"));
    w.write_event(Event::Empty(def)).unwrap();

    // workbook
    let mut ovr = BytesStart::new("Override");
    ovr.push_attribute(("PartName", "/xl/workbook.xml"));
    ovr.push_attribute((
        "ContentType",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
    ));
    w.write_event(Event::Empty(ovr)).unwrap();

    // styles
    let mut ovr = BytesStart::new("Override");
    ovr.push_attribute(("PartName", "/xl/styles.xml"));
    ovr.push_attribute((
        "ContentType",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
    ));
    w.write_event(Event::Empty(ovr)).unwrap();

    // sheets
    for i in 0..sheet_count {
        let mut ovr = BytesStart::new("Override");
        let part = format!("/xl/worksheets/sheet{}.xml", i + 1);
        ovr.push_attribute(("PartName", part.as_str()));
        ovr.push_attribute((
            "ContentType",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        ));
        w.write_event(Event::Empty(ovr)).unwrap();
    }

    w.write_event(Event::End(BytesEnd::new("Types"))).unwrap();
    String::from_utf8(buf.into_inner()).unwrap()
}

fn rels_xml() -> String {
    let mut buf = Cursor::new(Vec::new());
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))
        .unwrap();

    let mut root = BytesStart::new("Relationships");
    root.push_attribute((
        "xmlns",
        "http://schemas.openxmlformats.org/package/2006/relationships",
    ));
    w.write_event(Event::Start(root)).unwrap();

    let mut rel = BytesStart::new("Relationship");
    rel.push_attribute(("Id", "rId1"));
    rel.push_attribute((
        "Type",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
    ));
    rel.push_attribute(("Target", "xl/workbook.xml"));
    w.write_event(Event::Empty(rel)).unwrap();

    w.write_event(Event::End(BytesEnd::new("Relationships")))
        .unwrap();
    String::from_utf8(buf.into_inner()).unwrap()
}

fn workbook_xml(sheets: &[(String, Vec<Vec<String>>)]) -> String {
    let mut buf = Cursor::new(Vec::new());
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))
        .unwrap();

    let mut root = BytesStart::new("workbook");
    root.push_attribute((
        "xmlns",
        "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
    ));
    root.push_attribute((
        "xmlns:r",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
    ));
    w.write_event(Event::Start(root)).unwrap();

    w.write_event(Event::Start(BytesStart::new("sheets")))
        .unwrap();

    for (i, (name, _)) in sheets.iter().enumerate() {
        let mut sheet = BytesStart::new("sheet");
        sheet.push_attribute(("name", xml_escape(name).as_str()));
        sheet.push_attribute(("sheetId", (i + 1).to_string().as_str()));
        let rid = format!("rId{}", i + 1);
        sheet.push_attribute(("r:id", rid.as_str()));
        w.write_event(Event::Empty(sheet)).unwrap();
    }

    w.write_event(Event::End(BytesEnd::new("sheets"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("workbook")))
        .unwrap();
    String::from_utf8(buf.into_inner()).unwrap()
}

fn workbook_rels_xml(sheet_count: usize) -> String {
    let mut buf = Cursor::new(Vec::new());
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))
        .unwrap();

    let mut root = BytesStart::new("Relationships");
    root.push_attribute((
        "xmlns",
        "http://schemas.openxmlformats.org/package/2006/relationships",
    ));
    w.write_event(Event::Start(root)).unwrap();

    for i in 0..sheet_count {
        let mut rel = BytesStart::new("Relationship");
        let id = format!("rId{}", i + 1);
        rel.push_attribute(("Id", id.as_str()));
        rel.push_attribute((
            "Type",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
        ));
        let target = format!("worksheets/sheet{}.xml", i + 1);
        rel.push_attribute(("Target", target.as_str()));
        w.write_event(Event::Empty(rel)).unwrap();
    }

    // styles relationship
    let mut rel = BytesStart::new("Relationship");
    let id = format!("rId{}", sheet_count + 1);
    rel.push_attribute(("Id", id.as_str()));
    rel.push_attribute((
        "Type",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles",
    ));
    rel.push_attribute(("Target", "styles.xml"));
    w.write_event(Event::Empty(rel)).unwrap();

    w.write_event(Event::End(BytesEnd::new("Relationships")))
        .unwrap();
    String::from_utf8(buf.into_inner()).unwrap()
}

fn styles_xml() -> String {
    let mut buf = Cursor::new(Vec::new());
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))
        .unwrap();

    let mut root = BytesStart::new("styleSheet");
    root.push_attribute((
        "xmlns",
        "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
    ));
    w.write_event(Event::Start(root)).unwrap();

    // Minimal fonts
    let mut fonts = BytesStart::new("fonts");
    fonts.push_attribute(("count", "1"));
    w.write_event(Event::Start(fonts)).unwrap();
    w.write_event(Event::Start(BytesStart::new("font"))).unwrap();
    let mut sz = BytesStart::new("sz");
    sz.push_attribute(("val", "11"));
    w.write_event(Event::Empty(sz)).unwrap();
    w.write_event(Event::End(BytesEnd::new("font"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("fonts"))).unwrap();

    // Minimal fills
    let mut fills = BytesStart::new("fills");
    fills.push_attribute(("count", "1"));
    w.write_event(Event::Start(fills)).unwrap();
    w.write_event(Event::Start(BytesStart::new("fill"))).unwrap();
    let mut pat = BytesStart::new("patternFill");
    pat.push_attribute(("patternType", "none"));
    w.write_event(Event::Empty(pat)).unwrap();
    w.write_event(Event::End(BytesEnd::new("fill"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("fills"))).unwrap();

    // Minimal borders
    let mut borders = BytesStart::new("borders");
    borders.push_attribute(("count", "1"));
    w.write_event(Event::Start(borders)).unwrap();
    w.write_event(Event::Start(BytesStart::new("border"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("border"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("borders"))).unwrap();

    // Cell style xfs
    let mut csxfs = BytesStart::new("cellStyleXfs");
    csxfs.push_attribute(("count", "1"));
    w.write_event(Event::Start(csxfs)).unwrap();
    let mut xf = BytesStart::new("xf");
    xf.push_attribute(("numFmtId", "0"));
    xf.push_attribute(("fontId", "0"));
    xf.push_attribute(("fillId", "0"));
    xf.push_attribute(("borderId", "0"));
    w.write_event(Event::Empty(xf)).unwrap();
    w.write_event(Event::End(BytesEnd::new("cellStyleXfs"))).unwrap();

    // Cell xfs
    let mut cxfs = BytesStart::new("cellXfs");
    cxfs.push_attribute(("count", "1"));
    w.write_event(Event::Start(cxfs)).unwrap();
    let mut xf = BytesStart::new("xf");
    xf.push_attribute(("numFmtId", "0"));
    xf.push_attribute(("fontId", "0"));
    xf.push_attribute(("fillId", "0"));
    xf.push_attribute(("borderId", "0"));
    xf.push_attribute(("xfId", "0"));
    w.write_event(Event::Empty(xf)).unwrap();
    w.write_event(Event::End(BytesEnd::new("cellXfs"))).unwrap();

    w.write_event(Event::End(BytesEnd::new("styleSheet")))
        .unwrap();
    String::from_utf8(buf.into_inner()).unwrap()
}

fn sheet_xml(rows: &[Vec<String>]) -> Result<String> {
    let mut buf = Cursor::new(Vec::new());
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))))
        .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

    let mut root = BytesStart::new("worksheet");
    root.push_attribute((
        "xmlns",
        "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
    ));
    w.write_event(Event::Start(root))
        .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

    w.write_event(Event::Start(BytesStart::new("sheetData")))
        .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

    for (row_idx, row) in rows.iter().enumerate() {
        let row_num = row_idx + 1;
        let mut row_el = BytesStart::new("row");
        row_el.push_attribute(("r", row_num.to_string().as_str()));
        w.write_event(Event::Start(row_el))
            .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

        for (col_idx, value) in row.iter().enumerate() {
            let cell_ref = format!("{}{}", col_letter(col_idx), row_num);

            // Try to parse as number; fall back to inline string
            if let Ok(_num) = value.parse::<f64>() {
                let mut cell = BytesStart::new("c");
                cell.push_attribute(("r", cell_ref.as_str()));
                w.write_event(Event::Start(cell))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

                w.write_event(Event::Start(BytesStart::new("v")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
                w.write_event(Event::Text(BytesText::new(value)))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
                w.write_event(Event::End(BytesEnd::new("v")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

                w.write_event(Event::End(BytesEnd::new("c")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
            } else {
                // Inline string (t="inlineStr")
                let mut cell = BytesStart::new("c");
                cell.push_attribute(("r", cell_ref.as_str()));
                cell.push_attribute(("t", "inlineStr"));
                w.write_event(Event::Start(cell))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

                w.write_event(Event::Start(BytesStart::new("is")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
                w.write_event(Event::Start(BytesStart::new("t")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
                w.write_event(Event::Text(BytesText::new(&xml_escape(value))))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
                w.write_event(Event::End(BytesEnd::new("t")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
                w.write_event(Event::End(BytesEnd::new("is")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

                w.write_event(Event::End(BytesEnd::new("c")))
                    .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
            }
        }

        w.write_event(Event::End(BytesEnd::new("row")))
            .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
    }

    w.write_event(Event::End(BytesEnd::new("sheetData")))
        .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;
    w.write_event(Event::End(BytesEnd::new("worksheet")))
        .map_err(|e| AppError::Pdf(format!("XML write error: {e}")))?;

    String::from_utf8(buf.into_inner())
        .map_err(|e| AppError::Pdf(format!("UTF-8 encoding error: {e}")))
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();

    // --- Validate input ---
    let input_path = req
        .input_paths
        .first()
        .ok_or_else(|| AppError::Validation("No input file provided".into()))?;

    validate_pdf(input_path, "pdf_to_excel")?;
    emit_progress(&app, &op_id, 10, "Loading PDF...");

    let stem = if req.output_stem.is_empty() {
        output_stem(input_path)
    } else {
        req.output_stem.clone()
    };

    // --- Load Pdfium & open document ---
    let pdfium = load_pdfium()?;
    let document = open_pdf(&pdfium, input_path, None)?;

    let total_pages = document.pages().len() as usize;
    if total_pages == 0 {
        return Err(AppError::Pdf("PDF contains no pages".into()));
    }

    emit_progress(&app, &op_id, 20, "Extracting text...");

    // --- Extract text from each page into sheet data ---
    let mut sheets: Vec<(String, Vec<Vec<String>>)> = Vec::with_capacity(total_pages);

    for page_idx in 0..total_pages {
        let page = document
            .pages()
            .get(page_idx as u16)
            .map_err(|e| AppError::Pdf(format!("Failed to get page {}: {e}", page_idx + 1)))?;

        let text = page
            .text()
            .map_err(|e| {
                AppError::Pdf(format!(
                    "Failed to extract text from page {}: {e}",
                    page_idx + 1
                ))
            })?;

        let all_text = text.all();

        let rows: Vec<Vec<String>> = all_text
            .lines()
            .map(|line| {
                split_line_into_cells(line)
                    .into_iter()
                    .map(|s| s.to_owned())
                    .collect()
            })
            .filter(|row: &Vec<String>| !row.is_empty())
            .collect();

        let sheet_name = format!("Page {}", page_idx + 1);
        sheets.push((sheet_name, rows));

        // Progress: 20% -> 80% spread across pages
        let percent = 20 + ((page_idx + 1) as u8 * 60 / total_pages as u8).min(60);
        emit_progress(
            &app,
            &op_id,
            percent,
            &format!("Extracted page {} of {}", page_idx + 1, total_pages),
        );
    }

    // --- Write xlsx ---
    emit_progress(&app, &op_id, 85, "Writing Excel file...");

    let output_dir = input_path
        .parent()
        .ok_or_else(|| AppError::Pdf("Cannot determine output directory".into()))?;
    let output_path = output_dir.join(format!("{stem}.xlsx"));

    write_xlsx(&sheets, &output_path)?;

    emit_complete(&app, &op_id);
    Ok(output_path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_excel() {
        let p = std::path::PathBuf::from("/tmp/report.pdf");
        assert_eq!(output_stem(&p), "report");
    }

    #[test]
    fn split_line_into_cells_basic() {
        let cells = split_line_into_cells("Name    Age    City");
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], "Name");
        assert_eq!(cells[1], "Age");
        assert_eq!(cells[2], "City");
    }

    #[test]
    fn split_line_into_cells_tabs() {
        let cells = split_line_into_cells("A\tB\tC");
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], "A");
        assert_eq!(cells[1], "B");
        assert_eq!(cells[2], "C");
    }

    #[test]
    fn split_line_into_cells_single() {
        let cells = split_line_into_cells("Just one cell");
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0], "Just one cell");
    }

    #[test]
    fn split_line_into_cells_empty() {
        let cells = split_line_into_cells("");
        assert!(cells.is_empty());
    }

    #[test]
    fn split_line_into_cells_whitespace_only() {
        let cells = split_line_into_cells("   ");
        assert!(cells.is_empty());
    }

    #[test]
    fn col_letter_converts_correctly() {
        assert_eq!(col_letter(0), "A");
        assert_eq!(col_letter(1), "B");
        assert_eq!(col_letter(25), "Z");
        assert_eq!(col_letter(26), "AA");
        assert_eq!(col_letter(27), "AB");
        assert_eq!(col_letter(701), "ZZ");
        assert_eq!(col_letter(702), "AAA");
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn write_xlsx_creates_valid_zip() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_output.xlsx");

        let sheets = vec![(
            "Sheet1".to_owned(),
            vec![
                vec!["Name".to_owned(), "Age".to_owned()],
                vec!["Alice".to_owned(), "30".to_owned()],
            ],
        )];

        write_xlsx(&sheets, &path).unwrap();

        // Verify the file is a valid zip with expected entries
        let file = std::fs::File::open(&path).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();

        let names: Vec<&str> = archive.file_names().collect();
        assert!(names.contains(&"[Content_Types].xml"));
        assert!(names.contains(&"_rels/.rels"));
        assert!(names.contains(&"xl/workbook.xml"));
        assert!(names.contains(&"xl/_rels/workbook.xml.rels"));
        assert!(names.contains(&"xl/styles.xml"));
        assert!(names.contains(&"xl/worksheets/sheet1.xml"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_xlsx_multiple_sheets() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_multi_sheet.xlsx");

        let sheets = vec![
            ("Page 1".to_owned(), vec![vec!["Hello".to_owned()]]),
            ("Page 2".to_owned(), vec![vec!["World".to_owned()]]),
        ];

        write_xlsx(&sheets, &path).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();

        let names: Vec<&str> = archive.file_names().collect();
        assert!(names.contains(&"xl/worksheets/sheet1.xml"));
        assert!(names.contains(&"xl/worksheets/sheet2.xml"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sheet_xml_numeric_cells() {
        let rows = vec![vec!["42".to_owned(), "3.14".to_owned()]];
        let xml = sheet_xml(&rows).unwrap();
        // Numeric cells should NOT have t="inlineStr"
        assert!(!xml.contains("inlineStr") || !xml.contains(">42<"));
        // They should have a <v> element
        assert!(xml.contains("<v>42</v>"));
        assert!(xml.contains("<v>3.14</v>"));
    }

    #[test]
    fn sheet_xml_string_cells() {
        let rows = vec![vec!["Hello".to_owned()]];
        let xml = sheet_xml(&rows).unwrap();
        assert!(xml.contains("inlineStr"));
        assert!(xml.contains("Hello"));
    }
}
