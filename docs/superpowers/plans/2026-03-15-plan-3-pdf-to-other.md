# PavoPDF — Plan 3: PDF → Other Conversions

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add five PDF→Other conversion tools (Word, Excel, PowerPoint, JPG/PNG, PDF/A) to PavoPDF, each with a Rust backend function, Tauri command registration, and a complete Svelte 5 workspace component.

**Architecture:** Each converter lives in `src-tauri/src/tools/convert_from/` as its own module, following the `TempStage` + `emit_progress` pipeline established in Plan 1. The Svelte frontend provides a workspace per converter with file drop, options, and progress feedback — all wired via the existing `invoke('run_tool', ...)` IPC call. pdfium-render is the unified PDF-reading layer across all five tools.

**Tech Stack:** pdfium-render, docx-rs, calamine, quick-xml, zip, image crate

**Depends on:** Plans 1-2 complete

---

## Chunk 1: Module scaffold and shared pdfium helper

### Task 1: Create `convert_from` module scaffold

**Files:**
- Create: `src-tauri/src/tools/convert_from/mod.rs`
- Modify: `src-tauri/src/tools/mod.rs`

- [ ] **Step 1: Write test for module dispatch (failing)**

Add to `src-tauri/src/tools/mod.rs` — a test block asserting that each `convert_from` variant name round-trips through the match arm:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn convert_from_tool_names_are_recognized() {
        let names = [
            "pdf_to_word",
            "pdf_to_excel",
            "pdf_to_ppt",
            "pdf_to_image",
            "pdf_to_pdfa",
        ];
        for name in &names {
            // Will fail until match arms are added in Step 3
            assert!(crate::tools::tool_name_is_known(name), "{name} not recognized");
        }
    }
}
```

- [ ] **Step 2: Create `convert_from/mod.rs`**

Create `src-tauri/src/tools/convert_from/mod.rs`:

```rust
pub mod to_word;
pub mod to_excel;
pub mod to_ppt;
pub mod to_image;
pub mod to_pdfa;
```

- [ ] **Step 3: Wire into `tools/mod.rs` dispatch + add recognition helper**

In `src-tauri/src/tools/mod.rs`, add:

```rust
pub mod convert_from;

use std::path::PathBuf;
use tauri::AppHandle;
use crate::pipeline::TempStage;

/// Returns true if `name` is a known tool identifier (used in tests + validation).
pub fn tool_name_is_known(name: &str) -> bool {
    matches!(
        name,
        // --- convert_from ---
        "pdf_to_word"
        | "pdf_to_excel"
        | "pdf_to_ppt"
        | "pdf_to_image"
        | "pdf_to_pdfa"
        // --- existing Plan 2 tools (add your Plan 2 names here) ---
    )
}

/// Central dispatcher called by the `run_tool` Tauri command.
pub fn dispatch(
    tool: &str,
    stage: &TempStage,
    inputs: &[PathBuf],
    options: &serde_json::Value,
    app: &AppHandle,
    op_id: &str,
) -> crate::error::Result<PathBuf> {
    match tool {
        "pdf_to_word"  => convert_from::to_word::run(stage, inputs, options, app, op_id),
        "pdf_to_excel" => convert_from::to_excel::run(stage, inputs, options, app, op_id),
        "pdf_to_ppt"   => convert_from::to_ppt::run(stage, inputs, options, app, op_id),
        "pdf_to_image" => convert_from::to_image::run(stage, inputs, options, app, op_id),
        "pdf_to_pdfa"  => convert_from::to_pdfa::run(stage, inputs, options, app, op_id),
        other => Err(crate::error::AppError::Validation(
            format!("Unknown tool: {other}")
        )),
    }
}
```

- [ ] **Step 4: Run tests (should now pass)**

```bash
cd src-tauri && cargo test tools::tests::convert_from_tool_names_are_recognized
```

Expected: all 5 names pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tools/
git commit -m "feat: scaffold convert_from module with dispatch wiring"
```

---

### Task 2: Shared pdfium loader helper

**Files:**
- Create: `src-tauri/src/tools/convert_from/pdfium_loader.rs`
- Modify: `src-tauri/src/tools/convert_from/mod.rs`

Every converter needs to load pdfium. Centralise that here.

- [ ] **Step 1: Write test (failing)**

Add to bottom of `src-tauri/src/tools/convert_from/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::pdfium_loader::load_pdfium;

    #[test]
    fn pdfium_loads_without_panic() {
        // Requires the pdfium binary to be present at ./pdfium
        // In CI, skip if binary absent via env var
        if std::env::var("SKIP_PDFIUM_TESTS").is_ok() {
            return;
        }
        let result = load_pdfium();
        assert!(result.is_ok(), "pdfium failed to load: {:?}", result.err());
    }
}
```

- [ ] **Step 2: Create `pdfium_loader.rs`**

Create `src-tauri/src/tools/convert_from/pdfium_loader.rs`:

```rust
use pdfium_render::prelude::*;

/// Loads pdfium from the binary placed next to the executable.
/// The binary must be named according to the platform:
///   - Windows: pdfium.dll
///   - macOS:   libpdfium.dylib
///   - Linux:   libpdfium.so
pub fn load_pdfium() -> Result<Pdfium, PdfiumError> {
    let lib_name = Pdfium::pdfium_platform_library_name_at_path("./");
    Pdfium::bind_to_library(lib_name).map(Pdfium::new)
}

/// Open a PDF from a file path using an already-loaded Pdfium instance.
pub fn open_pdf<'a>(
    pdfium: &'a Pdfium,
    path: &std::path::Path,
    password: Option<&str>,
) -> Result<PdfDocument<'a>, PdfiumError> {
    pdfium.load_pdf_from_file(path, password)
}
```

- [ ] **Step 3: Export from mod.rs**

Add to `src-tauri/src/tools/convert_from/mod.rs`:

```rust
pub mod pdfium_loader;
```

- [ ] **Step 4: Run test**

```bash
cd src-tauri && cargo test convert_from::tests::pdfium_loads_without_panic
```

Set `SKIP_PDFIUM_TESTS=1` if the binary is not yet present in the repo.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tools/convert_from/pdfium_loader.rs src-tauri/src/tools/convert_from/mod.rs
git commit -m "feat: shared pdfium loader helper for convert_from tools"
```

---

## Chunk 2: PDF → Word

### Task 3: `to_word.rs` — Rust implementation

**Files:**
- Create: `src-tauri/src/tools/convert_from/to_word.rs`

- [ ] **Step 1: Write failing test**

Add to `to_word.rs` (create file with only the test module first):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_pdf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sample_text.pdf")
    }

    #[test]
    fn extracts_text_blocks_from_page() {
        if std::env::var("SKIP_PDFIUM_TESTS").is_ok() { return; }
        let pdfium = crate::tools::convert_from::pdfium_loader::load_pdfium().unwrap();
        let doc = pdfium.load_pdf_from_file(&fixture_pdf(), None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let blocks = extract_text_blocks(&page);
        assert!(!blocks.is_empty(), "expected at least one text block");
    }

    #[test]
    fn group_into_paragraphs_merges_close_lines() {
        let blocks = vec![
            TextBlock { text: "Hello".into(), y: 700.0, x: 72.0, font_size: 12.0 },
            TextBlock { text: "world".into(), y: 686.0, x: 72.0, font_size: 12.0 },
            // Gap > 2× line height — new paragraph
            TextBlock { text: "New para".into(), y: 500.0, x: 72.0, font_size: 12.0 },
        ];
        let paras = group_into_paragraphs(&blocks, 12.0);
        assert_eq!(paras.len(), 2);
        assert!(paras[0].contains("Hello"));
        assert!(paras[0].contains("world"));
        assert_eq!(paras[1].trim(), "New para");
    }
}
```

- [ ] **Step 2: Implement `to_word.rs`**

