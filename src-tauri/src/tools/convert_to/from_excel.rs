use std::path::{Path, PathBuf};

use calamine::{open_workbook_auto, Data, Reader};
use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument};
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::pipeline::progress::{emit_complete, emit_progress};
use crate::tools::ProcessRequest;

/// Maximum columns rendered per row before truncation.
const MAX_COLS: usize = 6;
/// Column width in mm.
const COL_WIDTH: f32 = 30.0;
/// Row height in mm.
const ROW_HEIGHT: f32 = 5.0;
/// Top margin in mm (start of first text row).
const TOP_Y: f32 = 280.0;
/// Bottom margin in mm.
const BOTTOM_Y: f32 = 20.0;
/// Left margin in mm.
const LEFT_X: f32 = 15.0;
/// Font size in pt.
const FONT_SIZE: f32 = 8.0;
fn output_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string()
}

/// Convert a calamine `Data` cell to a display string.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::Empty => String::new(),
        _ => String::new(),
    }
}

pub async fn run(app: AppHandle, req: ProcessRequest) -> Result<PathBuf> {
    let op_id = req.operation_id.clone();
    let input_path = req
        .input_paths
        .first()
        .ok_or_else(|| AppError::Validation("No input file".into()))?;

    // Validate extension
    let ext = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "xlsx" && ext != "xls" && ext != "xlsb" && ext != "ods" {
        return Err(AppError::Validation(format!(
            "Unsupported spreadsheet format: .{ext}"
        )));
    }

    emit_progress(&app, &op_id, 5, "Loading spreadsheet...");

    let mut workbook = open_workbook_auto(input_path)
        .map_err(|e| AppError::Pdf(format!("Failed to open workbook: {e}")))?;

    let sheet_names = workbook.sheet_names().to_owned();
    if sheet_names.is_empty() {
        return Err(AppError::Validation("Workbook has no sheets".into()));
    }

    let total_sheets = sheet_names.len();

    // Create PDF document
    let (doc, first_page, first_layer) =
        PdfDocument::new("Excel to PDF", Mm(210.0), Mm(297.0), &sheet_names[0]);
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(format!("Failed to add font: {e}")))?;

    for (idx, name) in sheet_names.iter().enumerate() {
        let percent = 10 + (idx * 80) / total_sheets.max(1);
        emit_progress(
            &app,
            &op_id,
            percent as u8,
            &format!("Rendering sheet '{name}'..."),
        );

        let range = match workbook.worksheet_range(name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // For the first sheet, use the already-created page; for subsequent sheets, add a new page.
        if idx == 0 {
            render_sheet_to_page(
                &doc,
                first_page,
                first_layer,
                &range,
                &font,
            );
        } else {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| {
                    row.iter()
                        .take(MAX_COLS)
                        .map(cell_to_string)
                        .collect()
                })
                .collect();
            render_rows_to_new_pages(&doc, &rows, &font, name);
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

/// Render a calamine range onto the first page (already created).
fn render_sheet_to_page(
    doc: &printpdf::PdfDocumentReference,
    page_idx: printpdf::indices::PdfPageIndex,
    layer_idx: printpdf::indices::PdfLayerIndex,
    range: &calamine::Range<Data>,
    font: &IndirectFontRef,
) {
    let layer = doc.get_page(page_idx).get_layer(layer_idx);
    let mut y = TOP_Y;

    for row in range.rows() {
        if y < BOTTOM_Y {
            break; // Simple: stop rendering if we exceed one page for sheet 1
        }
        for (col_idx, cell) in row.iter().take(MAX_COLS).enumerate() {
            let text = cell_to_string(cell);
            if !text.is_empty() {
                let x = LEFT_X + (col_idx as f32 * COL_WIDTH);
                layer.use_text(&text, FONT_SIZE, Mm(x), Mm(y), font);
            }
        }
        y -= ROW_HEIGHT;
    }
}

/// Render rows across one or more new pages.
fn render_rows_to_new_pages(
    doc: &printpdf::PdfDocumentReference,
    rows: &[Vec<String>],
    font: &IndirectFontRef,
    sheet_name: &str,
) {
    if rows.is_empty() {
        // Add an empty page for the sheet
        let (page, layer) = doc.add_page(Mm(210.0), Mm(297.0), sheet_name);
        let _layer_ref = doc.get_page(page).get_layer(layer);
        return;
    }

    let rows_per_page = ((TOP_Y - BOTTOM_Y) / ROW_HEIGHT) as usize;

    for chunk in rows.chunks(rows_per_page) {
        let (page, layer) = doc.add_page(Mm(210.0), Mm(297.0), sheet_name);
        let layer_ref = doc.get_page(page).get_layer(layer);
        let mut y = TOP_Y;

        for row in chunk {
            for (col_idx, text) in row.iter().enumerate() {
                if !text.is_empty() {
                    let x = LEFT_X + (col_idx as f32 * COL_WIDTH);
                    layer_ref.use_text(text, FONT_SIZE, Mm(x), Mm(y), font);
                }
            }
            y -= ROW_HEIGHT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stem_excel() {
        assert_eq!(output_stem(&PathBuf::from("/tmp/data.xlsx")), "data");
    }

    #[test]
    fn cell_to_string_formats() {
        assert_eq!(cell_to_string(&Data::Float(3.14)), "3.14");
        assert_eq!(cell_to_string(&Data::Int(42)), "42");
        assert_eq!(cell_to_string(&Data::Bool(true)), "true");
        assert_eq!(cell_to_string(&Data::Empty), "");
    }

    #[test]
    fn output_stem_no_extension() {
        assert_eq!(output_stem(&PathBuf::from("/tmp/report")), "report");
    }

    #[test]
    fn cell_to_string_string_variant() {
        assert_eq!(
            cell_to_string(&Data::String("hello".into())),
            "hello"
        );
    }
}
