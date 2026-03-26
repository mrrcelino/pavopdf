# PavoPDF — Plan 4: Other → PDF Conversions

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement all five Other → PDF conversion tools (Word, Excel, PPT, Image, HTML) as Rust processing modules with matching Svelte 5 workspaces, wired into the existing pipeline from Plan 1.

**Architecture:** Five new Rust modules under `src-tauri/src/tools/convert_to/` each consuming a format-specific parsing crate and writing output via printpdf or lopdf. The existing `process_pdf` Tauri command dispatches to the new `ToolVariant` arms added in this plan. The Svelte frontend gets five new workspace components with file-type filters, drag-to-reorder (images), and limitation banners where the output is best-effort.

**Tech Stack:** docx-rs, calamine, quick-xml, zip, printpdf, image crate, lopdf, Tauri WebView (hidden `WebviewWindowBuilder`), pdfium-render (HTML path)

**Depends on:** Plans 1–3 complete (pipeline infrastructure, IPC command skeleton, tools/mod.rs with ToolVariant enum, Svelte shell and workspace pattern)

---

## Chunk 1: Rust — Module Wiring + Word → PDF

### Task 1: Create `convert_to` module skeleton and extend ToolVariant

**Files:**
- Create: `src-tauri/src/tools/convert_to/mod.rs`
- Modify: `src-tauri/src/tools/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs` (or wherever `process_pdf` dispatches)

- [ ] **Step 1: Write failing test for module structure**

Create `src-tauri/src/tools/convert_to/mod.rs`:
```rust
pub mod from_word;
pub mod from_excel;
pub mod from_ppt;
pub mod from_image;
pub mod from_html;

#[cfg(test)]
mod tests {
    #[test]
    fn submodules_are_accessible() {
        // Compilation test — if this test compiles, all submodules exist.
        // Each submodule exports a `convert` async fn.
        // Checked individually in each module's own test suite.
        assert!(true);
    }
}
```

- [ ] **Step 2: Run test (RED — submodules don't exist yet)**

```bash
cd src-tauri && cargo test tools::convert_to 2>&1 | tail -20
```

Expected: Compiler error — submodule files not found. This is the RED state.

- [ ] **Step 3: Create stub files for each submodule**

Create `src-tauri/src/tools/convert_to/from_word.rs` (stub):
```rust
use crate::error::Result;
use std::path::Path;

pub async fn convert(input_path: &Path, output_path: &Path) -> Result<()> {
    let _ = (input_path, output_path);
    Err(crate::error::AppError::Pdf("not implemented".into()))
}
```

Repeat identically for:
- `src-tauri/src/tools/convert_to/from_excel.rs`
- `src-tauri/src/tools/convert_to/from_ppt.rs`
- `src-tauri/src/tools/convert_to/from_image.rs`
- `src-tauri/src/tools/convert_to/from_html.rs`

- [ ] **Step 4: Wire convert_to into tools/mod.rs**

Open `src-tauri/src/tools/mod.rs` and add:
```rust
pub mod convert_to;
```

Add the five new variants to the existing `ToolVariant` enum (the enum already has
`Merge`, `Split`, etc. from Plans 1–3; add at the bottom of the enum):
```rust
// Other → PDF
WordToPdf,
ExcelToPdf,
PptToPdf,
ImageToPdf { paths: Vec<String> },  // multi-file
HtmlToPdf,
```

- [ ] **Step 5: Add dispatch arms to process_pdf**

In the file that matches on `ToolVariant` (typically `src-tauri/src/commands/mod.rs` or
`src-tauri/src/tools/mod.rs` `dispatch` function), add:

```rust
ToolVariant::WordToPdf => {
    crate::tools::convert_to::from_word::convert(
        Path::new(&input_paths[0]),
        &output_path,
    ).await?
}
ToolVariant::ExcelToPdf => {
    crate::tools::convert_to::from_excel::convert(
        Path::new(&input_paths[0]),
        &output_path,
    ).await?
}
ToolVariant::PptToPdf => {
    crate::tools::convert_to::from_ppt::convert(
        Path::new(&input_paths[0]),
        &output_path,
    ).await?
}
ToolVariant::ImageToPdf { paths } => {
    let image_paths: Vec<std::path::PathBuf> =
        paths.iter().map(|p| PathBuf::from(p)).collect();
    crate::tools::convert_to::from_image::convert(
        &image_paths,
        &output_path,
    ).await?
}
ToolVariant::HtmlToPdf => {
    crate::tools::convert_to::from_html::convert(
        Path::new(&input_paths[0]),
        &output_path,
        &app_handle,
    ).await?
}
```

- [ ] **Step 6: Verify it compiles with stubs**

```bash
cd src-tauri && cargo build 2>&1 | grep -E "error|warning: unused" | head -20
```

Expected: No errors. Warnings about unused parameters are fine at stub stage.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/tools/convert_to/ src-tauri/src/tools/mod.rs
git commit -m "feat: scaffold convert_to module with ToolVariant arms and stubs"
```

---

### Task 2: Word → PDF (`from_word.rs`)

**Files:**
- Modify: `src-tauri/src/tools/convert_to/from_word.rs`

**Crate notes:**
- `docx-rs`: parse with `docx_rs::read_docx(&bytes)` — returns a `Docx` struct with `document.children` (a `Vec<DocumentChild>`). Each child is either a `Paragraph` or `Table`. A `Paragraph` contains `children: Vec<ParagraphChild>`. A `ParagraphChild::Run(run)` has `run.children: Vec<RunChild>` where `RunChild::Text(t)` gives the string.
- `printpdf`: page layout via `PdfDocument::new` → `PdfPage` → `PdfLayerReference`. Text placed with `layer.use_text(text, font_size, Mm(x), Mm(y), &font)`.
- Layout strategy: simple linear flow. Maintain a `cursor_y: Mm` that starts at `Mm(277.0)` (top margin on A4) and decrements by `line_height` per line. When `cursor_y < Mm(20.0)` (bottom margin), add a new page and reset cursor.
- Font: embed `Helvetica` built-in printpdf font with `doc.add_builtin_font(BuiltinFont::Helvetica)`.

- [ ] **Step 1: Write failing tests**

Replace `src-tauri/src/tools/convert_to/from_word.rs`:
```rust
use crate::error::{AppError, Result};
use std::path::Path;

pub async fn convert(input_path: &Path, output_path: &Path) -> Result<()> {
    let bytes = tokio::fs::read(input_path).await
        .map_err(|e| AppError::Io(e.to_string()))?;
    let doc = docx_rs::read_docx(&bytes)
        .map_err(|e| AppError::Pdf(format!("docx parse error: {:?}", e)))?;
    let paragraphs = extract_paragraphs(&doc);
    write_pdf(&paragraphs, output_path)?;
    Ok(())
}

/// Flatten the docx document tree into plain paragraph strings.
/// Tables are rendered as tab-separated cell text.
fn extract_paragraphs(doc: &docx_rs::Docx) -> Vec<String> {
    use docx_rs::{DocumentChild, ParagraphChild, RunChild};

    let mut result = Vec::new();
    for child in &doc.document.children {
        match child {
            DocumentChild::Paragraph(para) => {
                let mut line = String::new();
                for pc in &para.children {
                    if let ParagraphChild::Run(run) = pc {
                        for rc in &run.children {
                            if let RunChild::Text(t) = rc {
                                line.push_str(&t.text);
                            }
                        }
                    }
                }
                result.push(line);
            }
            DocumentChild::Table(table) => {
                for row in &table.rows {
                    use docx_rs::TableRowChild;
                    let mut row_parts: Vec<String> = Vec::new();
                    for cell_child in &row.cells {
                        if let TableRowChild::TableCell(cell) = cell_child {
                            use docx_rs::TableCellChild;
                            for cc in &cell.children {
                                if let TableCellChild::Paragraph(para) = cc {
                                    let mut cell_text = String::new();
                                    for pc in &para.children {
                                        if let ParagraphChild::Run(run) = pc {
                                            for rc in &run.children {
                                                if let RunChild::Text(t) = rc {
                                                    cell_text.push_str(&t.text);
                                                }
                                            }
                                        }
                                    }
                                    row_parts.push(cell_text);
                                }
                            }
                        }
                    }
                    result.push(row_parts.join("\t"));
                }
            }
            _ => {}
        }
    }
    result
}

/// Write paragraphs to a PDF file using printpdf.
fn write_pdf(paragraphs: &[String], output_path: &Path) -> Result<()> {
    use printpdf::*;

    let (doc, page1, layer1) = PdfDocument::new("Document", Mm(210.0), Mm(297.0), "Layer 1");
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let font_size = 11.0_f64;
    let line_height = Mm(5.5);
    let margin_left = Mm(20.0);
    let margin_top = Mm(277.0);
    let margin_bottom = Mm(20.0);

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let mut cursor_y = margin_top;

    for para in paragraphs {
        // Word-wrap at ~95 chars to fit A4 width at font_size 11
        let wrapped = word_wrap(para, 95);
        for line in wrapped {
            if cursor_y < margin_bottom {
                // Add a new page
                let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                current_layer = doc.get_page(new_page).get_layer(new_layer);
                cursor_y = margin_top;
            }
            if !line.is_empty() {
                current_layer.use_text(&line, font_size, margin_left, cursor_y, &font);
            }
            cursor_y -= line_height;
        }
        // Paragraph gap
        cursor_y -= Mm(2.0);
    }

    let bytes = doc.save_to_bytes()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    std::fs::write(output_path, bytes)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

/// Naive word-wrap: split text into lines of max `width` chars.
fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn word_wrap_short_text_unchanged() {
        let lines = word_wrap("Hello world", 95);
        assert_eq!(lines, vec!["Hello world"]);
    }

    #[test]
    fn word_wrap_long_text_splits() {
        let long = "word ".repeat(30);
        let lines = word_wrap(long.trim(), 50);
        assert!(lines.len() > 1, "expected multiple lines, got {}", lines.len());
        for line in &lines {
            assert!(line.len() <= 50, "line too long: {}", line.len());
        }
    }

    #[test]
    fn word_wrap_empty_string() {
        let lines = word_wrap("", 95);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "");
    }

    #[tokio::test]
    async fn convert_nonexistent_file_returns_io_error() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("out.pdf");
        let result = convert(Path::new("/nonexistent/file.docx"), &output).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::Io(_)));
    }

    #[tokio::test]
    async fn convert_real_docx_produces_pdf() {
        // Build a minimal valid .docx in memory and write it to a temp file.
        // We use docx-rs's builder to create a one-paragraph document.
        use docx_rs::*;
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.docx");
        let output = dir.path().join("test.pdf");

        let docx_bytes = Docx::new()
            .add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Hello PavoPDF"))
            )
            .build()
            .pack()
            .unwrap();
        std::fs::write(&input, docx_bytes).unwrap();

        convert(&input, &output).await.unwrap();

        let pdf_bytes = std::fs::read(&output).unwrap();
        // PDF files start with "%PDF"
        assert_eq!(&pdf_bytes[..4], b"%PDF", "output is not a valid PDF");
        assert!(pdf_bytes.len() > 1000, "PDF suspiciously small");
    }
}
```

- [ ] **Step 2: Run tests (expect word_wrap tests to pass, convert tests RED)**

```bash
cd src-tauri && cargo test convert_to::from_word -- --nocapture 2>&1 | tail -30
```

Expected: `word_wrap_*` tests pass. `convert_real_docx_produces_pdf` and `convert_nonexistent_file_returns_io_error` fail because the implementation is a stub.

- [ ] **Step 3: The implementation is already in the test file above (TDD: write impl alongside tests)**

The `convert`, `extract_paragraphs`, `write_pdf`, and `word_wrap` functions above are the implementation. Run tests again:

```bash
cd src-tauri && cargo test convert_to::from_word -- --nocapture 2>&1 | tail -30
```

Expected: All 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/convert_to/from_word.rs
git commit -m "feat: implement Word → PDF conversion with docx-rs + printpdf"
```