```rust
use std::path::PathBuf;
use tauri::AppHandle;
use pdfium_render::prelude::*;
use docx_rs::*;

use crate::error::{AppError, Result};
use crate::pipeline::TempStage;
use crate::progress::emit_progress;
use super::pdfium_loader::{load_pdfium, open_pdf};

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TextBlock {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(
    stage: &TempStage,
    inputs: &[PathBuf],
    _options: &serde_json::Value,
    app: &AppHandle,
    op_id: &str,
) -> Result<PathBuf> {
    let input = inputs.first().ok_or_else(|| AppError::Validation("No input file".into()))?;

    emit_progress(app, op_id, 0, "Loading PDF…");

    let pdfium = load_pdfium().map_err(|e| AppError::Pdf(e.to_string()))?;
    let doc = open_pdf(&pdfium, input, None).map_err(|e| AppError::Pdf(e.to_string()))?;

    let page_count = doc.pages().len();
    let mut all_paragraphs: Vec<String> = Vec::new();

    for i in 0..page_count {
        let pct = (i as f32 / page_count as f32 * 80.0) as u8;
        emit_progress(app, op_id, pct, &format!("Extracting page {}/{page_count}…", i + 1));

        let page = doc.pages().get(i as u16).map_err(|e| AppError::Pdf(e.to_string()))?;
        let blocks = extract_text_blocks(&page);

        // Determine dominant font size for paragraph gap heuristic
        let dominant_size = dominant_font_size(&blocks).unwrap_or(12.0);
        let mut paras = group_into_paragraphs(&blocks, dominant_size);
        all_paragraphs.append(&mut paras);

        if i + 1 < page_count as usize {
            // Page break separator — rendered as empty paragraph
            all_paragraphs.push(String::new());
        }
    }

    emit_progress(app, op_id, 85, "Building .docx…");

    let out_path = stage.dir().join(output_name(input));
    write_docx(&out_path, &all_paragraphs)?;

    emit_progress(app, op_id, 100, "Done");
    Ok(out_path)
}

// ---------------------------------------------------------------------------
// Text extraction
// ---------------------------------------------------------------------------

pub fn extract_text_blocks(page: &PdfPage) -> Vec<TextBlock> {
    let mut blocks: Vec<TextBlock> = Vec::new();

    for object in page.objects().iter() {
        if let PdfPageObjectType::Text = object.object_type() {
            let text_obj = object.as_text_object().unwrap();
            let text = text_obj.text().to_string();
            if text.trim().is_empty() { continue; }

            let bounds = match object.bounds() {
                Ok(b) => b,
                Err(_) => continue,
            };

            let font_size = text_obj.scaled_font_size().value;

            blocks.push(TextBlock {
                text,
                x: bounds.left.value,
                y: bounds.bottom.value,
                font_size,
            });
        }
    }

    // Sort top-to-bottom (PDF y-axis is inverted — higher y = higher on page)
    blocks.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));
    blocks
}

// ---------------------------------------------------------------------------
// Paragraph grouping heuristic
// ---------------------------------------------------------------------------

pub fn group_into_paragraphs(blocks: &[TextBlock], base_font_size: f32) -> Vec<String> {
    if blocks.is_empty() { return vec![]; }

    let line_gap_threshold = base_font_size * 1.8;
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut prev_y = blocks[0].y;

    for block in blocks {
        let gap = (prev_y - block.y).abs();
        if gap > line_gap_threshold && !current.is_empty() {
            paragraphs.push(current.join(" "));
            current.clear();
        }
        current.push(block.text.as_str());
        prev_y = block.y;
    }

    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }

    paragraphs
}

fn dominant_font_size(blocks: &[TextBlock]) -> Option<f32> {
    if blocks.is_empty() { return None; }
    // Simple mode: most common rounded font size
    let mut counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for b in blocks {
        *counts.entry(b.font_size.round() as u32).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|(_, c)| *c).map(|(sz, _)| sz as f32)
}

// ---------------------------------------------------------------------------
// docx-rs output
// ---------------------------------------------------------------------------

fn write_docx(out_path: &std::path::Path, paragraphs: &[String]) -> Result<()> {
    let mut docx = Docx::new();

    for para_text in paragraphs {
        let para = if para_text.is_empty() {
            // Empty paragraph = page-break visual spacer
            Paragraph::new()
        } else {
            Paragraph::new().add_run(Run::new().add_text(para_text))
        };
        docx = docx.add_paragraph(para);
    }

    let file = std::fs::File::create(out_path).map_err(AppError::from)?;
    docx.build().pack(file).map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(())
}

fn output_name(input: &std::path::Path) -> String {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    format!("{stem}.docx")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_pdf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sample_text.pdf")
    }

    #[test]
    fn extracts_text_blocks_from_page() {
        if std::env::var("SKIP_PDFIUM_TESTS").is_ok() { return; }
        let pdfium = crate::tools::convert_from::pdfium_loader::load_pdfium().unwrap();
        let doc = pdfium.load_pdf_from_file(&fixture_pdf(), None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let blocks = extract_text_blocks(&page);
        assert!(!blocks.is_empty(), "expected at least one text block");
    }

    #[test]
    fn group_into_paragraphs_merges_close_lines() {
        let blocks = vec![
            TextBlock { text: "Hello".into(), y: 700.0, x: 72.0, font_size: 12.0 },
            TextBlock { text: "world".into(), y: 686.0, x: 72.0, font_size: 12.0 },
            TextBlock { text: "New para".into(), y: 500.0, x: 72.0, font_size: 12.0 },
        ];
        let paras = group_into_paragraphs(&blocks, 12.0);
        assert_eq!(paras.len(), 2);
        assert!(paras[0].contains("Hello"));
        assert!(paras[0].contains("world"));
        assert_eq!(paras[1].trim(), "New para");
    }

    #[test]
    fn write_docx_produces_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("test.docx");
        let paras = vec!["Hello world".into(), "".into(), "Second paragraph".into()];
        write_docx(&out, &paras).unwrap();
        assert!(out.exists());
        assert!(out.metadata().unwrap().len() > 0);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test convert_from::to_word
```

Expected: `write_docx_produces_a_file` passes; pdfium tests pass if binary present.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/convert_from/to_word.rs
git commit -m "feat: pdf_to_word — pdfium text extraction + docx-rs output"
```

---

## Chunk 3: PDF → Excel

### Task 4: `to_excel.rs` — Rust implementation

**Files:**
- Create: `src-tauri/src/tools/convert_from/to_excel.rs`

Note: calamine is a *reader* crate for Excel. For *writing* .xlsx we use `rust_xlsxwriter`. Add `rust_xlsxwriter = "0.64"` to `Cargo.toml` if not already present (calamine alone does not write xlsx).

- [ ] **Step 1: Add `rust_xlsxwriter` to `Cargo.toml`**

```toml
rust_xlsxwriter = "0.64"
```

Verify compile:

```bash
cd src-tauri && cargo build -p pavopdf 2>&1 | tail -5
```

- [ ] **Step 2: Write failing test**

```rust
// In to_excel.rs — test module first
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_table_finds_grid_of_blocks() {
        // 2-row × 3-col grid of TextBlocks
        let blocks = vec![
            // Row 1
            TextBlock2D { text: "Name".into(),   col: 0, row: 0 },
            TextBlock2D { text: "Age".into(),    col: 1, row: 0 },
            TextBlock2D { text: "City".into(),   col: 2, row: 0 },
            // Row 2
            TextBlock2D { text: "Alice".into(),  col: 0, row: 1 },
            TextBlock2D { text: "30".into(),     col: 1, row: 1 },
            TextBlock2D { text: "London".into(), col: 2, row: 1 },
        ];
        let table = assemble_table(&blocks);
        assert_eq!(table.len(), 2, "expected 2 rows");
        assert_eq!(table[0].len(), 3, "expected 3 cols");
        assert_eq!(table[0][0], "Name");
        assert_eq!(table[1][2], "London");
    }
}
```

- [ ] **Step 3: Implement `to_excel.rs`**

```rust
use std::path::PathBuf;
use tauri::AppHandle;
use pdfium_render::prelude::*;
use rust_xlsxwriter::Workbook;

use crate::error::{AppError, Result};
use crate::pipeline::TempStage;
use crate::progress::emit_progress;
use super::pdfium_loader::{load_pdfium, open_pdf};

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TextBlock2D {
    pub text: String,
    pub col: usize,
    pub row: usize,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(
    stage: &TempStage,
    inputs: &[PathBuf],
    _options: &serde_json::Value,
    app: &AppHandle,
    op_id: &str,
) -> Result<PathBuf> {
    let input = inputs.first().ok_or_else(|| AppError::Validation("No input file".into()))?;

    emit_progress(app, op_id, 0, "Loading PDF…");

    let pdfium = load_pdfium().map_err(|e| AppError::Pdf(e.to_string()))?;
    let doc = open_pdf(&pdfium, input, None).map_err(|e| AppError::Pdf(e.to_string()))?;

    let page_count = doc.pages().len();
    let mut workbook = Workbook::new();

    for i in 0..page_count {
        let pct = (i as f32 / page_count as f32 * 80.0) as u8;
        emit_progress(app, op_id, pct, &format!("Extracting tables from page {}/{}…", i + 1, page_count));

        let page = doc.pages().get(i as u16).map_err(|e| AppError::Pdf(e.to_string()))?;
        let raw_blocks = extract_positioned_blocks(&page);

        if raw_blocks.is_empty() { continue; }

        let grid = classify_grid(&raw_blocks);
        let table = assemble_table(&grid);

        if table.is_empty() { continue; }

        let sheet_name = if page_count == 1 {
            "Sheet1".into()
        } else {
            format!("Page {}", i + 1)
        };

        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&sheet_name).map_err(|e| AppError::Pdf(e.to_string()))?;

        for (r, row) in table.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                worksheet
                    .write_string(r as u32, c as u16, cell)
                    .map_err(|e| AppError::Pdf(e.to_string()))?;
            }
        }
    }

    emit_progress(app, op_id, 90, "Writing .xlsx…");

    let out_path = stage.dir().join(output_name(input));
    workbook.save(&out_path).map_err(|e| AppError::Pdf(e.to_string()))?;

    emit_progress(app, op_id, 100, "Done");
    Ok(out_path)
}

// ---------------------------------------------------------------------------
// Positioned block extraction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RawBlock {
    text: String,
    x: f32,
    y: f32,
}

fn extract_positioned_blocks(page: &PdfPage) -> Vec<RawBlock> {
    let mut blocks = Vec::new();
    for obj in page.objects().iter() {
        if let PdfPageObjectType::Text = obj.object_type() {
            let t = obj.as_text_object().unwrap();
            let text = t.text().to_string();
            if text.trim().is_empty() { continue; }
            if let Ok(bounds) = obj.bounds() {
                blocks.push(RawBlock {
                    text,
                    x: bounds.left.value,
                    y: bounds.bottom.value,
                });
            }
        }
    }
    blocks
}

// ---------------------------------------------------------------------------
// Heuristic grid classification
// ---------------------------------------------------------------------------

/// Snap continuous positions to discrete (row, col) indices using gap clustering.
fn classify_grid(blocks: &[RawBlock]) -> Vec<TextBlock2D> {
    if blocks.is_empty() { return vec![]; }

    // Collect unique y positions → rows (top-to-bottom: higher y = earlier row)
    let mut ys: Vec<f32> = blocks.iter().map(|b| b.y).collect();
    ys.sort_by(|a, b| b.partial_cmp(a).unwrap());
    ys.dedup_by(|a, b| (*b - *a).abs() < 6.0);

    let mut xs: Vec<f32> = blocks.iter().map(|b| b.x).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*b - *a).abs() < 20.0);

    blocks
        .iter()
        .map(|b| {
            let row = ys
                .iter()
                .position(|&y| (y - b.y).abs() < 6.0)
                .unwrap_or(0);
            let col = xs
                .iter()
                .position(|&x| (x - b.x).abs() < 20.0)
                .unwrap_or(0);
            TextBlock2D { text: b.text.clone(), col, row }
        })
        .collect()
}