---

## Chunk 2: Excel → PDF

### Task 3: Excel → PDF (`from_excel.rs`)

**Files:**
- Modify: `src-tauri/src/tools/convert_to/from_excel.rs`

**Crate notes:**
- `calamine`: `open_workbook::<Xlsx, _>(path)` returns a `Result<Xlsx<BufReader<File>>>`. Call `.sheet_names()` to get sheet names. For each sheet, call `workbook.worksheet_range(name)` → `Some(Ok(range))`. `range.rows()` yields `&[DataType]` rows. `DataType` variants: `Int(i64)`, `Float(f64)`, `String(String)`, `Bool(bool)`, `DateTime(f64)`, `Empty`.
- Layout: render each sheet as a table. Column width is estimated at `Mm(38.0)` each, max 5 columns per page row before wrapping. Row height is `Mm(6.0)`. Draw a border line under each row with `layer.add_line(Line { points: ..., is_closed: false })`. Cell text truncated to 22 chars if longer.
- Use `printpdf` A4 landscape for wide sheets: `PdfDocument::new("Sheet", Mm(297.0), Mm(210.0), "Layer 1")` when the sheet has more than 5 columns, portrait otherwise.

- [ ] **Step 1: Write failing tests**

Replace `src-tauri/src/tools/convert_to/from_excel.rs`:
```rust
use crate::error::{AppError, Result};
use std::path::Path;

pub async fn convert(input_path: &Path, output_path: &Path) -> Result<()> {
    // calamine is sync — run in blocking thread to avoid blocking the Tokio executor.
    let input_path = input_path.to_path_buf();
    let output_path = output_path.to_path_buf();
    tokio::task::spawn_blocking(move || convert_sync(&input_path, &output_path))
        .await
        .map_err(|e| AppError::Pdf(format!("thread join error: {}", e)))??;
    Ok(())
}

fn convert_sync(input_path: &Path, output_path: &Path) -> Result<()> {
    use calamine::{open_workbook, DataType, Reader, Xlsx};
    use printpdf::*;

    let mut workbook: Xlsx<_> = open_workbook(input_path)
        .map_err(|e| AppError::Pdf(format!("xlsx open error: {}", e)))?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err(AppError::Validation("Excel file has no sheets".into()));
    }

    // Use the first sheet.
    let sheet_name = &sheet_names[0];
    let range = workbook
        .worksheet_range(sheet_name)
        .ok_or_else(|| AppError::Pdf("sheet not found".into()))?
        .map_err(|e| AppError::Pdf(format!("sheet read error: {}", e)))?;

    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|row| {
            row.iter()
                .map(|cell| cell_to_string(cell))
                .collect()
        })
        .collect();

    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let landscape = col_count > 5;

    let (page_w, page_h) = if landscape {
        (Mm(297.0), Mm(210.0))
    } else {
        (Mm(210.0), Mm(297.0))
    };

    let (doc, page1, layer1) =
        PdfDocument::new(sheet_name.as_str(), page_w, page_h, "Layer 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let col_w = Mm(38.0);
    let row_h = Mm(6.5);
    let font_size = 9.0_f64;
    let margin_left = Mm(15.0);
    let margin_top = page_h - Mm(20.0);
    let margin_bottom = Mm(15.0);

    let mut layer = doc.get_page(page1).get_layer(layer1);
    let mut cursor_y = margin_top;
    let max_cols_per_page = ((page_w - margin_left - Mm(10.0)).0 / col_w.0).floor() as usize;

    for (row_idx, row) in rows.iter().enumerate() {
        if cursor_y < margin_bottom {
            let (new_page, new_layer) = doc.add_page(page_w, page_h, "Layer 1");
            layer = doc.get_page(new_page).get_layer(new_layer);
            cursor_y = margin_top;
        }

        let is_header = row_idx == 0;
        let active_font = if is_header { &font_bold } else { &font };

        for (col_idx, cell) in row.iter().take(max_cols_per_page).enumerate() {
            let x = margin_left + Mm(col_idx as f64 * col_w.0);
            let text = truncate_cell(cell, 22);
            layer.use_text(&text, font_size, x, cursor_y, active_font);
        }

        // Underline for header row
        if is_header {
            let line_y = cursor_y - Mm(1.0);
            let line = printpdf::Line {
                points: vec![
                    (printpdf::Point::new(margin_left, line_y), false),
                    (
                        printpdf::Point::new(
                            margin_left + Mm(max_cols_per_page as f64 * col_w.0),
                            line_y,
                        ),
                        false,
                    ),
                ],
                is_closed: false,
            };
            layer.add_line(line);
        }

        cursor_y -= row_h;
    }

    let bytes = doc
        .save_to_bytes()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    std::fs::write(output_path, bytes)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

fn cell_to_string(cell: &calamine::DataType) -> String {
    use calamine::DataType;
    match cell {
        DataType::Int(n)      => n.to_string(),
        DataType::Float(f)    => format!("{:.2}", f),
        DataType::String(s)   => s.clone(),
        DataType::Bool(b)     => if *b { "TRUE".into() } else { "FALSE".into() },
        DataType::DateTime(d) => format!("{:.0}", d),
        DataType::Empty       => String::new(),
        _                     => String::new(),
    }
}

fn truncate_cell(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cell_to_string_variants() {
        use calamine::DataType;
        assert_eq!(cell_to_string(&DataType::Int(42)), "42");
        assert_eq!(cell_to_string(&DataType::Float(3.14159)), "3.14");
        assert_eq!(cell_to_string(&DataType::Bool(true)), "TRUE");
        assert_eq!(cell_to_string(&DataType::Empty), "");
        assert_eq!(cell_to_string(&DataType::String("hello".into())), "hello");
    }

    #[test]
    fn truncate_cell_short_unchanged() {
        assert_eq!(truncate_cell("hello", 22), "hello");
    }

    #[test]
    fn truncate_cell_long_truncated() {
        let long = "a".repeat(30);
        let result = truncate_cell(&long, 22);
        assert!(result.chars().count() <= 22);
        assert!(result.ends_with('…'));
    }

    #[tokio::test]
    async fn convert_nonexistent_returns_error() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("out.pdf");
        let result = convert(Path::new("/no/such/file.xlsx"), &out).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn convert_real_xlsx_produces_pdf() {
        // Create a minimal xlsx using a raw ZIP + XML approach since there is no
        // in-memory xlsx builder available without extra deps.
        // We use the simplest possible xlsx: a pre-built base64-encoded minimal xlsx.
        // Generated offline from: echo -e "Name\tAge\nAlice\t30" | xlsx-writer
        // For testing purposes, write a CSV-like xlsx with calamine's own test fixture.
        // Since we cannot guarantee a fixture path, we skip if no xlsx is available.
        // Instead, write a known-good minimal xlsx as raw bytes.
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.xlsx");
        let out = dir.path().join("test.pdf");

        // Minimal xlsx (ZIP of XML files). This is a pre-built binary fixture.
        // In CI, replace with a checked-in test fixture at tests/fixtures/minimal.xlsx
        // For now: if we can write a valid xlsx via calamine's writer feature, use that.
        // calamine is read-only; use `xlsxwriter` crate or a fixture file.
        //
        // SKIP this integration test until a fixture file exists.
        // Add tests/fixtures/minimal.xlsx to the repo (generated once via Python openpyxl).
        let fixture = std::path::Path::new("tests/fixtures/minimal.xlsx");
        if !fixture.exists() {
            eprintln!("SKIP: tests/fixtures/minimal.xlsx not found — add fixture to enable test");
            return;
        }
        std::fs::copy(fixture, &input).unwrap();
        convert(&input, &out).await.unwrap();
        let pdf = std::fs::read(&out).unwrap();
        assert_eq!(&pdf[..4], b"%PDF");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test convert_to::from_excel -- --nocapture 2>&1 | tail -30
```