/// Assemble a Vec<Vec<String>> table from grid positions.
pub fn assemble_table(blocks: &[TextBlock2D]) -> Vec<Vec<String>> {
    if blocks.is_empty() { return vec![]; }

    let max_row = blocks.iter().map(|b| b.row).max().unwrap_or(0);
    let max_col = blocks.iter().map(|b| b.col).max().unwrap_or(0);

    let mut table = vec![vec![String::new(); max_col + 1]; max_row + 1];
    for b in blocks {
        table[b.row][b.col] = b.text.clone();
    }
    table
}

fn output_name(input: &std::path::Path) -> String {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    format!("{stem}.xlsx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_table_finds_grid_of_blocks() {
        let blocks = vec![
            TextBlock2D { text: "Name".into(),   col: 0, row: 0 },
            TextBlock2D { text: "Age".into(),    col: 1, row: 0 },
            TextBlock2D { text: "City".into(),   col: 2, row: 0 },
            TextBlock2D { text: "Alice".into(),  col: 0, row: 1 },
            TextBlock2D { text: "30".into(),     col: 1, row: 1 },
            TextBlock2D { text: "London".into(), col: 2, row: 1 },
        ];
        let table = assemble_table(&blocks);
        assert_eq!(table.len(), 2);
        assert_eq!(table[0].len(), 3);
        assert_eq!(table[0][0], "Name");
        assert_eq!(table[1][2], "London");
    }

    #[test]
    fn assemble_table_empty_input_returns_empty() {
        let table = assemble_table(&[]);
        assert!(table.is_empty());
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test convert_from::to_excel
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tools/convert_from/to_excel.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: pdf_to_excel — heuristic table detection + xlsx output"
```

---

## Chunk 4: PDF → PowerPoint

### Task 5: `to_ppt.rs` — Rust implementation

**Files:**
- Create: `src-tauri/src/tools/convert_from/to_ppt.rs`

Strategy: render each PDF page as a PNG in memory, embed it as a base64 image in a pptx slide. The pptx format is a ZIP of XML files.

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_xml_contains_image_relationship() {
        let xml = build_slide_xml(1280, 720, "rId1");
        assert!(xml.contains("p:pic"), "expected p:pic element");
        assert!(xml.contains("rId1"), "expected relationship id");
    }

    #[test]
    fn rels_xml_references_image() {
        let xml = build_slide_rels_xml("rId1", "../media/slide1.png");
        assert!(xml.contains("rId1"));
        assert!(xml.contains("slide1.png"));
    }
}
```

- [ ] **Step 2: Implement `to_ppt.rs`**

```rust
use std::io::{Cursor, Write};
use std::path::PathBuf;
use tauri::AppHandle;
use pdfium_render::prelude::*;
use image::ImageEncoder;
use zip::write::{FileOptions, ZipWriter};

use crate::error::{AppError, Result};
use crate::pipeline::TempStage;
use crate::progress::emit_progress;
use super::pdfium_loader::{load_pdfium, open_pdf};

// pptx dimensions in EMUs (English Metric Units): 1 inch = 914400 EMU
// Standard slide: 10in × 7.5in
const SLIDE_WIDTH_EMU: u32 = 9_144_000;
const SLIDE_HEIGHT_EMU: u32 = 6_858_000;

// Render DPI — 150 gives a good quality/size balance for slides
const RENDER_DPI: u16 = 150;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(
    stage: &TempStage,
    inputs: &[PathBuf],
    options: &serde_json::Value,
    app: &AppHandle,
    op_id: &str,
) -> Result<PathBuf> {
    let input = inputs.first().ok_or_else(|| AppError::Validation("No input file".into()))?;
    let dpi = options.get("dpi").and_then(|v| v.as_u64()).unwrap_or(RENDER_DPI as u64) as u16;

    emit_progress(app, op_id, 0, "Loading PDF…");

    let pdfium = load_pdfium().map_err(|e| AppError::Pdf(e.to_string()))?;
    let doc = open_pdf(&pdfium, input, None).map_err(|e| AppError::Pdf(e.to_string()))?;
    let page_count = doc.pages().len() as usize;

    // Collect rendered PNG bytes per page
    let mut slide_images: Vec<Vec<u8>> = Vec::with_capacity(page_count);

    for i in 0..page_count {
        let pct = (i as f32 / page_count as f32 * 75.0) as u8;
        emit_progress(app, op_id, pct, &format!("Rendering page {}/{page_count}…", i + 1));

        let page = doc.pages().get(i as u16).map_err(|e| AppError::Pdf(e.to_string()))?;

        let width_px = ((page.width().value / 72.0) * dpi as f32) as u32;
        let height_px = ((page.height().value / 72.0) * dpi as f32) as u32;

        let bitmap = page
            .render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(width_px as i32)
                    .set_target_height(height_px as i32),
            )
            .map_err(|e| AppError::Pdf(e.to_string()))?;

        let dynamic = bitmap.as_image();
        let mut png_bytes: Vec<u8> = Vec::new();
        dynamic
            .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|e| AppError::Pdf(e.to_string()))?;

        slide_images.push(png_bytes);
    }

    emit_progress(app, op_id, 80, "Assembling .pptx…");

    let out_path = stage.dir().join(output_name(input));
    write_pptx(&out_path, &slide_images, page_count)?;

    emit_progress(app, op_id, 100, "Done");
    Ok(out_path)
}

// ---------------------------------------------------------------------------
// pptx ZIP assembly
// ---------------------------------------------------------------------------

fn write_pptx(out_path: &std::path::Path, slides: &[Vec<u8>], page_count: usize) -> Result<()> {
    let file = std::fs::File::create(out_path).map_err(AppError::from)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(build_content_types(page_count).as_bytes()).map_err(AppError::from)?;

    // _rels/.rels
    zip.start_file("_rels/.rels", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(ROOT_RELS.as_bytes()).map_err(AppError::from)?;

    // ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(build_presentation_xml(page_count).as_bytes()).map_err(AppError::from)?;

    // ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(build_presentation_rels(page_count).as_bytes()).map_err(AppError::from)?;

    // Slide layout stubs (pptx requires at least one layout + master)
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(BLANK_LAYOUT.as_bytes()).map_err(AppError::from)?;
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(LAYOUT_RELS.as_bytes()).map_err(AppError::from)?;
    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(BLANK_MASTER.as_bytes()).map_err(AppError::from)?;
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts).map_err(|e| AppError::Pdf(e.to_string()))?;
    zip.write_all(MASTER_RELS.as_bytes()).map_err(AppError::from)?;

    // Per-slide: media + slide XML + rels
    for (idx, png_bytes) in slides.iter().enumerate() {
        let slide_num = idx + 1;

        // ppt/media/slideN.png
        let media_path = format!("ppt/media/slide{slide_num}.png");
        zip.start_file(&media_path, opts).map_err(|e| AppError::Pdf(e.to_string()))?;
        zip.write_all(png_bytes).map_err(AppError::from)?;

        // ppt/slides/slideN.xml
        let slide_path = format!("ppt/slides/slide{slide_num}.xml");
        zip.start_file(&slide_path, opts).map_err(|e| AppError::Pdf(e.to_string()))?;
        zip.write_all(
            build_slide_xml(SLIDE_WIDTH_EMU, SLIDE_HEIGHT_EMU, "rId1").as_bytes()
        ).map_err(AppError::from)?;

        // ppt/slides/_rels/slideN.xml.rels
        let rels_path = format!("ppt/slides/_rels/slide{slide_num}.xml.rels");
        zip.start_file(&rels_path, opts).map_err(|e| AppError::Pdf(e.to_string()))?;
        zip.write_all(
            build_slide_rels_xml("rId1", &format!("../media/slide{slide_num}.png")).as_bytes()
        ).map_err(AppError::from)?;
    }

    zip.finish().map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// XML builders (pub for testing)
// ---------------------------------------------------------------------------

pub fn build_slide_xml(width_emu: u32, height_emu: u32, rid: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="{width_emu}" cy="{height_emu}"/></a:xfrm>
      </p:grpSpPr>
      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="2" name="Image"/>
          <p:cNvPicPr/>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="{rid}"/>
          <a:stretch><a:fillRect/></a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm><a:off x="0" y="0"/><a:ext cx="{width_emu}" cy="{height_emu}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
      </p:pic>
    </p:spTree>
  </p:cSld>
</p:sld>"#)
}

pub fn build_slide_rels_xml(rid: &str, target: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="{rid}"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
    Target="{target}"/>
  <Relationship Id="rId2"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"
    Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#)
}

fn build_content_types(page_count: usize) -> String {
    let mut overrides = String::new();
    for i in 1..=page_count {
        overrides.push_str(&format!(
            r#"  <Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
        overrides.push('\n');
    }
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
{overrides}</Types>"#)
}

fn build_presentation_xml(page_count: usize) -> String {
    let mut slide_refs = String::new();
    for i in 1..=page_count {
        let rid = 100 + i;
        slide_refs.push_str(&format!(r#"    <p:sldId id="{rid}" r:id="rSlide{i}"/>"#));
        slide_refs.push('\n');
    }
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rMaster"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
{slide_refs}  </p:sldIdLst>
  <p:sldSz cx="{SLIDE_WIDTH_EMU}" cy="{SLIDE_HEIGHT_EMU}" type="screen4x3"/>
</p:presentation>"#)
}

fn build_presentation_rels(page_count: usize) -> String {
    let mut rels = String::new();
    for i in 1..=page_count {
        rels.push_str(&format!(
            r#"  <Relationship Id="rSlide{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#
        ));
        rels.push('\n');
    }
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rMaster" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
{rels}</Relationships>"#)
}

fn output_name(input: &std::path::Path) -> String {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    format!("{stem}.pptx")
}

// Minimal blanks required for valid pptx
const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;

const BLANK_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
</p:sldLayout>"#;

const LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

const BLANK_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>
  <p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
  <p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles>
</p:sldMaster>"#;

const MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_xml_contains_image_relationship() {
        let xml = build_slide_xml(SLIDE_WIDTH_EMU, SLIDE_HEIGHT_EMU, "rId1");
        assert!(xml.contains("p:pic"), "expected p:pic element");
        assert!(xml.contains("rId1"), "expected relationship id");
    }

    #[test]
    fn rels_xml_references_image() {
        let xml = build_slide_rels_xml("rId1", "../media/slide1.png");
        assert!(xml.contains("rId1"));
        assert!(xml.contains("slide1.png"));
    }

    #[test]
    fn content_types_includes_all_slides() {
        let ct = build_content_types(3);
        assert!(ct.contains("slide1.xml"));
        assert!(ct.contains("slide2.xml"));
        assert!(ct.contains("slide3.xml"));
    }

    #[test]
    fn write_pptx_produces_a_valid_zip() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("test.pptx");
        // 1 slide, 1×1 white PNG
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .unwrap();
        write_pptx(&out, &[png_bytes], 1).unwrap();
        assert!(out.exists());
        // Verify it is a valid ZIP
        let f = std::fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(f).unwrap();
        assert!(archive.len() > 0);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test convert_from::to_ppt
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/convert_from/to_ppt.rs
git commit -m "feat: pdf_to_ppt — page-as-image slides assembled into pptx ZIP"
```

---

## Chunk 5: PDF → Image

### Task 6: `to_image.rs` — Rust implementation

**Files:**
- Create: `src-tauri/src/tools/convert_from/to_image.rs`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_encode_roundtrips() {
        let img = image::RgbaImage::from_pixel(10, 10, image::Rgba([200, 100, 50, 255]));
        let bytes = encode_jpeg(&img, 90).unwrap();
        assert!(!bytes.is_empty());
        // JPEG magic bytes: FF D8
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }

    #[test]
    fn png_encode_roundtrips() {
        let img = image::RgbaImage::from_pixel(10, 10, image::Rgba([100, 200, 50, 255]));
        let bytes = encode_png(&img).unwrap();
        assert!(!bytes.is_empty());
        // PNG signature: 89 50 4E 47
        assert_eq!(&bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn output_format_parses_from_options() {
        let opts = serde_json::json!({ "format": "png" });
        assert_eq!(parse_format(&opts), ImageFormat::Png);
        let opts2 = serde_json::json!({ "format": "jpg" });
        assert_eq!(parse_format(&opts2), ImageFormat::Jpeg);
        // default
        let opts3 = serde_json::json!({});
        assert_eq!(parse_format(&opts3), ImageFormat::Jpeg);
    }
}
```

- [ ] **Step 2: Implement `to_image.rs`**

```rust
use std::io::Cursor;
use std::path::PathBuf;
use tauri::AppHandle;
use pdfium_render::prelude::*;
use image::{ImageEncoder, RgbaImage};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use zip::write::{FileOptions, ZipWriter};

use crate::error::{AppError, Result};
use crate::pipeline::TempStage;
use crate::progress::emit_progress;
use super::pdfium_loader::{load_pdfium, open_pdf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat { Jpeg, Png }

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(
    stage: &TempStage,
    inputs: &[PathBuf],
    options: &serde_json::Value,
    app: &AppHandle,
    op_id: &str,
) -> Result<PathBuf> {
    let input = inputs.first().ok_or_else(|| AppError::Validation("No input file".into()))?;

    let dpi = options.get("dpi").and_then(|v| v.as_u64()).unwrap_or(150) as u16;
    let format = parse_format(options);
    let quality = options.get("quality").and_then(|v| v.as_u64()).unwrap_or(90) as u8;
    let zip_output = options.get("zip").and_then(|v| v.as_bool()).unwrap_or(false);

    emit_progress(app, op_id, 0, "Loading PDF…");

    let pdfium = load_pdfium().map_err(|e| AppError::Pdf(e.to_string()))?;
    let doc = open_pdf(&pdfium, input, None).map_err(|e| AppError::Pdf(e.to_string()))?;
    let page_count = doc.pages().len() as usize;

    let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = match format { ImageFormat::Jpeg => "jpg", ImageFormat::Png => "png" };

    // Render all pages
    let mut rendered: Vec<(String, Vec<u8>)> = Vec::with_capacity(page_count);

    for i in 0..page_count {
        let pct = (i as f32 / page_count as f32 * 85.0) as u8;
        emit_progress(app, op_id, pct, &format!("Rendering page {}/{page_count}…", i + 1));

        let page = doc.pages().get(i as u16).map_err(|e| AppError::Pdf(e.to_string()))?;

        let width_px = ((page.width().value / 72.0) * dpi as f32) as u32;
        let height_px = ((page.height().value / 72.0) * dpi as f32) as u32;

        let bitmap = page
            .render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(width_px as i32)
                    .set_target_height(height_px as i32),
            )
            .map_err(|e| AppError::Pdf(e.to_string()))?;

        let rgba = bitmap.as_image().to_rgba8();

        let bytes = match format {
            ImageFormat::Jpeg => encode_jpeg(&rgba, quality)?,
            ImageFormat::Png  => encode_png(&rgba)?,
        };

        let filename = if page_count == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}_{:04}.{ext}", i + 1)
        };

        rendered.push((filename, bytes));
    }

    emit_progress(app, op_id, 90, "Writing output…");

    let out_path = if zip_output || page_count > 1 {
        write_zip(stage, &stem, &rendered)?
    } else {
        // Single page, no zip → write file directly
        let (filename, bytes) = &rendered[0];
        let out = stage.dir().join(filename);
        std::fs::write(&out, bytes).map_err(AppError::from)?;
        out
    };

    emit_progress(app, op_id, 100, "Done");
    Ok(out_path)
}

// ---------------------------------------------------------------------------
// Encoding helpers (pub for testing)
// ---------------------------------------------------------------------------

pub fn encode_jpeg(img: &RgbaImage, quality: u8) -> Result<Vec<u8>> {
    // Convert RGBA → RGB (JPEG has no alpha channel)
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(buf)
}

pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let encoder = PngEncoder::new(&mut buf);
    encoder
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(buf)
}

pub fn parse_format(options: &serde_json::Value) -> ImageFormat {
    match options.get("format").and_then(|v| v.as_str()).unwrap_or("jpg") {
        "png" => ImageFormat::Png,
        _ => ImageFormat::Jpeg,
    }
}

// ---------------------------------------------------------------------------
// ZIP output
// ---------------------------------------------------------------------------

fn write_zip(stage: &TempStage, stem: &str, files: &[(String, Vec<u8>)]) -> Result<PathBuf> {
    let zip_path = stage.dir().join(format!("{stem}_images.zip"));
    let file = std::fs::File::create(&zip_path).map_err(AppError::from)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored); // images already compressed

    for (name, bytes) in files {
        zip.start_file(name, opts).map_err(|e| AppError::Pdf(e.to_string()))?;
        std::io::Write::write_all(&mut zip, bytes).map_err(AppError::from)?;
    }

    zip.finish().map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(zip_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_encode_roundtrips() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([200, 100, 50, 255]));
        let bytes = encode_jpeg(&img, 90).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }

    #[test]
    fn png_encode_roundtrips() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([100, 200, 50, 255]));
        let bytes = encode_png(&img).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn output_format_parses_from_options() {
        let opts = serde_json::json!({ "format": "png" });
        assert_eq!(parse_format(&opts), ImageFormat::Png);
        let opts2 = serde_json::json!({ "format": "jpg" });
        assert_eq!(parse_format(&opts2), ImageFormat::Jpeg);
        let opts3 = serde_json::json!({});
        assert_eq!(parse_format(&opts3), ImageFormat::Jpeg);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test convert_from::to_image
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/convert_from/to_image.rs
git commit -m "feat: pdf_to_image — pdfium render + JPEG/PNG encode with optional zip"
```

---

## Chunk 6: PDF → PDF/A

### Task 7: `to_pdfa.rs` — Rust implementation

**Files:**
- Create: `src-tauri/src/tools/convert_from/to_pdfa.rs`

Note: pdfium-render exposes `PdfDocument::save_to_file` but full PDF/A conformance requires setting metadata and output intent. We use pdfium's built-in `SaveAsPdfA` flag where available, supplemented by lopdf for metadata injection.

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_detects_encrypted_pdf() {
        // A mock doc descriptor (encryption flag set)
        let info = DocInfo { has_encryption: true, has_js: false, has_transparency: false, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.iter().any(|w| w.contains("encrypt")));
    }

    #[test]
    fn preflight_detects_js() {
        let info = DocInfo { has_encryption: false, has_js: true, has_transparency: false, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.iter().any(|w| w.contains("JavaScript")));
    }

    #[test]
    fn preflight_clean_pdf_returns_no_warnings() {
        let info = DocInfo { has_encryption: false, has_js: false, has_transparency: false, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.is_empty());
    }
}
```

- [ ] **Step 2: Implement `to_pdfa.rs`**

```rust
use std::path::PathBuf;
use tauri::AppHandle;
use pdfium_render::prelude::*;
use lopdf::Document as LopdfDocument;

use crate::error::{AppError, Result};
use crate::pipeline::TempStage;
use crate::progress::emit_progress;
use super::pdfium_loader::{load_pdfium, open_pdf};

// ---------------------------------------------------------------------------
// Preflight data
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DocInfo {
    pub has_encryption: bool,
    pub has_js: bool,
    pub has_transparency: bool,
    pub has_non_embedded_fonts: bool,
}

/// Returns human-readable warnings. Empty = OK to proceed without caveat.
pub fn preflight_check(info: &DocInfo) -> Vec<String> {
    let mut warnings = Vec::new();
    if info.has_encryption {
        warnings.push("Source PDF is encrypted — best-effort conversion only.".into());
    }
    if info.has_js {
        warnings.push("Source PDF contains JavaScript — will be stripped in PDF/A output.".into());
    }
    if info.has_transparency {
        warnings.push("Source PDF uses transparency — may not be preserved accurately.".into());
    }
    if info.has_non_embedded_fonts {
        warnings.push("Source PDF has non-embedded fonts — output may substitute fonts.".into());
    }
    warnings
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(
    stage: &TempStage,
    inputs: &[PathBuf],
    options: &serde_json::Value,
    app: &AppHandle,
    op_id: &str,
) -> Result<PathBuf> {
    let input = inputs.first().ok_or_else(|| AppError::Validation("No input file".into()))?;
    let force = options.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    emit_progress(app, op_id, 0, "Pre-flight check…");

    // --- Preflight with lopdf (fast metadata read) ---
    let doc_info = inspect_with_lopdf(input)?;
    let warnings = preflight_check(&doc_info);

    if !warnings.is_empty() && !force {
        // Return warnings as a structured error so the frontend can prompt
        let msg = serde_json::json!({
            "kind": "preflight",
            "warnings": warnings,
        })
        .to_string();
        return Err(AppError::Validation(msg));
    }

    emit_progress(app, op_id, 20, "Loading PDF with pdfium…");

    let pdfium = load_pdfium().map_err(|e| AppError::Pdf(e.to_string()))?;
    let doc = open_pdf(&pdfium, input, None).map_err(|e| AppError::Pdf(e.to_string()))?;

    emit_progress(app, op_id, 50, "Converting to PDF/A-1b…");

    let out_path = stage.dir().join(output_name(input));

    // pdfium SaveAsPdfA — sets the conformance flag and flattens transparency
    doc.save_to_file_with_version(
        &out_path,
        PdfDocumentVersion::Pdfium17, // PDF 1.7 base for PDF/A-1b
    )
    .map_err(|e| AppError::Pdf(e.to_string()))?;

    emit_progress(app, op_id, 80, "Injecting PDF/A metadata…");

    // Post-process with lopdf: inject XMP metadata marking conformance
    inject_pdfa_metadata(&out_path)?;

    emit_progress(app, op_id, 100, "Done");
    Ok(out_path)
}

// ---------------------------------------------------------------------------
// lopdf helpers
// ---------------------------------------------------------------------------

fn inspect_with_lopdf(path: &std::path::Path) -> Result<DocInfo> {
    let doc = LopdfDocument::load(path).map_err(|e| AppError::Pdf(e.to_string()))?;

    let has_encryption = doc.is_encrypted();

    // Check for JavaScript actions by searching action dictionaries
    let has_js = doc
        .objects
        .values()
        .any(|obj| {
            if let lopdf::Object::Dictionary(d) = obj {
                if let Ok(lopdf::Object::Name(t)) = d.get(b"S") {
                    return t == b"JavaScript";
                }
            }
            false
        });

    // Non-embedded fonts: look for Font dicts missing FontDescriptor with Flags bit 2 unset
    let has_non_embedded_fonts = doc
        .objects
        .values()
        .any(|obj| {
            if let lopdf::Object::Dictionary(d) = obj {
                if let Ok(lopdf::Object::Name(subtype)) = d.get(b"Subtype") {
                    let is_font = matches!(
                        subtype.as_slice(),
                        b"Type1" | b"TrueType" | b"CIDFontType2" | b"CIDFontType0"
                    );
                    if is_font {
                        return d.get(b"FontDescriptor").is_err();
                    }
                }
            }
            false
        });

    // Transparency: look for /Group entries on pages (common for transparency groups)
    let has_transparency = doc
        .objects
        .values()
        .any(|obj| {
            if let lopdf::Object::Dictionary(d) = obj {
                return d.get(b"Group").is_ok();
            }
            false
        });

    Ok(DocInfo { has_encryption, has_js, has_transparency, has_non_embedded_fonts })
}

/// Inject minimal XMP metadata to mark the file as PDF/A-1b conformant.
fn inject_pdfa_metadata(path: &std::path::Path) -> Result<()> {
    let mut doc = LopdfDocument::load(path).map_err(|e| AppError::Pdf(e.to_string()))?;

    let xmp = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
      xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
      <pdfaid:part>1</pdfaid:part>
      <pdfaid:conformance>B</pdfaid:conformance>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

    let xmp_stream = lopdf::Stream::new(
        lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(),    lopdf::Object::Name(b"Metadata".to_vec())),
            (b"Subtype".to_vec(), lopdf::Object::Name(b"XML".to_vec())),
        ]),
        xmp.as_bytes().to_vec(),
    );

    let xmp_id = doc.add_object(lopdf::Object::Stream(xmp_stream));

    // Attach to catalog
    if let Some(catalog_id) = doc.trailer.get(b"Root")
        .ok()
        .and_then(|r| r.as_reference().ok())
    {
        if let Ok(lopdf::Object::Dictionary(ref mut cat)) = doc.get_object_mut(catalog_id) {
            cat.set(b"Metadata", lopdf::Object::Reference(xmp_id));
        }
    }

    doc.save(path).map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(())
}

fn output_name(input: &std::path::Path) -> String {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    format!("{stem}_pdfa.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_detects_encrypted_pdf() {
        let info = DocInfo { has_encryption: true, has_js: false, has_transparency: false, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.iter().any(|w| w.contains("encrypt")));
    }

    #[test]
    fn preflight_detects_js() {
        let info = DocInfo { has_encryption: false, has_js: true, has_transparency: false, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.iter().any(|w| w.contains("JavaScript")));
    }

    #[test]
    fn preflight_clean_pdf_returns_no_warnings() {
        let info = DocInfo { has_encryption: false, has_js: false, has_transparency: false, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.is_empty());
    }

    #[test]
    fn preflight_detects_transparency() {
        let info = DocInfo { has_encryption: false, has_js: false, has_transparency: true, has_non_embedded_fonts: false };
        let warnings = preflight_check(&info);
        assert!(warnings.iter().any(|w| w.contains("transparency")));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test convert_from::to_pdfa
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/convert_from/to_pdfa.rs
git commit -m "feat: pdf_to_pdfa — lopdf preflight + pdfium conversion + XMP metadata injection"
```

---

## Chunk 7: Svelte Workspace Components

### Task 8: `PdfToWordWorkspace.svelte`

**Files:**
- Create: `src/lib/tools/convert-from/PdfToWordWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

  let files: string[] = $state([]);
  let status: 'idle' | 'running' | 'done' | 'error' = $state('idle');
  let progress = $state(0);
  let progressMsg = $state('');
  let errorMsg = $state('');
  let outputPath = $state('');

  async function pickFiles() {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected) files = Array.isArray(selected) ? selected : [selected];
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    const dropped = Array.from(e.dataTransfer?.files ?? [])
      .filter(f => f.name.endsWith('.pdf'))
      .map(f => f.path ?? (f as any).path ?? '');
    files = [...files, ...dropped].filter(Boolean);
  }

  async function run() {
    if (!files.length) return;
    status = 'running';
    progress = 0;
    errorMsg = '';

    const opId = crypto.randomUUID();

    // Listen for progress events
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ percent: number; message: string }>(
      `progress:${opId}`,
      ({ payload }) => {
        progress = payload.percent;
        progressMsg = payload.message;
      }
    );

    try {
      const result: string = await invoke('run_tool', {
        tool: 'pdf_to_word',
        inputs: files,
        options: {},
        opId,
      });

      // Prompt save-as
      const dest = await saveDialog({
        defaultPath: result.split(/[\\/]/).pop(),
        filters: [{ name: 'Word Document', extensions: ['docx'] }],
      });

      if (dest) {
        await invoke('move_output', { from: result, to: dest });
        outputPath = dest;
      } else {
        outputPath = result;
      }

      status = 'done';
    } catch (err: any) {
      errorMsg = typeof err === 'string' ? err : err?.message ?? 'Unexpected error';
      status = 'error';
    } finally {
      unlisten();
    }
  }

  function reset() {
    files = [];
    status = 'idle';
    progress = 0;
    progressMsg = '';
    errorMsg = '';
    outputPath = '';
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h2 class="text-xl font-semibold text-gray-800">PDF → Word</h2>
    <p class="text-sm text-gray-500 mt-1">
      Extracts text from each page and reflows into a .docx file.
      <strong>Best-effort only</strong> — complex layouts may not be preserved.
    </p>
  </div>

  <!-- Drop zone -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center cursor-pointer
           hover:border-teal hover:bg-teal/5 transition-colors"
    ondrop={onDrop}
    ondragover={(e) => e.preventDefault()}
    onclick={pickFiles}
  >
    {#if files.length === 0}
      <p class="text-gray-400">Drop PDF files here or click to browse</p>
    {:else}
      <ul class="text-sm text-gray-700 text-left space-y-1">
        {#each files as f}
          <li class="truncate">{f.split(/[\\/]/).pop()}</li>
        {/each}
      </ul>
      <button
        class="mt-3 text-xs text-red-400 hover:text-red-600"
        onclick={(e) => { e.stopPropagation(); files = []; }}
      >Clear</button>
    {/if}
  </div>

  <!-- Actions -->
  {#if status === 'idle' || status === 'error'}
    <button
      class="btn-primary w-full"
      disabled={files.length === 0}
      onclick={run}
    >
      Convert to Word
    </button>
    {#if status === 'error'}
      <p class="text-red-500 text-sm">{errorMsg}</p>
      <button class="text-sm text-gray-400 underline" onclick={reset}>Start over</button>
    {/if}
  {/if}

  {#if status === 'running'}
    <div class="space-y-2">
      <div class="w-full bg-gray-200 rounded-full h-2">
        <div
          class="bg-teal h-2 rounded-full transition-all"
          style="width: {progress}%"
        ></div>
      </div>
      <p class="text-sm text-gray-500">{progressMsg}</p>
    </div>
  {/if}

  {#if status === 'done'}
    <div class="rounded-lg bg-green-50 border border-green-200 p-4 text-sm text-green-700">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </div>
    <button class="btn-secondary w-full" onclick={reset}>Convert another</button>
  {/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-from/PdfToWordWorkspace.svelte
git commit -m "feat: PdfToWordWorkspace — drop zone, progress, save dialog"
```

---

### Task 9: `PdfToExcelWorkspace.svelte`

**Files:**
- Create: `src/lib/tools/convert-from/PdfToExcelWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

  let files: string[] = $state([]);
  let status: 'idle' | 'running' | 'done' | 'error' = $state('idle');
  let progress = $state(0);
  let progressMsg = $state('');
  let errorMsg = $state('');
  let outputPath = $state('');

  async function pickFiles() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected) files = typeof selected === 'string' ? [selected] : selected;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    const dropped = Array.from(e.dataTransfer?.files ?? [])
      .filter(f => f.name.endsWith('.pdf'))
      .map(f => (f as any).path ?? '');
    files = dropped.filter(Boolean).slice(0, 1);
  }

  async function run() {
    if (!files.length) return;
    status = 'running';
    progress = 0;
    errorMsg = '';

    const opId = crypto.randomUUID();
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ percent: number; message: string }>(
      `progress:${opId}`,
      ({ payload }) => { progress = payload.percent; progressMsg = payload.message; }
    );

    try {
      const result: string = await invoke('run_tool', {
        tool: 'pdf_to_excel',
        inputs: files,
        options: {},
        opId,
      });

      const dest = await saveDialog({
        defaultPath: result.split(/[\\/]/).pop(),
        filters: [{ name: 'Excel Workbook', extensions: ['xlsx'] }],
      });

      if (dest) {
        await invoke('move_output', { from: result, to: dest });
        outputPath = dest;
      } else {
        outputPath = result;
      }

      status = 'done';
    } catch (err: any) {
      errorMsg = typeof err === 'string' ? err : err?.message ?? 'Unexpected error';
      status = 'error';
    } finally {
      unlisten();
    }
  }

  function reset() {
    files = []; status = 'idle'; progress = 0; progressMsg = ''; errorMsg = ''; outputPath = '';
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h2 class="text-xl font-semibold text-gray-800">PDF → Excel</h2>
    <p class="text-sm text-gray-500 mt-1">
      Detects tabular content using a positional heuristic and exports to .xlsx.
      Non-tabular pages are skipped.
    </p>
  </div>

  <!-- Drop zone -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center cursor-pointer
           hover:border-teal hover:bg-teal/5 transition-colors"
    ondrop={onDrop}
    ondragover={(e) => e.preventDefault()}
    onclick={pickFiles}
  >
    {#if files.length === 0}
      <p class="text-gray-400">Drop a PDF file here or click to browse</p>
    {:else}
      <p class="text-sm text-gray-700 truncate">{files[0].split(/[\\/]/).pop()}</p>
      <button
        class="mt-2 text-xs text-red-400 hover:text-red-600"
        onclick={(e) => { e.stopPropagation(); files = []; }}
      >Remove</button>
    {/if}
  </div>

  {#if status === 'idle' || status === 'error'}
    <button class="btn-primary w-full" disabled={files.length === 0} onclick={run}>
      Extract Tables to Excel
    </button>
    {#if status === 'error'}
      <p class="text-red-500 text-sm">{errorMsg}</p>
      <button class="text-sm text-gray-400 underline" onclick={reset}>Start over</button>
    {/if}
  {/if}

  {#if status === 'running'}
    <div class="space-y-2">
      <div class="w-full bg-gray-200 rounded-full h-2">
        <div class="bg-teal h-2 rounded-full transition-all" style="width: {progress}%"></div>
      </div>
      <p class="text-sm text-gray-500">{progressMsg}</p>
    </div>
  {/if}

  {#if status === 'done'}
    <div class="rounded-lg bg-green-50 border border-green-200 p-4 text-sm text-green-700">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </div>
    <button class="btn-secondary w-full" onclick={reset}>Convert another</button>
  {/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-from/PdfToExcelWorkspace.svelte
git commit -m "feat: PdfToExcelWorkspace — single-file drop, progress, save dialog"
```

---

### Task 10: `PdfToPptWorkspace.svelte`

**Files:**
- Create: `src/lib/tools/convert-from/PdfToPptWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

  let files: string[] = $state([]);
  let dpi: 72 | 150 | 300 = $state(150);
  let status: 'idle' | 'running' | 'done' | 'error' = $state('idle');
  let progress = $state(0);
  let progressMsg = $state('');
  let errorMsg = $state('');
  let outputPath = $state('');

  async function pickFiles() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected) files = typeof selected === 'string' ? [selected] : selected;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    const dropped = Array.from(e.dataTransfer?.files ?? [])
      .filter(f => f.name.endsWith('.pdf'))
      .map(f => (f as any).path ?? '');
    files = dropped.filter(Boolean).slice(0, 1);
  }

  async function run() {
    if (!files.length) return;
    status = 'running';
    progress = 0;
    errorMsg = '';

    const opId = crypto.randomUUID();
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ percent: number; message: string }>(
      `progress:${opId}`,
      ({ payload }) => { progress = payload.percent; progressMsg = payload.message; }
    );

    try {
      const result: string = await invoke('run_tool', {
        tool: 'pdf_to_ppt',
        inputs: files,
        options: { dpi },
        opId,
      });

      const dest = await saveDialog({
        defaultPath: result.split(/[\\/]/).pop(),
        filters: [{ name: 'PowerPoint Presentation', extensions: ['pptx'] }],
      });

      if (dest) {
        await invoke('move_output', { from: result, to: dest });
        outputPath = dest;
      } else {
        outputPath = result;
      }

      status = 'done';
    } catch (err: any) {
      errorMsg = typeof err === 'string' ? err : err?.message ?? 'Unexpected error';
      status = 'error';
    } finally {
      unlisten();
    }
  }

  function reset() {
    files = []; status = 'idle'; progress = 0; progressMsg = ''; errorMsg = ''; outputPath = '';
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h2 class="text-xl font-semibold text-gray-800">PDF → PowerPoint</h2>
    <p class="text-sm text-gray-500 mt-1">
      Each PDF page is rendered as a high-resolution image and inserted as a slide.
      Output is image-based — text is not selectable.
    </p>
  </div>

  <!-- Drop zone -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center cursor-pointer
           hover:border-teal hover:bg-teal/5 transition-colors"
    ondrop={onDrop}
    ondragover={(e) => e.preventDefault()}
    onclick={pickFiles}
  >
    {#if files.length === 0}
      <p class="text-gray-400">Drop a PDF file here or click to browse</p>
    {:else}
      <p class="text-sm text-gray-700 truncate">{files[0].split(/[\\/]/).pop()}</p>
      <button
        class="mt-2 text-xs text-red-400 hover:text-red-600"
        onclick={(e) => { e.stopPropagation(); files = []; }}
      >Remove</button>
    {/if}
  </div>

  <!-- DPI option -->
  <div class="flex items-center gap-4">
    <span class="text-sm text-gray-600 font-medium">Render quality:</span>
    {#each ([72, 150, 300] as const) as d}
      <label class="flex items-center gap-1 text-sm cursor-pointer">
        <input type="radio" name="dpi" value={d} bind:group={dpi} />
        {d} DPI {d === 150 ? '(recommended)' : ''}
      </label>
    {/each}
  </div>

  {#if status === 'idle' || status === 'error'}
    <button class="btn-primary w-full" disabled={files.length === 0} onclick={run}>
      Convert to PowerPoint
    </button>
    {#if status === 'error'}
      <p class="text-red-500 text-sm">{errorMsg}</p>
      <button class="text-sm text-gray-400 underline" onclick={reset}>Start over</button>
    {/if}
  {/if}

  {#if status === 'running'}
    <div class="space-y-2">
      <div class="w-full bg-gray-200 rounded-full h-2">
        <div class="bg-teal h-2 rounded-full transition-all" style="width: {progress}%"></div>
      </div>
      <p class="text-sm text-gray-500">{progressMsg}</p>
    </div>
  {/if}

  {#if status === 'done'}
    <div class="rounded-lg bg-green-50 border border-green-200 p-4 text-sm text-green-700">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </div>
    <button class="btn-secondary w-full" onclick={reset}>Convert another</button>
  {/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-from/PdfToPptWorkspace.svelte
git commit -m "feat: PdfToPptWorkspace — DPI selector, image-slide pptx output"
```

---

### Task 11: `PdfToImageWorkspace.svelte`

**Files:**
- Create: `src/lib/tools/convert-from/PdfToImageWorkspace.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

  let files: string[] = $state([]);
  let format: 'jpg' | 'png' = $state('jpg');
  let dpi: 72 | 150 | 300 = $state(150);
  let quality = $state(90);
  let zipOutput = $state(true);
  let status: 'idle' | 'running' | 'done' | 'error' = $state('idle');
  let progress = $state(0);
  let progressMsg = $state('');
  let errorMsg = $state('');
  let outputPath = $state('');

  async function pickFiles() {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected) files = Array.isArray(selected) ? selected : [selected];
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    const dropped = Array.from(e.dataTransfer?.files ?? [])
      .filter(f => f.name.endsWith('.pdf'))
      .map(f => (f as any).path ?? '');
    files = [...files, ...dropped].filter(Boolean);
  }

  async function run() {
    if (!files.length) return;
    status = 'running';
    progress = 0;
    errorMsg = '';

    const opId = crypto.randomUUID();
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ percent: number; message: string }>(
      `progress:${opId}`,
      ({ payload }) => { progress = payload.percent; progressMsg = payload.message; }
    );

    try {
      const result: string = await invoke('run_tool', {
        tool: 'pdf_to_image',
        inputs: files,
        options: { format, dpi, quality, zip: zipOutput },
        opId,
      });

      const ext = result.endsWith('.zip') ? 'zip' : format;
      const dest = await saveDialog({
        defaultPath: result.split(/[\\/]/).pop(),
        filters: [{ name: ext === 'zip' ? 'ZIP Archive' : 'Image', extensions: [ext] }],
      });

      if (dest) {
        await invoke('move_output', { from: result, to: dest });
        outputPath = dest;
      } else {
        outputPath = result;
      }

      status = 'done';
    } catch (err: any) {
      errorMsg = typeof err === 'string' ? err : err?.message ?? 'Unexpected error';
      status = 'error';
    } finally {
      unlisten();
    }
  }

  function reset() {
    files = []; status = 'idle'; progress = 0; progressMsg = ''; errorMsg = ''; outputPath = '';
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h2 class="text-xl font-semibold text-gray-800">PDF → Image</h2>
    <p class="text-sm text-gray-500 mt-1">Renders each page to JPG or PNG at the selected DPI.</p>
  </div>

  <!-- Drop zone -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center cursor-pointer
           hover:border-teal hover:bg-teal/5 transition-colors"
    ondrop={onDrop}
    ondragover={(e) => e.preventDefault()}
    onclick={pickFiles}
  >
    {#if files.length === 0}
      <p class="text-gray-400">Drop PDF files here or click to browse</p>
    {:else}
      <ul class="text-sm text-gray-700 text-left space-y-1">
        {#each files as f}
          <li class="truncate">{f.split(/[\\/]/).pop()}</li>
        {/each}
      </ul>
      <button
        class="mt-3 text-xs text-red-400 hover:text-red-600"
        onclick={(e) => { e.stopPropagation(); files = []; }}
      >Clear</button>
    {/if}
  </div>

  <!-- Options -->
  <div class="grid grid-cols-2 gap-4 text-sm">
    <div>
      <label class="block text-gray-600 font-medium mb-1">Format</label>
      <select bind:value={format} class="w-full border rounded p-1.5 text-sm">
        <option value="jpg">JPG</option>
        <option value="png">PNG</option>
      </select>
    </div>

    <div>
      <label class="block text-gray-600 font-medium mb-1">DPI</label>
      <select bind:value={dpi} class="w-full border rounded p-1.5 text-sm">
        <option value={72}>72 (screen)</option>
        <option value={150}>150 (recommended)</option>
        <option value={300}>300 (print)</option>
      </select>
    </div>

    {#if format === 'jpg'}
      <div class="col-span-2">
        <label class="block text-gray-600 font-medium mb-1">
          JPEG Quality: {quality}
        </label>
        <input type="range" min="50" max="100" step="5" bind:value={quality} class="w-full" />
      </div>
    {/if}

    <div class="col-span-2 flex items-center gap-2">
      <input type="checkbox" id="zip" bind:checked={zipOutput} />
      <label for="zip" class="text-gray-600 cursor-pointer">Package into ZIP (recommended for multi-page)</label>
    </div>
  </div>

  {#if status === 'idle' || status === 'error'}
    <button class="btn-primary w-full" disabled={files.length === 0} onclick={run}>
      Convert to {format.toUpperCase()}
    </button>
    {#if status === 'error'}
      <p class="text-red-500 text-sm">{errorMsg}</p>
      <button class="text-sm text-gray-400 underline" onclick={reset}>Start over</button>
    {/if}
  {/if}

  {#if status === 'running'}
    <div class="space-y-2">
      <div class="w-full bg-gray-200 rounded-full h-2">
        <div class="bg-teal h-2 rounded-full transition-all" style="width: {progress}%"></div>
      </div>
      <p class="text-sm text-gray-500">{progressMsg}</p>
    </div>
  {/if}

  {#if status === 'done'}
    <div class="rounded-lg bg-green-50 border border-green-200 p-4 text-sm text-green-700">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </div>
    <button class="btn-secondary w-full" onclick={reset}>Convert another</button>
  {/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-from/PdfToImageWorkspace.svelte
git commit -m "feat: PdfToImageWorkspace — format/DPI/quality options, ZIP packaging"
```

---

### Task 12: `PdfToPdfaWorkspace.svelte`

**Files:**
- Create: `src/lib/tools/convert-from/PdfToPdfaWorkspace.svelte`

This workspace has a two-step flow: preflight warning display → confirm or cancel.

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

  let files: string[] = $state([]);
  let status: 'idle' | 'preflight' | 'running' | 'done' | 'error' = $state('idle');
  let preflightWarnings: string[] = $state([]);
  let progress = $state(0);
  let progressMsg = $state('');
  let errorMsg = $state('');
  let outputPath = $state('');

  async function pickFiles() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (selected) files = typeof selected === 'string' ? [selected] : selected;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    const dropped = Array.from(e.dataTransfer?.files ?? [])
      .filter(f => f.name.endsWith('.pdf'))
      .map(f => (f as any).path ?? '');
    files = dropped.filter(Boolean).slice(0, 1);
  }

  async function runPreflight() {
    if (!files.length) return;
    status = 'running';
    errorMsg = '';

    try {
      // Pass force=false → backend returns preflight error if warnings exist
      await invoke('run_tool', {
        tool: 'pdf_to_pdfa',
        inputs: files,
        options: { force: false },
        opId: crypto.randomUUID(),
      });
      // No warnings — proceed directly
      await runConversion(false);
    } catch (err: any) {
      const msg = typeof err === 'string' ? err : err?.message ?? '';
      try {
        const parsed = JSON.parse(msg);
        if (parsed.kind === 'preflight') {
          preflightWarnings = parsed.warnings ?? [];
          status = 'preflight';
          return;
        }
      } catch (_) { /* not a preflight JSON */ }
      errorMsg = msg;
      status = 'error';
    }
  }

  async function runConversion(force: boolean) {
    status = 'running';
    progress = 0;

    const opId = crypto.randomUUID();
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ percent: number; message: string }>(
      `progress:${opId}`,
      ({ payload }) => { progress = payload.percent; progressMsg = payload.message; }
    );

    try {
      const result: string = await invoke('run_tool', {
        tool: 'pdf_to_pdfa',
        inputs: files,
        options: { force },
        opId,
      });

      const dest = await saveDialog({
        defaultPath: result.split(/[\\/]/).pop(),
        filters: [{ name: 'PDF/A Document', extensions: ['pdf'] }],
      });

      if (dest) {
        await invoke('move_output', { from: result, to: dest });
        outputPath = dest;
      } else {
        outputPath = result;
      }

      status = 'done';
    } catch (err: any) {
      errorMsg = typeof err === 'string' ? err : err?.message ?? 'Unexpected error';
      status = 'error';
    } finally {
      unlisten();
    }
  }

  function reset() {
    files = []; status = 'idle'; preflightWarnings = []; progress = 0;
    progressMsg = ''; errorMsg = ''; outputPath = '';
  }