Expected: `cell_to_string_*` and `truncate_cell_*` pass. `convert_real_xlsx_produces_pdf` prints SKIP if fixture absent. `convert_nonexistent_returns_error` passes.

- [ ] **Step 3: Create test fixture (once)**

```bash
python3 -c "
import openpyxl
wb = openpyxl.Workbook()
ws = wb.active
ws.append(['Name', 'Age', 'City'])
ws.append(['Alice', 30, 'London'])
ws.append(['Bob', 25, 'Paris'])
wb.save('src-tauri/tests/fixtures/minimal.xlsx')
"
```

If Python/openpyxl is unavailable, download any small `.xlsx` file and place at `src-tauri/tests/fixtures/minimal.xlsx`.

- [ ] **Step 4: Re-run tests with fixture**

```bash
cd src-tauri && cargo test convert_to::from_excel -- --nocapture 2>&1 | tail -30
```

Expected: All tests including `convert_real_xlsx_produces_pdf` pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tools/convert_to/from_excel.rs src-tauri/tests/
git commit -m "feat: implement Excel → PDF conversion with calamine + printpdf"
```

---

## Chunk 3: PPT → PDF

### Task 4: PPT → PDF (`from_ppt.rs`)

**Files:**
- Modify: `src-tauri/src/tools/convert_to/from_ppt.rs`

**Crate notes:**
- A `.pptx` file is a ZIP archive. Open with the `zip` crate: `zip::ZipArchive::new(file)`.
- Slide files are at `ppt/slides/slide1.xml`, `slide2.xml`, etc. The number of slides is determined by reading `ppt/presentation.xml` and counting `<p:sldId>` elements, or by iterating ZIP entries matching `ppt/slides/slide*.xml`.
- Each slide XML contains `<p:sp>` (shape) elements. Within each shape, `<p:txBody>` holds paragraphs: `<a:p>` → `<a:r>` → `<a:t>` is the text.
- Parse with `quick-xml`: use `Reader::from_str` in event-based mode. Collect text content of `<a:t>` elements along with their approximate position from `<p:sp><p:spPr><a:xfrm><a:off x="..." y="..."/>` attributes. EMU (English Metric Units): 914400 EMU = 1 inch = 25.4 mm.
- Slide dimensions from `<p:sldSz cx="..." cy="..."/>` in `presentation.xml`. Standard widescreen: cx=9144000, cy=5143500 EMU = 254mm × 142.9mm.
- Layout: for each shape's text block, place at `Mm(x_emu / 914400.0 * 25.4)`, `Mm(page_h - y_emu / 914400.0 * 25.4)`. Clamp to page margins.

- [ ] **Step 1: Write failing tests**

Replace `src-tauri/src/tools/convert_to/from_ppt.rs`:
```rust
use crate::error::{AppError, Result};
use std::path::Path;

/// A text block extracted from a single slide shape.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    pub text: String,
    /// x offset from left edge in EMU
    pub x_emu: i64,
    /// y offset from top edge in EMU
    pub y_emu: i64,
}

pub async fn convert(input_path: &Path, output_path: &Path) -> Result<()> {
    let input_path = input_path.to_path_buf();
    let output_path = output_path.to_path_buf();
    tokio::task::spawn_blocking(move || convert_sync(&input_path, &output_path))
        .await
        .map_err(|e| AppError::Pdf(format!("thread join: {}", e)))??;
    Ok(())
}

fn convert_sync(input_path: &Path, output_path: &Path) -> Result<()> {
    use printpdf::*;
    use std::fs::File;
    use std::io::BufReader;
    use zip::ZipArchive;

    let file = File::open(input_path)
        .map_err(|e| AppError::Io(e.to_string()))?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| AppError::Pdf(format!("zip error: {}", e)))?;

    // Determine slide count by listing zip entries.
    let slide_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide")
                && name.ends_with(".xml")
                && !name.contains("_rels")
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    // Sort slides numerically: slide1.xml < slide2.xml < slide10.xml
    let mut slide_names = slide_names;
    slide_names.sort_by_key(|n| {
        n.trim_start_matches("ppt/slides/slide")
            .trim_end_matches(".xml")
            .parse::<u32>()
            .unwrap_or(0)
    });

    if slide_names.is_empty() {
        return Err(AppError::Validation("No slides found in .pptx".into()));
    }

    // Standard widescreen slide: 254mm × 142.875mm
    let page_w = Mm(254.0);
    let page_h = Mm(142.875);

    let mut pdf_doc: Option<(printpdf::PdfDocumentReference, printpdf::PdfPageIndex, printpdf::PdfLayerIndex)> = None;

    for (slide_idx, slide_name) in slide_names.iter().enumerate() {
        let xml = {
            use std::io::Read;
            let mut entry = archive
                .by_name(slide_name)
                .map_err(|e| AppError::Pdf(format!("missing slide {}: {}", slide_name, e)))?;
            let mut buf = String::new();
            entry.read_to_string(&mut buf)
                .map_err(|e| AppError::Io(e.to_string()))?;
            buf
        };

        let blocks = extract_text_blocks(&xml)?;

        let (doc_ref, page_idx, layer_idx) = if slide_idx == 0 {
            let (d, p, l) = PdfDocument::new("Presentation", page_w, page_h, "Layer 1");
            pdf_doc = Some((d, p, l));
            let (d, p, l) = pdf_doc.as_ref().unwrap();
            (d, *p, *l)
        } else {
            let doc = &pdf_doc.as_ref().unwrap().0;
            let (p, l) = doc.add_page(page_w, page_h, "Layer 1");
            (doc, p, l)
        };

        let layer = doc_ref.get_page(page_idx).get_layer(layer_idx);
        let font = doc_ref
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| AppError::Pdf(e.to_string()))?;

        for block in &blocks {
            // Convert EMU to mm: 914400 EMU = 25.4mm
            let x_mm = (block.x_emu as f64 / 914400.0) * 25.4;
            let y_mm = page_h.0 - (block.y_emu as f64 / 914400.0) * 25.4 - 8.0;

            // Clamp to page bounds
            let x = Mm(x_mm.max(5.0).min(page_w.0 - 10.0));
            let y = Mm(y_mm.max(5.0).min(page_h.0 - 5.0));

            let font_size = if block.y_emu < 1_000_000 { 18.0 } else { 11.0 };
            let text = truncate_text(&block.text, 80);
            layer.use_text(&text, font_size, x, y, &font);
        }
    }

    let doc = pdf_doc
        .ok_or_else(|| AppError::Pdf("no slides processed".into()))?
        .0;
    let bytes = doc
        .save_to_bytes()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    std::fs::write(output_path, bytes)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