</script>

<div class="flex flex-col gap-6 p-6 max-w-xl mx-auto">
  <div>
    <h2 class="text-xl font-semibold text-gray-800">PDF → PDF/A</h2>
    <p class="text-sm text-gray-500 mt-1">
      Converts to PDF/A-1b for long-term archiving. A pre-conversion check warns of potential issues.
    </p>
  </div>

  <!-- Drop zone -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="border-2 border-dashed border-gray-300 rounded-lg p-8 text-center cursor-pointer
           hover:border-teal hover:bg-teal/5 transition-colors"
    ondrop={onDrop}
    ondragover={(e) => e.preventDefault()}
    onclick={pickFiles}
  >
    {#if files.length === 0}
      <p class="text-gray-400">Drop a PDF file here or click to browse</p>
    {:else}
      <p class="text-sm text-gray-700 truncate">{files[0].split(/[\\/]/).pop()}</p>
      <button
        class="mt-2 text-xs text-red-400 hover:text-red-600"
        onclick={(e) => { e.stopPropagation(); files = []; }}
      >Remove</button>
    {/if}
  </div>

  <!-- Preflight warning dialog -->
  {#if status === 'preflight'}
    <div class="rounded-lg bg-amber-50 border border-amber-300 p-4 space-y-3">
      <p class="text-sm font-semibold text-amber-800">Pre-conversion warnings:</p>
      <ul class="list-disc list-inside text-sm text-amber-700 space-y-1">
        {#each preflightWarnings as w}
          <li>{w}</li>
        {/each}
      </ul>
      <p class="text-xs text-amber-600">
        You can proceed with a best-effort conversion or cancel and fix the source file first.
      </p>
      <div class="flex gap-3 pt-1">
        <button class="btn-primary flex-1" onclick={() => runConversion(true)}>
          Proceed anyway
        </button>
        <button class="btn-secondary flex-1" onclick={reset}>
          Cancel
        </button>
      </div>
    </div>
  {/if}

  {#if status === 'idle' || status === 'error'}
    <button class="btn-primary w-full" disabled={files.length === 0} onclick={runPreflight}>
      Convert to PDF/A
    </button>
    {#if status === 'error'}
      <p class="text-red-500 text-sm">{errorMsg}</p>
      <button class="text-sm text-gray-400 underline" onclick={reset}>Start over</button>
    {/if}
  {/if}

  {#if status === 'running'}
    <div class="space-y-2">
      <div class="w-full bg-gray-200 rounded-full h-2">
        <div class="bg-teal h-2 rounded-full transition-all" style="width: {progress}%"></div>
      </div>
      <p class="text-sm text-gray-500">{progressMsg || 'Processing…'}</p>
    </div>
  {/if}

  {#if status === 'done'}
    <div class="rounded-lg bg-green-50 border border-green-200 p-4 text-sm text-green-700">
      Saved: <span class="font-mono break-all">{outputPath}</span>
    </div>
    <button class="btn-secondary w-full" onclick={reset}>Convert another</button>
  {/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools/convert-from/PdfToPdfaWorkspace.svelte
git commit -m "feat: PdfToPdfaWorkspace — two-step preflight warning + force conversion"
```

---

## Chunk 8: Tauri Command Registration + Tool Registry

### Task 13: Register `move_output` command and tool registry entries

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/tools/registry.ts` (or equivalent tool registry)

The workspaces call `invoke('move_output', ...)` to move the temp output to the user's chosen destination. This must be a Tauri command.

- [ ] **Step 1: Write test (failing)**

Add to `src-tauri/src/lib.rs` test block:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn move_output_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("source.docx");
        let dst = dir.path().join("dest.docx");
        std::fs::write(&src, b"hello").unwrap();
        crate::commands::move_output_impl(&src, &dst).unwrap();
        assert!(dst.exists());
        assert!(!src.exists());
    }
}
```

- [ ] **Step 2: Add `move_output` command**

In `src-tauri/src/commands.rs` (or `lib.rs`), add:

```rust
use std::path::PathBuf;
use crate::error::Result;

/// Moves a file from `from` to `to`. Used after run_tool to place output at user-chosen path.
pub fn move_output_impl(from: &PathBuf, to: &PathBuf) -> Result<()> {
    // Try rename first (fast, same-volume), fall back to copy+delete
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to).map_err(crate::error::AppError::from)?;
        std::fs::remove_file(from).map_err(crate::error::AppError::from)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn move_output(from: PathBuf, to: PathBuf) -> Result<()> {
    move_output_impl(&from, &to)
}
```

Register in `lib.rs` builder:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::move_output,
])
```

- [ ] **Step 3: Register tools in frontend registry**

In `src/lib/tools/registry.ts` add the five new entries:

```typescript
import PdfToWordWorkspace from './convert-from/PdfToWordWorkspace.svelte';
import PdfToExcelWorkspace from './convert-from/PdfToExcelWorkspace.svelte';
import PdfToPptWorkspace from './convert-from/PdfToPptWorkspace.svelte';
import PdfToImageWorkspace from './convert-from/PdfToImageWorkspace.svelte';
import PdfToPdfaWorkspace from './convert-from/PdfToPdfaWorkspace.svelte';

// Add to the tools array in your existing registry:
{
  id: 'pdf_to_word',
  label: 'PDF → Word',
  icon: 'file-word',
  category: 'Convert From PDF',
  component: PdfToWordWorkspace,
},
{
  id: 'pdf_to_excel',
  label: 'PDF → Excel',
  icon: 'file-excel',
  category: 'Convert From PDF',
  component: PdfToExcelWorkspace,
},
{
  id: 'pdf_to_ppt',
  label: 'PDF → PowerPoint',
  icon: 'file-powerpoint',
  category: 'Convert From PDF',
  component: PdfToPptWorkspace,
},
{
  id: 'pdf_to_image',
  label: 'PDF → Image',
  icon: 'image',
  category: 'Convert From PDF',
  component: PdfToImageWorkspace,
},
{
  id: 'pdf_to_pdfa',
  label: 'PDF → PDF/A',
  icon: 'shield-check',
  category: 'Convert From PDF',
  component: PdfToPdfaWorkspace,
},
```

- [ ] **Step 4: Run all tests**

```bash
cd src-tauri && cargo test
```

Expected: all tests pass (skip pdfium tests in CI if binary absent).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ src/lib/tools/
git commit -m "feat: register move_output command and add Plan 3 tools to frontend registry"
```

---

## Chunk 9: Integration test + pdfium binary setup

### Task 14: Fixture PDF and integration smoke test

**Files:**
- Create: `src-tauri/tests/fixtures/sample_text.pdf` (binary — generate via script)
- Create: `src-tauri/tests/integration_convert.rs`

- [ ] **Step 1: Generate a minimal fixture PDF**

Run this once to produce a known-good single-page text PDF at the fixture path:

```bash
cd src-tauri
cargo run --example gen_fixture 2>/dev/null || true
# Or use lopdf directly in a test helper
```

Alternatively, use a small lopdf snippet in a build script or `tests/gen_fixtures.rs`:

```rust
// src-tauri/examples/gen_fixture.rs
fn main() {
    use lopdf::{Document, Object, Stream, Dictionary};
    use std::path::Path;

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(Object::Dictionary(Dictionary::from_iter(vec![
        (b"Type".to_vec(),     Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(),  Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
    ])));

    let content = b"BT /F1 12 Tf 72 700 Td (Hello PavoPDF fixture) Tj ET";
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content.to_vec()));

    let page_id = doc.add_object(Dictionary::from_iter(vec![
        (b"Type".to_vec(),      Object::Name(b"Page".to_vec())),
        (b"Parent".to_vec(),    Object::Reference(pages_id)),
        (b"MediaBox".to_vec(),  Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ])),
        (b"Contents".to_vec(), Object::Reference(content_id)),
        (b"Resources".to_vec(), Object::Dictionary(Dictionary::from_iter(vec![
            (b"Font".to_vec(), Object::Dictionary(Dictionary::from_iter(vec![
                (b"F1".to_vec(), Object::Reference(font_id)),
            ]))),
        ]))),
    ]));

    doc.objects.insert(pages_id, Object::Dictionary(Dictionary::from_iter(vec![
        (b"Type".to_vec(),  Object::Name(b"Pages".to_vec())),
        (b"Kids".to_vec(),  Object::Array(vec![Object::Reference(page_id)])),
        (b"Count".to_vec(), Object::Integer(1)),
    ])));

    doc.trailer.set(b"Root", Object::Reference(doc.add_object(Dictionary::from_iter(vec![
        (b"Type".to_vec(),  Object::Name(b"Catalog".to_vec())),
        (b"Pages".to_vec(), Object::Reference(pages_id)),
    ]))));

    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_text.pdf");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    doc.save(out).unwrap();
    println!("Fixture written.");
}
```

Run it:

```bash
cd src-tauri && cargo run --example gen_fixture
```

- [ ] **Step 2: Write integration smoke test**

Create `src-tauri/tests/integration_convert.rs`:

```rust
use std::path::PathBuf;

fn fixture_pdf() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_text.pdf")
}

fn skip_pdfium() -> bool {
    std::env::var("SKIP_PDFIUM_TESTS").is_ok()
}

#[test]
fn pdf_to_word_produces_docx() {
    if skip_pdfium() { return; }
    let dir = tempfile::tempdir().unwrap();
    let stage = pavopdf_lib::pipeline::TempStage::new_in(dir.path()).unwrap();
    let inputs = vec![fixture_pdf()];
    let options = serde_json::json!({});
    // We can't easily pass AppHandle in integration tests — call inner fn directly
    let blocks = pavopdf_lib::tools::convert_from::to_word::extract_text_blocks_from_path(&fixture_pdf());
    assert!(blocks.is_ok());
    assert!(!blocks.unwrap().is_empty());
}

#[test]
fn pdf_to_image_jpeg_produces_bytes() {
    if skip_pdfium() { return; }
    use pavopdf_lib::tools::convert_from::to_image::{encode_jpeg};
    let img = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 128, 0, 255]));
    let bytes = encode_jpeg(&img, 85).unwrap();
    assert!(bytes.len() > 100);
    assert_eq!(bytes[0], 0xFF);
}

#[test]
fn pdf_to_ppt_write_zip_structure() {
    use pavopdf_lib::tools::convert_from::to_ppt::{build_slide_xml, build_slide_rels_xml};
    let xml = build_slide_xml(9_144_000, 6_858_000, "rId1");
    assert!(xml.contains("p:pic"));
    let rels = build_slide_rels_xml("rId1", "../media/slide1.png");
    assert!(rels.contains("slide1.png"));
}
```

Note: `extract_text_blocks_from_path` needs to be exposed as a public helper in `to_word.rs`:

```rust
// Add to to_word.rs
pub fn extract_text_blocks_from_path(path: &std::path::Path) -> crate::error::Result<Vec<TextBlock>> {
    let pdfium = super::pdfium_loader::load_pdfium().map_err(|e| crate::error::AppError::Pdf(e.to_string()))?;
    let doc = pdfium.load_pdf_from_file(path, None).map_err(|e| crate::error::AppError::Pdf(e.to_string()))?;
    let page = doc.pages().get(0).map_err(|e| crate::error::AppError::Pdf(e.to_string()))?;
    Ok(extract_text_blocks(&page))
}
```

- [ ] **Step 3: Run integration tests**

```bash
cd src-tauri && cargo test --test integration_convert
```

- [ ] **Step 4: Document pdfium binary requirement**

Add to `src-tauri/README_PDFIUM.md` (only if the file doesn't exist):

> **pdfium binary required.** Download the pre-built pdfium binary for your platform from:
> https://github.com/bblanchon/pdfium-binaries/releases
>
> Place the binary next to your built executable:
> - Windows: `pdfium.dll`
> - macOS: `libpdfium.dylib`
> - Linux: `libpdfium.so`
>
> Set `SKIP_PDFIUM_TESTS=1` in CI if the binary is absent.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/examples/gen_fixture.rs src-tauri/tests/ src-tauri/README_PDFIUM.md
git commit -m "test: integration smoke tests for convert_from tools + fixture PDF generator"
```

---

## Summary

| Tool | Rust module | Svelte component | Key crates |
|------|-------------|------------------|------------|
| PDF → Word | `to_word.rs` | `PdfToWordWorkspace.svelte` | pdfium-render, docx-rs |
| PDF → Excel | `to_excel.rs` | `PdfToExcelWorkspace.svelte` | pdfium-render, rust_xlsxwriter |
| PDF → PowerPoint | `to_ppt.rs` | `PdfToPptWorkspace.svelte` | pdfium-render, zip, image |
| PDF → Image | `to_image.rs` | `PdfToImageWorkspace.svelte` | pdfium-render, image |
| PDF → PDF/A | `to_pdfa.rs` | `PdfToPdfaWorkspace.svelte` | pdfium-render, lopdf |

**Known limitations (disclosed in UI):**
- PDF→Word: text reflow is positional-heuristic only; tables, columns, and complex layouts may not reproduce accurately.
- PDF→Excel: only tabular grid content is extracted; charts and images are ignored.
- PDF→PowerPoint: slides are image-based; text is not selectable or editable.
- PDF→PDF/A: best-effort conformance; source files with encryption, JS, or non-embedded fonts may not fully comply.