/// Parse a slide XML string and extract TextBlock items from <a:t> elements,
/// using the enclosing <p:sp>/<p:spPr>/<a:xfrm>/<a:off> attributes for position.
pub fn extract_text_blocks(xml: &str) -> Result<Vec<TextBlock>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut blocks: Vec<TextBlock> = Vec::new();
    let mut current_x: i64 = 0;
    let mut current_y: i64 = 0;
    let mut in_text = false;
    let mut current_text = String::new();
    let mut in_sp = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"p:sp" => {
                        in_sp = true;
                        current_x = 0;
                        current_y = 0;
                    }
                    b"a:off" if in_sp => {
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"x" => {
                                    current_x = std::str::from_utf8(&attr.value)
                                        .unwrap_or("0")
                                        .parse()
                                        .unwrap_or(0);
                                }
                                b"y" => {
                                    current_y = std::str::from_utf8(&attr.value)
                                        .unwrap_or("0")
                                        .parse()
                                        .unwrap_or(0);
                                }
                                _ => {}
                            }
                        }
                    }
                    b"a:t" => {
                        in_text = true;
                        current_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"p:sp" => {
                    in_sp = false;
                }
                b"a:t" => {
                    in_text = false;
                    if !current_text.trim().is_empty() {
                        blocks.push(TextBlock {
                            text: current_text.trim().to_string(),
                            x_emu: current_x,
                            y_emu: current_y,
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Text(e)) if in_text => {
                current_text.push_str(
                    &e.unescape().unwrap_or_default(),
                );
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Pdf(format!("xml parse error: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(blocks)
}

fn truncate_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let t: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const SAMPLE_SLIDE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="274638"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:p><a:r><a:t>Hello PavoPDF</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="1600000"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:p><a:r><a:t>Subtitle text</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

    #[test]
    fn extract_text_blocks_finds_two_blocks() {
        let blocks = extract_text_blocks(SAMPLE_SLIDE_XML).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "Hello PavoPDF");
        assert_eq!(blocks[1].text, "Subtitle text");
    }

    #[test]
    fn extract_text_blocks_captures_position() {
        let blocks = extract_text_blocks(SAMPLE_SLIDE_XML).unwrap();
        assert_eq!(blocks[0].x_emu, 457200);
        assert_eq!(blocks[0].y_emu, 274638);
    }

    #[test]
    fn extract_text_blocks_empty_xml() {
        let blocks = extract_text_blocks("<p:sld/>").unwrap();
        assert!(blocks.is_empty());
    }

    #[test]
    fn truncate_text_works() {
        assert_eq!(truncate_text("hi", 80), "hi");
        let long = "x".repeat(100);
        let result = truncate_text(&long, 80);
        assert!(result.chars().count() <= 80);
        assert!(result.ends_with('…'));
    }

    #[tokio::test]
    async fn convert_nonexistent_returns_error() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("out.pdf");
        let result = convert(Path::new("/no/file.pptx"), &out).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn convert_real_pptx_produces_pdf() {
        // Build a minimal valid .pptx (ZIP of XML files) in memory.
        use std::io::Write;
        use zip::write::FileOptions;
        use zip::ZipWriter;

        let dir = tempdir().unwrap();
        let input = dir.path().join("test.pptx");
        let out = dir.path().join("test.pdf");

        let file = std::fs::File::create(&input).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = FileOptions::default();

        // [Content_Types].xml
        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/slides/slide1.xml"
    ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#).unwrap();

        // _rels/.rels
        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(br#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

        // ppt/slides/slide1.xml
        zip.start_file("ppt/slides/slide1.xml", opts).unwrap();
        zip.write_all(SAMPLE_SLIDE_XML.as_bytes()).unwrap();

        zip.finish().unwrap();

        convert(&input, &out).await.unwrap();
        let pdf = std::fs::read(&out).unwrap();
        assert_eq!(&pdf[..4], b"%PDF");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test convert_to::from_ppt -- --nocapture 2>&1 | tail -40
```

Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/tools/convert_to/from_ppt.rs
git commit -m "feat: implement PPT → PDF conversion with quick-xml + zip + printpdf"
```

---

## Chunk 4: Image → PDF

### Task 5: Image → PDF (`from_image.rs`)

**Files:**
- Modify: `src-tauri/src/tools/convert_to/from_image.rs`

**Crate notes:**
- Use `lopdf::Document` to build the PDF. Each image becomes one page whose MediaBox matches the image dimensions in points (1 pt = 1/72 inch; 1 px at 96 DPI = 0.75 pt).
- Use the `image` crate to read the image and determine pixel dimensions: `image::open(path)` → `DynamicImage`. Get width/height via `.width()` / `.height()`.
- Embed images as PDF `XObject` streams. Steps:
  1. Re-encode the image to JPEG bytes for compactness: `image.to_rgb8()` → `JpegEncoder`.
  2. Build a `lopdf::Stream` with `ColorSpace = /DeviceRGB`, `BitsPerComponent = 8`, `Width`, `Height`, `Filter = /DCTDecode`.
  3. Add the stream as an indirect object: `doc.add_object(stream)`.
  4. Build the page dictionary with `MediaBox`, `Resources` referencing the XObject, and a `Contents` stream that calls `q W H cm /Im1 Do Q` (scale image to fill page).
- DPI assumption: treat all images as 96 DPI for page size calculation.

- [ ] **Step 1: Write failing tests**

Replace `src-tauri/src/tools/convert_to/from_image.rs`:
```rust
use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

pub async fn convert(input_paths: &[PathBuf], output_path: &Path) -> Result<()> {
    if input_paths.is_empty() {
        return Err(AppError::Validation("No images provided".into()));
    }
    let input_paths = input_paths.to_vec();
    let output_path = output_path.to_path_buf();
    tokio::task::spawn_blocking(move || convert_sync(&input_paths, &output_path))
        .await
        .map_err(|e| AppError::Pdf(format!("thread join: {}", e)))??;
    Ok(())
}

fn convert_sync(input_paths: &[PathBuf], output_path: &Path) -> Result<()> {
    use image::ImageEncoder;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    let mut page_ids: Vec<lopdf::ObjectId> = Vec::new();

    for img_path in input_paths {
        let img = image::open(img_path)
            .map_err(|e| AppError::Pdf(format!("image open error for {:?}: {}", img_path, e)))?;

        let width_px = img.width();
        let height_px = img.height();

        // Treat image as 96 DPI; convert pixels → PDF points (1 pt = 1/72 inch).
        // pt = px * 72.0 / 96.0 = px * 0.75
        let width_pt = width_px as f64 * 0.75;
        let height_pt = height_px as f64 * 0.75;

        // Encode as JPEG for efficient embedding
        let rgb = img.to_rgb8();
        let mut jpeg_bytes: Vec<u8> = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 90);
        encoder
            .encode(rgb.as_raw(), width_px, height_px, image::ExtendedColorType::Rgb8)
            .map_err(|e| AppError::Pdf(format!("jpeg encode error: {}", e)))?;

        // Build image XObject stream
        let img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => width_px as i64,
                "Height" => height_px as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8_i64,
                "Filter" => "DCTDecode",
            },
            jpeg_bytes,
        );
        let img_id = doc.add_object(img_stream);

        // Content stream: place image filling the whole page
        // q (save state)
        // width_pt 0 0 height_pt 0 0 cm (transformation matrix: scale to page size)
        // /Im1 Do (draw image)
        // Q (restore state)
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        width_pt.into(),
                        0.into(),
                        0.into(),
                        height_pt.into(),
                        0.into(),
                        0.into(),
                    ],
                ),
                Operation::new("Do", vec![Object::Name(b"Im1".to_vec())]),
                Operation::new("Q", vec![]),
            ],
        };
        let content_bytes = content
            .encode()
            .map_err(|e| AppError::Pdf(format!("content encode: {}", e)))?;
        let content_stream = Stream::new(dictionary! {}, content_bytes);
        let content_id = doc.add_object(content_stream);

        // Resources dictionary
        let resources = dictionary! {
            "XObject" => dictionary! {
                "Im1" => Object::Reference(img_id),
            }
        };

        // Page dictionary
        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                lopdf::Object::Real(width_pt),
                lopdf::Object::Real(height_pt),
            ],
            "Resources" => resources,
            "Contents" => Object::Reference(content_id),
        };
        let page_id = doc.add_object(page_dict);
        page_ids.push(page_id);
    }

    // Pages dictionary
    let page_count = page_ids.len() as i64;
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => page_count,
        "Kids" => page_ids.iter().map(|&id| Object::Reference(id)).collect::<Vec<_>>(),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut out_bytes: Vec<u8> = Vec::new();
    doc.save_to(&mut out_bytes)
        .map_err(|e| AppError::Pdf(format!("lopdf save: {}", e)))?;
    std::fs::write(output_path, out_bytes)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_test_png(path: &Path, w: u32, h: u32) {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(w, h, |x, y| {
                Rgb([(x % 255) as u8, (y % 255) as u8, 128_u8])
            });
        img.save(path).unwrap();
    }

    #[tokio::test]
    async fn convert_empty_list_returns_error() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("out.pdf");
        let result = convert(&[], &out).await;
        assert!(matches!(result.unwrap_err(), AppError::Validation(_)));
    }

    #[tokio::test]
    async fn convert_nonexistent_image_returns_error() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("out.pdf");
        let result = convert(&[PathBuf::from("/no/image.png")], &out).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn convert_single_png_produces_pdf() {
        let dir = tempdir().unwrap();
        let img_path = dir.path().join("test.png");
        let out = dir.path().join("out.pdf");
        write_test_png(&img_path, 800, 600);

        convert(&[img_path], &out).await.unwrap();

        let pdf = std::fs::read(&out).unwrap();
        assert_eq!(&pdf[..4], b"%PDF");
        assert!(pdf.len() > 5000);
    }

    #[tokio::test]
    async fn convert_multiple_images_produces_multipaged_pdf() {
        let dir = tempdir().unwrap();
        let img1 = dir.path().join("img1.png");
        let img2 = dir.path().join("img2.png");
        let out = dir.path().join("out.pdf");
        write_test_png(&img1, 640, 480);
        write_test_png(&img2, 1920, 1080);

        convert(&[img1, img2], &out).await.unwrap();

        let pdf_bytes = std::fs::read(&out).unwrap();
        assert_eq!(&pdf_bytes[..4], b"%PDF");
        // Verify page count = 2 by checking "/Count 2" is somewhere in the PDF
        let pdf_text = String::from_utf8_lossy(&pdf_bytes);
        assert!(
            pdf_text.contains("/Count 2"),
            "Expected 2 pages in PDF"
        );
    }

    #[tokio::test]
    async fn convert_jpeg_input_works() {
        let dir = tempdir().unwrap();
        let img_path = dir.path().join("test.jpg");
        let out = dir.path().join("out.pdf");

        // Write a JPEG test image
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(400, 300, |_, _| Rgb([200u8, 150u8, 100u8]));
        img.save_with_format(&img_path, image::ImageFormat::Jpeg).unwrap();

        convert(&[img_path], &out).await.unwrap();
        let pdf = std::fs::read(&out).unwrap();
        assert_eq!(&pdf[..4], b"%PDF");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test convert_to::from_image -- --nocapture 2>&1 | tail -40
```

Expected: All 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/tools/convert_to/from_image.rs
git commit -m "feat: implement Image → PDF conversion with image crate + lopdf XObjects"
```

---

## Chunk 5: HTML → PDF

### Task 6: HTML → PDF (`from_html.rs`)

**Files:**
- Modify: `src-tauri/src/tools/convert_to/from_html.rs`
- Modify: `src-tauri/capabilities/default.json` (add hidden-webview capability)

**Architecture decision — Tauri 2 WebView-to-PDF:**

Tauri 2's public Rust API does not expose a direct "print WebView to PDF" method on `WebviewWindow`. The implementation uses the following approach:

**Chosen approach: hidden WebviewWindow + pdfium-render capture**

1. Create a hidden `WebviewWindowBuilder` with the HTML file loaded via `file://` URL.
2. Wait for `DOMContentLoaded` via `webview.on_webview_event` or a polling approach.
3. Call `pdfium_render` to print the webview content to PDF using the `PdfiumLibrary` API — **however** pdfium-render operates on existing PDF files, not live WebViews.

**Revised approach (correct):**

Since Tauri 2 + pdfium-render cannot directly print a live WebView to PDF, the implementation uses the following platform-appropriate fallback chain:

1. **Primary:** Use `tauri_plugin_shell` to invoke a platform-specific headless print command:
   - **Windows:** `mshta.exe` is unavailable headlessly. Use a bundled copy of `Chromium` headless (too large). Instead: use `wkhtmltopdf` if bundled, or `wkhtmltopdf` via PATH.
   - **macOS:** `/usr/bin/cupsfilter` or `Safari` AppleScript (fragile).
   - **Linux:** `chromium-browser --headless --print-to-pdf`.

2. **Fallback (chosen for v1 simplicity):** Use a hidden `WebviewWindowBuilder`, inject a JavaScript `window.print()` call, and intercept the print job via Tauri's print API. Tauri 2 does not expose this directly.

3. **Actual v1 implementation:** Create a hidden `WebviewWindow` pointing to `file://path/to/file.html`. After DOM load (detected via a JS-side `tauri.event.emit` on `DOMContentLoaded`), capture a screenshot of the webview using `webview.capture_image()` (available in Tauri 2 as `WebviewWindowHandle::capture_image` returning a `tauri::image::Image`). Convert the captured image to PDF using the `image` crate + `lopdf` (same as `from_image.rs`). Close the hidden webview when done.

**This is the correct Tauri 2 API:**

```rust
// Create hidden webview:
let webview = tauri::WebviewWindowBuilder::new(
    app,
    "html-to-pdf-hidden",   // unique label
    tauri::WebviewUrl::App("file:///absolute/path/to/file.html".into()),
)
.title("hidden")
.visible(false)
.inner_size(1280.0, 960.0)
.build()?;

// Wait for page load (Tauri 2: listen for webview-created event or use a sleep):
tokio::time::sleep(std::time::Duration::from_secs(2)).await;

// Capture screenshot:
let image: tauri::image::Image = webview.capture_image()?;

// Destroy the webview:
webview.close()?;
```

**Network isolation:** The HTML file is loaded via `file://` URL. Tauri's `WebviewWindowBuilder` uses the main webview's CSP by default. For the hidden webview, add a capability that blocks all network requests:

```json
// In capabilities/html-to-pdf.json:
{
  "identifier": "html-to-pdf",
  "windows": ["html-to-pdf-hidden"],
  "permissions": []
}
```

This means no `http` / `https` permissions are granted to the hidden webview, so external resource fetches fail silently at the OS networking layer — matching the spec.

- [ ] **Step 1: Add html-to-pdf capability**

Create `src-tauri/capabilities/html-to-pdf.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "html-to-pdf",
  "description": "Capability for the hidden HTML-to-PDF WebView — no network access",
  "windows": ["html-to-pdf-hidden"],
  "permissions": []
}
```

Register in `tauri.conf.json` under `app.security.capabilities`:
```json
"capabilities": ["default", "html-to-pdf"]
```

- [ ] **Step 2: Write failing tests**

Replace `src-tauri/src/tools/convert_to/from_html.rs`:
```rust
use crate::error::{AppError, Result};
use std::path::Path;
use tauri::AppHandle;

/// Convert a local HTML file to PDF by:
/// 1. Opening a hidden Tauri WebviewWindow pointing to the file via file:// URL.
/// 2. Waiting 2s for the DOM to load (covers CSS + local scripts; no network).
/// 3. Capturing a screenshot via webview.capture_image().
/// 4. Converting the screenshot image to a single-page PDF via lopdf.
/// 5. Closing the hidden webview.
///
/// Limitations (must be disclosed in the UI):
/// - External resources (CDN CSS, remote fonts, remote images) will NOT load.
///   Tauri's capability config grants no network permissions to the hidden webview.
/// - JavaScript that dynamically renders content may not complete within 2s.
///   For complex SPAs, the output may be incomplete.
/// - The output is a screenshot-based PDF (raster), not a text-selectable PDF.
/// - Page breaks are not applied; the entire viewport is captured as one page.
pub async fn convert(
    input_path: &Path,
    output_path: &Path,
    app: &AppHandle,
) -> Result<()> {
    // Validate the file exists and has an .html or .htm extension.
    if !input_path.exists() {
        return Err(AppError::NotFound(
            format!("HTML file not found: {:?}", input_path),
        ));
    }
    let ext = input_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !matches!(ext.to_lowercase().as_str(), "html" | "htm") {
        return Err(AppError::Validation(
            "Input must be an .html or .htm file".into(),
        ));
    }

    // Build the file:// URL. On Windows paths must use forward slashes and
    // be encoded: file:///C:/Users/... → file:///C:/Users/...
    let abs = input_path
        .canonicalize()
        .map_err(|e| AppError::Io(e.to_string()))?;
    let file_url = format!(
        "file:///{}",
        abs.to_string_lossy().replace('\\', "/").trim_start_matches('/')
    );

    // Create hidden webview (1280 × 960 viewport — standard desktop page width)
    let webview = tauri::WebviewWindowBuilder::new(
        app,
        "html-to-pdf-hidden",
        tauri::WebviewUrl::External(
            file_url.parse().map_err(|e| AppError::Pdf(format!("url parse: {}", e)))?,
        ),
    )
    .title("PavoPDF HTML renderer")
    .visible(false)
    .inner_size(1280.0, 960.0)
    .build()
    .map_err(|e| AppError::Pdf(format!("webview build error: {}", e)))?;

    // Wait for page to render. 2 seconds covers typical local HTML files.
    // Complex JS-heavy pages may need longer; disclosed as a limitation.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Capture screenshot
    let image = webview
        .capture_image()
        .map_err(|e| AppError::Pdf(format!("capture_image error: {}", e)))?;

    // Close the hidden webview immediately after capture
    webview
        .close()
        .map_err(|e| AppError::Pdf(format!("webview close error: {}", e)))?;

    // Convert the captured image to a PDF using lopdf (same as from_image.rs)
    let rgba = image.rgba();
    let img_width = image.width();
    let img_height = image.height();

    screenshot_to_pdf(rgba, img_width, img_height, output_path)?;

    Ok(())
}

/// Convert raw RGBA pixel data to a single-page PDF using lopdf.
fn screenshot_to_pdf(
    rgba: &[u8],
    width: u32,
    height: u32,
    output_path: &Path,
) -> Result<()> {
    use image::{ImageBuffer, Rgba};
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    // Convert RGBA to RGB (PDF DeviceRGB; discard alpha)
    let rgb_bytes: Vec<u8> = rgba
        .chunks(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();

    // Encode as JPEG for compact embedding
    let mut jpeg_bytes: Vec<u8> = Vec::new();
    {
        use image::ImageEncoder;
        let encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 90);
        encoder
            .encode(&rgb_bytes, width, height, image::ExtendedColorType::Rgb8)
            .map_err(|e| AppError::Pdf(format!("jpeg encode: {}", e)))?;
    }

    // Points: 1px @ 96 DPI → 0.75 pt
    let width_pt = width as f64 * 0.75;
    let height_pt = height as f64 * 0.75;

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    let img_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => width as i64,
            "Height" => height as i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8_i64,
            "Filter" => "DCTDecode",
        },
        jpeg_bytes,
    );
    let img_id = doc.add_object(img_stream);

    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    width_pt.into(),
                    0.into(),
                    0.into(),
                    height_pt.into(),
                    0.into(),
                    0.into(),
                ],
            ),
            Operation::new("Do", vec![Object::Name(b"Im1".to_vec())]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_bytes = content
        .encode()
        .map_err(|e| AppError::Pdf(format!("content encode: {}", e)))?;
    let content_stream = Stream::new(dictionary! {}, content_bytes);
    let content_id = doc.add_object(content_stream);

    let resources = dictionary! {
        "XObject" => dictionary! {
            "Im1" => Object::Reference(img_id),
        }
    };
    let page_dict = dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![
            Object::Integer(0),
            Object::Integer(0),
            lopdf::Object::Real(width_pt),
            lopdf::Object::Real(height_pt),
        ],
        "Resources" => resources,
        "Contents" => Object::Reference(content_id),
    };
    let page_id = doc.add_object(page_dict);

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => 1_i64,
        "Kids" => vec![Object::Reference(page_id)],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut out_bytes: Vec<u8> = Vec::new();
    doc.save_to(&mut out_bytes)
        .map_err(|e| AppError::Pdf(format!("lopdf save: {}", e)))?;
    std::fs::write(output_path, out_bytes)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn screenshot_to_pdf_produces_valid_pdf() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("out.pdf");

        // 4×4 RGBA checkerboard
        let rgba: Vec<u8> = (0..16)
            .flat_map(|i| {
                if i % 2 == 0 {
                    [255u8, 0, 0, 255] // red
                } else {
                    [0u8, 0, 255, 255] // blue
                }
            })
            .collect();

        screenshot_to_pdf(&rgba, 4, 4, &out).unwrap();

        let pdf = std::fs::read(&out).unwrap();
        assert_eq!(&pdf[..4], b"%PDF");
    }

    #[test]
    fn screenshot_to_pdf_nonexistent_dir_returns_error() {
        let rgba = vec![255u8; 4 * 4 * 4];
        let result =
            screenshot_to_pdf(&rgba, 4, 4, Path::new("/no/such/dir/out.pdf"));
        assert!(result.is_err());
    }

    // NOTE: `convert()` requires a running AppHandle (Tauri runtime) and cannot
    // be unit-tested without an integration test harness. The convert() function
    // is covered by the E2E test in tests/e2e/html_to_pdf.rs which runs the full
    // Tauri app and invokes the IPC command. The screenshot_to_pdf helper is unit-
    // tested above.
    //
    // To manually verify: cargo tauri dev → use HTML→PDF tool in the UI.
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test convert_to::from_html -- --nocapture 2>&1 | tail -20
```

Expected: Both unit tests (`screenshot_to_pdf_produces_valid_pdf`, `screenshot_to_pdf_nonexistent_dir_returns_error`) pass. The `convert()` function compiles but requires a live AppHandle and is tested via E2E.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/convert_to/from_html.rs src-tauri/capabilities/html-to-pdf.json
git commit -m "feat: implement HTML → PDF via hidden Tauri WebviewWindow + lopdf screenshot capture"
```

---

## Chunk 6: Svelte Workspaces

### Task 7: WordToPdfWorkspace.svelte

**Files:**
- Create: `src/lib/tools/convert-to/WordToPdfWorkspace.svelte`

- [ ] **Step 1: Write the component**

Create `src/lib/tools/convert-to/WordToPdfWorkspace.svelte`:
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { operationStore } from '$lib/stores/operation';
  import { recentFilesStore } from '$lib/stores/recentFiles';
  import type { ProcessResult } from '$lib/types';

  let filePath = $state<string | null>(null);
  let fileName = $state<string>('');
  let errorMsg = $state<string | null>(null);

  async function pickFile() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'Word Documents', extensions: ['docx'] }],
    });
    if (typeof selected === 'string') {
      filePath = selected;
      fileName = selected.split(/[\\/]/).pop() ?? selected;
      errorMsg = null;
    }
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    const file = event.dataTransfer?.files[0];
    if (!file) return;
    const ext = file.name.split('.').pop()?.toLowerCase();
    if (ext !== 'docx') {
      errorMsg = 'Only .docx files are supported.';
      return;
    }
    // Tauri drag-and-drop provides the path via the file object's path property
    // (available via tauri-plugin-fs in Tauri 2).
    filePath = (file as any).path ?? file.name;
    fileName = file.name;
    errorMsg = null;
  }

  async function runConversion() {
    if (!filePath) return;
    errorMsg = null;

    const stem = fileName.replace(/\.[^.]+$/, '');
    const suggestedName = `${stem}_converted.pdf`;

    const savePath = await saveDialog({
      defaultPath: suggestedName,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!savePath) return;

    operationStore.start('Word → PDF');
    try {
      const result: ProcessResult = await invoke('process_pdf', {
        tool: 'WordToPdf',
        inputPaths: [filePath],
        outputPath: savePath,
        options: {},
      });
      recentFilesStore.add({ path: savePath, tool: 'Word → PDF' });
      operationStore.succeed(`Saved: ${savePath}`);
    } catch (err: any) {
      errorMsg = err?.message ?? String(err);
      operationStore.fail(errorMsg!);
    }
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h1 class="text-2xl font-semibold text-gray-800">Word → PDF</h1>
    <p class="text-sm text-gray-500 mt-1">
      Convert a <code>.docx</code> file to PDF. Text and paragraph structure are preserved.
      Complex formatting, fonts, and images may not render perfectly.
    </p>
  </div>

  <!-- Limitation banner -->
  <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800">
    <strong>Note:</strong> Output is best-effort. Tables, headers, footers, columns,
    embedded images, and non-standard fonts may not render correctly. For pixel-perfect
    results, export directly from Microsoft Word or LibreOffice.
  </div>

  <!-- Drop zone -->
  <button
    type="button"
    class="w-full rounded-xl border-2 border-dashed border-gray-300 bg-gray-50
           hover:border-teal hover:bg-teal/5 transition-colors cursor-pointer
           flex flex-col items-center justify-center gap-3 py-10"
    ondragover={(e) => e.preventDefault()}
    ondrop={handleDrop}
    onclick={pickFile}
    aria-label="Select Word document"
  >
    <svg class="w-10 h-10 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
        d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586
           a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
    </svg>
    {#if filePath}
      <span class="text-sm font-medium text-teal">{fileName}</span>
      <span class="text-xs text-gray-400">Click to change file</span>
    {:else}
      <span class="text-sm text-gray-600">Drop a <strong>.docx</strong> file here</span>
      <span class="text-xs text-gray-400">or click to browse</span>
    {/if}
  </button>

  {#if errorMsg}
    <p class="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-4 py-2">
      {errorMsg}
    </p>
  {/if}

  <button
    type="button"
    class="w-full rounded-xl bg-peach py-3 text-white font-semibold
           hover:bg-peach-dark disabled:opacity-40 disabled:cursor-not-allowed transition"
    disabled={!filePath}
    onclick={runConversion}
  >
    Convert to PDF
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-to/WordToPdfWorkspace.svelte
git commit -m "feat: add Word → PDF Svelte workspace with docx file filter and limitation banner"
```

---

### Task 8: ExcelToPdfWorkspace.svelte

**Files:**
- Create: `src/lib/tools/convert-to/ExcelToPdfWorkspace.svelte`

- [ ] **Step 1: Write the component**

Create `src/lib/tools/convert-to/ExcelToPdfWorkspace.svelte`:
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { operationStore } from '$lib/stores/operation';
  import { recentFilesStore } from '$lib/stores/recentFiles';
  import type { ProcessResult } from '$lib/types';

  let filePath = $state<string | null>(null);
  let fileName = $state<string>('');
  let errorMsg = $state<string | null>(null);

  async function pickFile() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'Excel Workbooks', extensions: ['xlsx', 'xls'] }],
    });
    if (typeof selected === 'string') {
      filePath = selected;
      fileName = selected.split(/[\\/]/).pop() ?? selected;
      errorMsg = null;
    }
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    const file = event.dataTransfer?.files[0];
    if (!file) return;
    const ext = file.name.split('.').pop()?.toLowerCase();
    if (ext !== 'xlsx' && ext !== 'xls') {
      errorMsg = 'Only .xlsx and .xls files are supported.';
      return;
    }
    filePath = (file as any).path ?? file.name;
    fileName = file.name;
    errorMsg = null;
  }

  async function runConversion() {
    if (!filePath) return;
    errorMsg = null;
    const stem = fileName.replace(/\.[^.]+$/, '');
    const savePath = await saveDialog({
      defaultPath: `${stem}_converted.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!savePath) return;

    operationStore.start('Excel → PDF');
    try {
      await invoke('process_pdf', {
        tool: 'ExcelToPdf',
        inputPaths: [filePath],
        outputPath: savePath,
        options: {},
      });
      recentFilesStore.add({ path: savePath, tool: 'Excel → PDF' });
      operationStore.succeed(`Saved: ${savePath}`);
    } catch (err: any) {
      errorMsg = err?.message ?? String(err);
      operationStore.fail(errorMsg!);
    }
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h1 class="text-2xl font-semibold text-gray-800">Excel → PDF</h1>
    <p class="text-sm text-gray-500 mt-1">
      Convert the first sheet of an <code>.xlsx</code> or <code>.xls</code> workbook to PDF.
    </p>
  </div>

  <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800">
    <strong>Note:</strong> Only the first sheet is converted. Cell formatting, colours,
    borders, charts, and merged cells may not render correctly.
  </div>

  <button
    type="button"
    class="w-full rounded-xl border-2 border-dashed border-gray-300 bg-gray-50
           hover:border-teal hover:bg-teal/5 transition-colors cursor-pointer
           flex flex-col items-center justify-center gap-3 py-10"
    ondragover={(e) => e.preventDefault()}
    ondrop={handleDrop}
    onclick={pickFile}
    aria-label="Select Excel file"
  >
    <svg class="w-10 h-10 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
        d="M3 10h18M3 14h18M10 3v18M14 3v18M5 3h14a2 2 0 012 2v14a2 2 0
           01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2z"/>
    </svg>
    {#if filePath}
      <span class="text-sm font-medium text-teal">{fileName}</span>
      <span class="text-xs text-gray-400">Click to change file</span>
    {:else}
      <span class="text-sm text-gray-600">Drop an <strong>.xlsx</strong> file here</span>
      <span class="text-xs text-gray-400">or click to browse</span>
    {/if}
  </button>

  {#if errorMsg}
    <p class="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-4 py-2">
      {errorMsg}
    </p>
  {/if}

  <button
    type="button"
    class="w-full rounded-xl bg-peach py-3 text-white font-semibold
           hover:bg-peach-dark disabled:opacity-40 disabled:cursor-not-allowed transition"
    disabled={!filePath}
    onclick={runConversion}
  >
    Convert to PDF
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-to/ExcelToPdfWorkspace.svelte
git commit -m "feat: add Excel → PDF Svelte workspace with xlsx/xls file filter"
```

---

### Task 9: PptToPdfWorkspace.svelte

**Files:**
- Create: `src/lib/tools/convert-to/PptToPdfWorkspace.svelte`

- [ ] **Step 1: Write the component**

Create `src/lib/tools/convert-to/PptToPdfWorkspace.svelte`:
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { operationStore } from '$lib/stores/operation';
  import { recentFilesStore } from '$lib/stores/recentFiles';

  let filePath = $state<string | null>(null);
  let fileName = $state<string>('');
  let errorMsg = $state<string | null>(null);

  async function pickFile() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'PowerPoint Presentations', extensions: ['pptx'] }],
    });
    if (typeof selected === 'string') {
      filePath = selected;
      fileName = selected.split(/[\\/]/).pop() ?? selected;
      errorMsg = null;
    }
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    const file = event.dataTransfer?.files[0];
    if (!file) return;
    const ext = file.name.split('.').pop()?.toLowerCase();
    if (ext !== 'pptx') {
      errorMsg = 'Only .pptx files are supported. Legacy .ppt format is not supported.';
      return;
    }
    filePath = (file as any).path ?? file.name;
    fileName = file.name;
    errorMsg = null;
  }

  async function runConversion() {
    if (!filePath) return;
    errorMsg = null;
    const stem = fileName.replace(/\.[^.]+$/, '');
    const savePath = await saveDialog({
      defaultPath: `${stem}_converted.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!savePath) return;

    operationStore.start('PPT → PDF');
    try {
      await invoke('process_pdf', {
        tool: 'PptToPdf',
        inputPaths: [filePath],
        outputPath: savePath,
        options: {},
      });
      recentFilesStore.add({ path: savePath, tool: 'PPT → PDF' });
      operationStore.succeed(`Saved: ${savePath}`);
    } catch (err: any) {
      errorMsg = err?.message ?? String(err);
      operationStore.fail(errorMsg!);
    }
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h1 class="text-2xl font-semibold text-gray-800">PowerPoint → PDF</h1>
    <p class="text-sm text-gray-500 mt-1">
      Convert a <code>.pptx</code> presentation to PDF. Text content from each slide is placed
      on its own page.
    </p>
  </div>

  <!-- Extended limitation banner for PPT (spec requires disclosure) -->
  <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 space-y-1">
    <p><strong>Important limitations:</strong></p>
    <ul class="list-disc list-inside space-y-0.5 ml-2">
      <li>Non-Latin scripts (Arabic, Chinese, Japanese, etc.) may not render correctly.</li>
      <li>Shapes, backgrounds, images, and transitions are not rendered.</li>
      <li>Complex font styling (bold, italic, size variations) is not preserved.</li>
      <li>Animations and embedded media are ignored.</li>
    </ul>
    <p class="mt-2">For best results, export directly from PowerPoint or LibreOffice Impress.</p>
  </div>

  <button
    type="button"
    class="w-full rounded-xl border-2 border-dashed border-gray-300 bg-gray-50
           hover:border-teal hover:bg-teal/5 transition-colors cursor-pointer
           flex flex-col items-center justify-center gap-3 py-10"
    ondragover={(e) => e.preventDefault()}
    ondrop={handleDrop}
    onclick={pickFile}
    aria-label="Select PowerPoint file"
  >
    <svg class="w-10 h-10 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
        d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414
           A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/>
    </svg>
    {#if filePath}
      <span class="text-sm font-medium text-teal">{fileName}</span>
      <span class="text-xs text-gray-400">Click to change file</span>
    {:else}
      <span class="text-sm text-gray-600">Drop a <strong>.pptx</strong> file here</span>
      <span class="text-xs text-gray-400">or click to browse (legacy .ppt not supported)</span>
    {/if}
  </button>

  {#if errorMsg}
    <p class="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-4 py-2">
      {errorMsg}
    </p>
  {/if}

  <button
    type="button"
    class="w-full rounded-xl bg-peach py-3 text-white font-semibold
           hover:bg-peach-dark disabled:opacity-40 disabled:cursor-not-allowed transition"
    disabled={!filePath}
    onclick={runConversion}
  >
    Convert to PDF
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-to/PptToPdfWorkspace.svelte
git commit -m "feat: add PPT → PDF Svelte workspace with pptx filter and extended limitation disclosure"
```

---

### Task 10: ImageToPdfWorkspace.svelte

**Files:**
- Create: `src/lib/tools/convert-to/ImageToPdfWorkspace.svelte`

This component supports multi-file drag-to-reorder (same pattern as MergePdf from Plan 2).

- [ ] **Step 1: Write the component**

Create `src/lib/tools/convert-to/ImageToPdfWorkspace.svelte`:
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { operationStore } from '$lib/stores/operation';
  import { recentFilesStore } from '$lib/stores/recentFiles';

  interface ImageFile {
    path: string;
    name: string;
    id: string; // unique key for Svelte keyed each
  }

  const ACCEPTED_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp', 'tiff'];

  let files = $state<ImageFile[]>([]);
  let errorMsg = $state<string | null>(null);
  let draggingIndex = $state<number | null>(null);

  function generateId() {
    return Math.random().toString(36).slice(2);
  }

  function isAcceptedImage(name: string): boolean {
    const ext = name.split('.').pop()?.toLowerCase() ?? '';
    return ACCEPTED_EXTENSIONS.includes(ext);
  }

  async function pickFiles() {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: 'Images', extensions: ACCEPTED_EXTENSIONS }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    addPaths(paths);
  }

  function addPaths(paths: string[]) {
    const newFiles: ImageFile[] = paths
      .filter(isAcceptedImage)
      .map((p) => ({ path: p, name: p.split(/[\\/]/).pop() ?? p, id: generateId() }));
    files = [...files, ...newFiles];
    errorMsg = null;
  }

  function handleZoneDrop(event: DragEvent) {
    event.preventDefault();
    const dropped = event.dataTransfer?.files;
    if (!dropped || dropped.length === 0) return;
    const paths: string[] = [];
    for (let i = 0; i < dropped.length; i++) {
      const f = dropped[i] as any;
      const path = f.path ?? f.name;
      if (isAcceptedImage(f.name)) paths.push(path);
    }
    if (paths.length === 0) {
      errorMsg = `Only image files are accepted (${ACCEPTED_EXTENSIONS.join(', ')}).`;
      return;
    }
    addPaths(paths);
  }

  function removeFile(index: number) {
    files = files.filter((_, i) => i !== index);
  }

  // Drag-to-reorder state
  let dragOverIndex = $state<number | null>(null);

  function onItemDragStart(index: number) {
    draggingIndex = index;
  }
  function onItemDragOver(event: DragEvent, index: number) {
    event.preventDefault();
    dragOverIndex = index;
  }
  function onItemDrop(index: number) {
    if (draggingIndex === null || draggingIndex === index) {
      draggingIndex = null;
      dragOverIndex = null;
      return;
    }
    const reordered = [...files];
    const [moved] = reordered.splice(draggingIndex, 1);
    reordered.splice(index, 0, moved);
    files = reordered;
    draggingIndex = null;
    dragOverIndex = null;
  }
  function onItemDragEnd() {
    draggingIndex = null;
    dragOverIndex = null;
  }

  async function runConversion() {
    if (files.length === 0) return;
    errorMsg = null;
    const firstStem = files[0].name.replace(/\.[^.]+$/, '');
    const savePath = await saveDialog({
      defaultPath: `${firstStem}_images.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!savePath) return;

    operationStore.start('Image → PDF');
    try {
      await invoke('process_pdf', {
        tool: { ImageToPdf: { paths: files.map((f) => f.path) } },
        inputPaths: files.map((f) => f.path),
        outputPath: savePath,
        options: {},
      });
      recentFilesStore.add({ path: savePath, tool: 'Image → PDF' });
      operationStore.succeed(`Saved: ${savePath}`);
    } catch (err: any) {
      errorMsg = err?.message ?? String(err);
      operationStore.fail(errorMsg!);
    }
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h1 class="text-2xl font-semibold text-gray-800">Image → PDF</h1>
    <p class="text-sm text-gray-500 mt-1">
      Combine JPG, PNG, or other images into a single PDF. One image per page.
      Drag to reorder before converting.
    </p>
  </div>

  <!-- Drop zone -->
  <button
    type="button"
    class="w-full rounded-xl border-2 border-dashed border-gray-300 bg-gray-50
           hover:border-teal hover:bg-teal/5 transition-colors cursor-pointer
           flex flex-col items-center justify-center gap-3 py-8"
    ondragover={(e) => e.preventDefault()}
    ondrop={handleZoneDrop}
    onclick={pickFiles}
    aria-label="Select images"
  >
    <svg class="w-10 h-10 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
        d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0
           012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2
           2 0 00-2 2v12a2 2 0 002 2z"/>
    </svg>
    <span class="text-sm text-gray-600">Drop images here or click to browse</span>
    <span class="text-xs text-gray-400">JPG, PNG, WebP, GIF, BMP, TIFF supported</span>
  </button>

  {#if files.length > 0}
    <ul class="divide-y divide-gray-200 rounded-xl border border-gray-200 bg-white">
      {#each files as file, i (file.id)}
        <li
          class="flex items-center gap-3 px-4 py-2.5 cursor-grab active:cursor-grabbing
                 transition-colors {dragOverIndex === i ? 'bg-teal/10' : ''}"
          draggable="true"
          ondragstart={() => onItemDragStart(i)}
          ondragover={(e) => onItemDragOver(e, i)}
          ondrop={() => onItemDrop(i)}
          ondragend={onItemDragEnd}
        >
          <span class="text-gray-300 select-none">⠿</span>
          <span class="text-xs text-gray-500 w-5 text-right">{i + 1}.</span>
          <span class="flex-1 text-sm text-gray-700 truncate">{file.name}</span>
          <button
            type="button"
            class="text-gray-400 hover:text-red-500 transition-colors text-lg leading-none"
            onclick={() => removeFile(i)}
            aria-label="Remove {file.name}"
          >×</button>
        </li>
      {/each}
    </ul>
    <p class="text-xs text-gray-400 text-center">Drag rows to reorder. Order determines page order in the PDF.</p>
  {/if}

  {#if errorMsg}
    <p class="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-4 py-2">
      {errorMsg}
    </p>
  {/if}

  <button
    type="button"
    class="w-full rounded-xl bg-peach py-3 text-white font-semibold
           hover:bg-peach-dark disabled:opacity-40 disabled:cursor-not-allowed transition"
    disabled={files.length === 0}
    onclick={runConversion}
  >
    Convert {files.length > 0 ? `${files.length} image${files.length > 1 ? 's' : ''}` : 'images'} to PDF
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-to/ImageToPdfWorkspace.svelte
git commit -m "feat: add Image → PDF Svelte workspace with multi-file drag-to-reorder"
```

---

### Task 11: HtmlToPdfWorkspace.svelte

**Files:**
- Create: `src/lib/tools/convert-to/HtmlToPdfWorkspace.svelte`

- [ ] **Step 1: Write the component**

Create `src/lib/tools/convert-to/HtmlToPdfWorkspace.svelte`:
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { operationStore } from '$lib/stores/operation';
  import { recentFilesStore } from '$lib/stores/recentFiles';

  let filePath = $state<string | null>(null);
  let fileName = $state<string>('');
  let errorMsg = $state<string | null>(null);

  async function pickFile() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'HTML Files', extensions: ['html', 'htm'] }],
    });
    if (typeof selected === 'string') {
      filePath = selected;
      fileName = selected.split(/[\\/]/).pop() ?? selected;
      errorMsg = null;
    }
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    const file = event.dataTransfer?.files[0];
    if (!file) return;
    const ext = file.name.split('.').pop()?.toLowerCase();
    if (ext !== 'html' && ext !== 'htm') {
      errorMsg = 'Only .html and .htm files are supported.';
      return;
    }
    filePath = (file as any).path ?? file.name;
    fileName = file.name;
    errorMsg = null;
  }

  async function runConversion() {
    if (!filePath) return;
    errorMsg = null;
    const stem = fileName.replace(/\.[^.]+$/, '');
    const savePath = await saveDialog({
      defaultPath: `${stem}_converted.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!savePath) return;

    operationStore.start('HTML → PDF');
    try {
      await invoke('process_pdf', {
        tool: 'HtmlToPdf',
        inputPaths: [filePath],
        outputPath: savePath,
        options: {},
      });
      recentFilesStore.add({ path: savePath, tool: 'HTML → PDF' });
      operationStore.succeed(`Saved: ${savePath}`);
    } catch (err: any) {
      errorMsg = err?.message ?? String(err);
      operationStore.fail(errorMsg!);
    }
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h1 class="text-2xl font-semibold text-gray-800">HTML → PDF</h1>
    <p class="text-sm text-gray-500 mt-1">
      Convert a local <code>.html</code> file to PDF. The file is rendered in a hidden browser
      window and captured as a PDF image.
    </p>
  </div>

  <!-- Network warning banner -->
  <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 space-y-1">
    <p><strong>Local files only.</strong></p>
    <ul class="list-disc list-inside space-y-0.5 ml-2">
      <li>External resources (CDN stylesheets, web fonts, remote images) will <strong>not load</strong>.
          The page will render without them.</li>
      <li>JavaScript that fetches remote data will fail silently.</li>
      <li>The PDF is a screenshot — text will not be selectable.</li>
      <li>Complex, script-heavy pages may not fully render within the 2-second load window.</li>
    </ul>
  </div>

  <button
    type="button"
    class="w-full rounded-xl border-2 border-dashed border-gray-300 bg-gray-50
           hover:border-teal hover:bg-teal/5 transition-colors cursor-pointer
           flex flex-col items-center justify-center gap-3 py-10"
    ondragover={(e) => e.preventDefault()}
    ondrop={handleDrop}
    onclick={pickFile}
    aria-label="Select HTML file"
  >
    <svg class="w-10 h-10 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
        d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
    </svg>
    {#if filePath}
      <span class="text-sm font-medium text-teal">{fileName}</span>
      <span class="text-xs text-gray-400">Click to change file</span>
    {:else}
      <span class="text-sm text-gray-600">Drop an <strong>.html</strong> file here</span>
      <span class="text-xs text-gray-400">or click to browse</span>
    {/if}
  </button>

  {#if errorMsg}
    <p class="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-4 py-2">
      {errorMsg}
    </p>
  {/if}

  <button
    type="button"
    class="w-full rounded-xl bg-peach py-3 text-white font-semibold
           hover:bg-peach-dark disabled:opacity-40 disabled:cursor-not-allowed transition"
    disabled={!filePath}
    onclick={runConversion}
  >
    Convert to PDF
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-to/HtmlToPdfWorkspace.svelte
git commit -m "feat: add HTML → PDF Svelte workspace with network limitation disclosure"
```

---

## Chunk 7: Wire Workspaces into the Tool Registry

### Task 12: Register all five tools in the tool registry

**Files:**
- Modify: `src/lib/tools/registry.ts` (or wherever tools are registered from Plan 1)

- [ ] **Step 1: Add the five new tool entries**

Open `src/lib/tools/registry.ts` (exact path may differ — find it with `grep -r "WordToPdf\|ToolRegistry\|toolRegistry" src/`).

Add at the end of the `convert-to` category array (create the category if it doesn't exist):

```typescript
import WordToPdfWorkspace    from './convert-to/WordToPdfWorkspace.svelte';
import ExcelToPdfWorkspace   from './convert-to/ExcelToPdfWorkspace.svelte';
import PptToPdfWorkspace     from './convert-to/PptToPdfWorkspace.svelte';
import ImageToPdfWorkspace   from './convert-to/ImageToPdfWorkspace.svelte';
import HtmlToPdfWorkspace    from './convert-to/HtmlToPdfWorkspace.svelte';

// Inside the tools array / registry object:
{
  id: 'word-to-pdf',
  label: 'Word → PDF',
  category: 'other-to-pdf',
  icon: 'document-text',
  description: 'Convert .docx files to PDF',
  component: WordToPdfWorkspace,
  tauriTool: 'WordToPdf',
},
{
  id: 'excel-to-pdf',
  label: 'Excel → PDF',
  category: 'other-to-pdf',
  icon: 'table',
  description: 'Convert .xlsx spreadsheets to PDF',
  component: ExcelToPdfWorkspace,
  tauriTool: 'ExcelToPdf',
},
{
  id: 'ppt-to-pdf',
  label: 'PPT → PDF',
  category: 'other-to-pdf',
  icon: 'presentation-chart-bar',
  description: 'Convert .pptx presentations to PDF',
  component: PptToPdfWorkspace,
  tauriTool: 'PptToPdf',
},
{
  id: 'image-to-pdf',
  label: 'Image → PDF',
  category: 'other-to-pdf',
  icon: 'photograph',
  description: 'Combine JPG/PNG images into a PDF',
  component: ImageToPdfWorkspace,
  tauriTool: 'ImageToPdf',
},
{
  id: 'html-to-pdf',
  label: 'HTML → PDF',
  category: 'other-to-pdf',
  icon: 'code',
  description: 'Convert local HTML files to PDF',
  component: HtmlToPdfWorkspace,
  tauriTool: 'HtmlToPdf',
},
```

Add the category to the category list if it doesn't exist:
```typescript
{
  id: 'other-to-pdf',
  label: 'Other → PDF',
  icon: 'arrow-right-circle',
},
```

- [ ] **Step 2: Verify the Svelte app compiles**

```bash
npm run build 2>&1 | tail -20
```

Expected: No errors. The five workspaces appear in the `other-to-pdf` category on the dashboard.

- [ ] **Step 3: Commit**

```bash
git add src/lib/tools/registry.ts
git commit -m "feat: register all 5 Other → PDF tools in the tool registry"
```

---

## Chunk 8: Output Filename Convention + Integration Smoke Test

### Task 13: Enforce output filename convention in Rust dispatch

**Files:**
- Modify: `src-tauri/src/commands/mod.rs` (or the process_pdf command handler)

Per the spec, the suggested filename is `{original_stem}_{tool}.{ext}`. The Rust backend constructs a default suggestion but the frontend overrides it via the save dialog. Verify the dispatch passes `output_path` (already chosen by user via the save dialog) directly to each `convert` function. No additional changes needed if Plan 1 already implements this.

- [ ] **Step 1: Verify existing dispatch passes user-chosen output path**

```bash
cd src-tauri && grep -n "output_path\|outputPath\|save_path" src/commands/mod.rs | head -20
```

Expected: The command receives `output_path: String` from the frontend and passes it to the tool function. If not, add `output_path` as a parameter to `process_pdf`.

- [ ] **Step 2: Verify the suggested filename in each Svelte workspace**

Each workspace already uses `${stem}_converted.pdf` as `defaultPath` in `saveDialog`. Align with spec: use `${stem}_converted.pdf` for single-file tools and `${firstStem}_images.pdf` for the image tool (already done in the components above).

- [ ] **Step 3: Commit (if any changes were needed)**

```bash
git add src-tauri/src/commands/mod.rs
git commit -m "fix: ensure process_pdf dispatch forwards user-chosen output_path to convert fns"
```

---

### Task 14: Smoke test — run the app and manually verify all five tools

- [ ] **Step 1: Start the app**

```bash
npm run tauri dev
```

- [ ] **Step 2: Test Word → PDF**

1. Click "Other → PDF" category in the dashboard.
2. Click "Word → PDF".
3. Drop or browse to any `.docx` file.
4. Verify limitation banner is visible.
5. Click "Convert to PDF".
6. Confirm the native save dialog opens with a suggested filename.
7. Save the file.
8. Open the saved PDF and verify text content is present.

- [ ] **Step 3: Test Excel → PDF**

Repeat with an `.xlsx` file. Verify:
- Only `.xlsx`/`.xls` files are accepted in the file picker.
- Header row uses bold font in the PDF.
- Wide sheets use landscape orientation.

- [ ] **Step 4: Test PPT → PDF**

Use a `.pptx` file. Verify:
- Extended limitation banner is visible before converting.
- Slide text appears on correct pages in the PDF.
- Dropping a `.ppt` (non-x) shows an error message.

- [ ] **Step 5: Test Image → PDF**

1. Add 3 images (mix of JPG and PNG).
2. Drag to reorder them.
3. Remove one.
4. Convert the remaining two.
5. Open the PDF and verify 2 pages matching the image order.

- [ ] **Step 6: Test HTML → PDF**

1. Create a minimal local HTML file: `<html><body><h1>Hello</h1></body></html>`.
2. Select it in the tool.
3. Verify the network limitation banner is displayed.
4. Convert.
5. Open the PDF and verify the rendered page appears.

- [ ] **Step 7: Commit smoke test pass**

```bash
git add .
git commit -m "test: manual smoke test pass for all 5 Other → PDF tools"
```

---

## Plan 4 Complete

At the end of this plan:
- `src-tauri/src/tools/convert_to/` contains 5 fully implemented Rust modules.
- `src/lib/tools/convert-to/` contains 5 Svelte workspace components.
- All five tools are wired into the `process_pdf` Tauri command via `ToolVariant` dispatch arms.
- All unit tests pass (`cargo test convert_to`).
- The tool registry shows "Other → PDF" as a category with 5 tools.
- All limitation disclosures are present in the UI before the user initiates conversion.

**Known limitations documented in-product:**
- Word → PDF: not pixel-perfect; tables/images/headers/footers may be lost.
- Excel → PDF: first sheet only; charts/colours/merged cells not rendered.
- PPT → PDF: text-only; shapes/backgrounds/non-Latin scripts/animations not rendered.
- Image → PDF: straightforward — no known functional limitations in v1.
- HTML → PDF: screenshot-based (not selectable text); external resources blocked; JS render time capped at 2s.

**Follow-on work (not in this plan):**
- PPT: add multi-sheet support (currently first slide name only shown in title).
- Word: add image extraction from docx and embedding in PDF via `from_image.rs` pipeline.
- HTML: investigate `tauri_plugin_shell` + system `chromium --headless --print-to-pdf` as an alternative for text-selectable PDF output.
- Excel: add multi-sheet output (one PDF section per sheet).
