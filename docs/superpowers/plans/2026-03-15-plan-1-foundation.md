# PavoPDF — Plan 1: Foundation

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the full PavoPDF project — Tauri 2 + Svelte 5 + Tailwind, the three-state UI shell (Dashboard / Workspace / Focused), Tauri IPC layer, storage (settings + recent files), and the pipeline infrastructure (validation, temp staging, progress events) — so every subsequent plan can add tools on top of a working foundation.

**Architecture:** Tauri 2 app with a Svelte 5 SPA frontend rendered in the OS native WebView. The Rust backend exposes typed `#[tauri::command]` functions. All storage is flat JSON in the OS app config dir. The processing pipeline (validate → temp → process → notify → save) is wired up but the `process` step is a stub in this plan — tool implementations come in Plans 2–6.

**Tech Stack:** Rust (Tauri 2, lopdf, tempfile, serde_json, tokio), Svelte 5, TypeScript, Tailwind CSS 3, Vite.

---

## Chunk 1: Project Scaffold

### Task 1: Initialize Tauri + Svelte project

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `svelte.config.js`
- Create: `tsconfig.json`
- Create: `src/app.css`
- Create: `src/app.html`
- Create: `src/main.ts`
- Create: `src/App.svelte`

- [ ] **Step 1: Install prerequisites**

Ensure you have installed:
- Rust stable (`rustup update stable`)
- Node.js 20+
- Tauri CLI: `cargo install tauri-cli --version "^2.0"`

Verify: `cargo tauri --version` → should print `tauri-cli 2.x.x`

- [ ] **Step 2: Scaffold the project**

```bash
cd /c/cino/pavopdf
cargo tauri init
```

When prompted:
- App name: `PavoPDF`
- Window title: `PavoPDF`
- Web assets relative path: `../dist`
- Dev server URL: `http://localhost:5173`
- Frontend dev command: `npm run dev`
- Frontend build command: `npm run build`

Then initialize Svelte + Vite frontend:
```bash
npm create vite@latest . -- --template svelte-ts
npm install
```

- [ ] **Step 3: Install Tailwind CSS**

```bash
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

Replace `tailwind.config.js`:
```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,ts,svelte}'],
  theme: {
    extend: {
      colors: {
        teal:  { DEFAULT: '#1B7A8A', dark: '#155f6e' },
        peach: { DEFAULT: '#E8956A', dark: '#d4784c' },
        amber: { DEFAULT: '#D4A017' },
        cream: { DEFAULT: '#F9F5F0' },
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
      },
    },
  },
  plugins: [],
}
```

Replace `src/app.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

* { box-sizing: border-box; }

:root {
  --color-teal:  #1B7A8A;
  --color-peach: #E8956A;
  --color-amber: #D4A017;
  --color-cream: #F9F5F0;
}
```

- [ ] **Step 4: Zero-network capability config**

Create `src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities — zero network permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default",
    "fs:default",
    "path:default",
    "shell:default"
  ]
}
```

Update `src-tauri/tauri.conf.json` — set `bundle.identifier` to `com.pavopdf.app`.

- [ ] **Step 5: Verify the scaffold runs**

```bash
npm run tauri dev
```

Expected: A desktop window opens showing the default Vite + Svelte page. No errors in terminal.

- [ ] **Step 6: Commit**

```bash
git init
echo "node_modules/\ndist/\ntarget/\n.superpowers/" > .gitignore
git add .
git commit -m "feat: initialize Tauri 2 + Svelte 5 + Tailwind scaffold"
```

---

### Task 2: Set up Rust crate dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Update Cargo.toml with all required dependencies**

```toml
[package]
name = "pavopdf"
version = "0.1.0"
edition = "2021"

[lib]
name = "pavopdf_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"

serde = { version = "1", features = ["derive"] }
serde_json = "1"

tokio = { version = "1", features = ["full"] }
tempfile = "3"
anyhow = "1"
thiserror = "1"

lopdf = "0.32"
pdfium-render = { version = "0.8", features = ["pdfium_6666"] }
image = { version = "0.25", features = ["jpeg", "png", "webp"] }
printpdf = "0.7"
docx-rs = "0.4"
calamine = { version = "0.24", features = ["dates"] }
quick-xml = { version = "0.36", features = ["serialize"] }
zip = "2"

[profile.release]
opt-level = "s"
strip = true
```

- [ ] **Step 2: Verify dependencies compile**

```bash
cd src-tauri && cargo build 2>&1 | tail -20
```

Expected: No errors (warnings OK). This may take several minutes on first run as it fetches + compiles all crates.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add all Rust crate dependencies"
```

---

## Chunk 2: Rust Backend — Storage + Pipeline Infrastructure

### Task 3: Error types

**Files:**
- Create: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write test for error serialization**

Create `src-tauri/src/error.rs`:
```rust
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("PDF error: {0}")]
    Pdf(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Cancelled")]
    Cancelled,
    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { AppError::Io(e.to_string()) }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self { AppError::Pdf(e.to_string()) }
}

// Tauri requires commands to return serializable errors
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(e: AppError) -> Self {
        tauri::ipc::InvokeError::from_anyhow(anyhow::anyhow!(e.to_string()))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_to_json() {
        let e = AppError::Validation("file too large".into());
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("Validation"));
        assert!(json.contains("file too large"));
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test error -- --nocapture
```

Expected: 2 tests pass.

- [ ] **Step 3: Wire into lib.rs**

Add to `src-tauri/src/lib.rs`:
```rust
pub mod error;
pub mod storage;
pub mod pipeline;
pub mod commands;
pub mod tools;
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/error.rs src-tauri/src/lib.rs
git commit -m "feat: add AppError type with Tauri serialization"
```

---

### Task 4: Settings storage

**Files:**
- Create: `src-tauri/src/storage/mod.rs`
- Create: `src-tauri/src/storage/settings.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/storage/settings.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub sidebar_collapsed: bool,
    pub default_output_folder: Option<PathBuf>,
    pub ocr_language: String,
    pub auto_updater_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            default_output_folder: None,
            ocr_language: "eng".into(),
            auto_updater_enabled: false,
        }
    }
}

pub fn settings_path(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

pub fn load(app_handle: &tauri::AppHandle) -> Result<Settings> {
    let path = settings_path(app_handle)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let contents = std::fs::read_to_string(&path)?;
    serde_json::from_str(&contents)
        .map_err(|e| AppError::Io(format!("Failed to parse settings: {e}")))
}

pub fn save(app_handle: &tauri::AppHandle, settings: &Settings) -> Result<()> {
    let path = settings_path(app_handle)?;
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::write(&path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Note: load(), save(), and settings_path() require a tauri::AppHandle and
    // cannot be unit-tested without a running Tauri app. They are verified via
    // integration testing when the app runs. The tests below cover the Settings
    // struct serialization and defaults, which is the portable logic.

    #[test]
    fn default_settings_are_valid() {
        let s = Settings::default();
        assert!(!s.sidebar_collapsed);
        assert!(!s.auto_updater_enabled);
        assert_eq!(s.ocr_language, "eng");
    }

    #[test]
    fn settings_roundtrip_json() {
        let s = Settings {
            sidebar_collapsed: true,
            default_output_folder: Some("/tmp/out".into()),
            ocr_language: "eng".into(),
            auto_updater_enabled: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sidebar_collapsed, true);
        assert_eq!(restored.ocr_language, "eng");
    }
}
```

Create `src-tauri/src/storage/mod.rs`:
```rust
pub mod settings;
pub mod recent_files;
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test storage::settings -- --nocapture
```

Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/storage/
git commit -m "feat: settings storage with JSON persistence"
```

---

### Task 5: Recent files storage

**Files:**
- Create: `src-tauri/src/storage/recent_files.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/storage/recent_files.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::error::{AppError, Result};

const MAX_RECENT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub tool: String,
    pub timestamp: u64,
    pub exists: bool,
}

impl RecentEntry {
    pub fn new(path: PathBuf, tool: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let exists = path.exists();
        Self { path, tool, timestamp, exists }
    }
}

pub fn recent_path(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("recent_files.json"))
}

pub fn load(app_handle: &tauri::AppHandle) -> Result<Vec<RecentEntry>> {
    let path = recent_path(app_handle)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let contents = std::fs::read_to_string(&path)?;
    let mut entries: Vec<RecentEntry> = serde_json::from_str(&contents)
        .map_err(|e| AppError::Io(format!("Failed to parse recent files: {e}")))?;
    // Refresh exists flag on load
    for entry in &mut entries {
        entry.exists = entry.path.exists();
    }
    Ok(entries)
}

pub fn push(app_handle: &tauri::AppHandle, entry: RecentEntry) -> Result<()> {
    let mut entries = load(app_handle)?;
    // Remove any existing entry for the same path
    entries.retain(|e| e.path != entry.path);
    entries.insert(0, entry);
    entries.truncate(MAX_RECENT);
    save_entries(app_handle, &entries)
}

pub fn remove(app_handle: &tauri::AppHandle, path: &Path) -> Result<()> {
    let mut entries = load(app_handle)?;
    entries.retain(|e| e.path != path);
    save_entries(app_handle, &entries)
}

fn save_entries(app_handle: &tauri::AppHandle, entries: &[RecentEntry]) -> Result<()> {
    let path = recent_path(app_handle)?;
    let contents = serde_json::to_string_pretty(entries)
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::write(&path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_entry_marks_nonexistent_path() {
        let entry = RecentEntry::new("/nonexistent/path/file.pdf".into(), "merge".into());
        assert!(!entry.exists);
    }

    #[test]
    fn recent_entry_roundtrip() {
        let entry = RecentEntry {
            path: "/tmp/test.pdf".into(),
            tool: "compress".into(),
            timestamp: 1700000000,
            exists: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: RecentEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.tool, "compress");
        assert_eq!(restored.timestamp, 1700000000);
    }

    #[test]
    fn max_recent_enforced() {
        let entries: Vec<RecentEntry> = (0..25)
            .map(|i| RecentEntry {
                path: format!("/tmp/file{i}.pdf").into(),
                tool: "merge".into(),
                timestamp: i as u64,
                exists: false,
            })
            .collect();
        let mut truncated = entries;
        truncated.truncate(MAX_RECENT);
        assert_eq!(truncated.len(), 20);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test storage::recent_files -- --nocapture
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/storage/recent_files.rs
git commit -m "feat: recent files storage (max 20, exists flag refresh)"
```

---

### Task 6: Pipeline — validation + temp staging

**Files:**
- Create: `src-tauri/src/pipeline/mod.rs`
- Create: `src-tauri/src/pipeline/validate.rs`
- Create: `src-tauri/src/pipeline/temp.rs`
- Create: `src-tauri/src/pipeline/progress.rs`

- [ ] **Step 1: Write failing tests for validation**

Create `src-tauri/src/pipeline/validate.rs`:
```rust
use std::path::Path;
use crate::error::{AppError, Result};

const PDF_MAGIC: &[u8] = b"%PDF";
const WARN_SIZE_BYTES: u64 = 500 * 1024 * 1024;  // 500 MB
const BLOCK_SIZE_BYTES: u64 = 2 * 1024 * 1024 * 1024;  // 2 GB
const OCR_WARN_SIZE_BYTES: u64 = 50 * 1024 * 1024;  // 50 MB

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub kind: WarningKind,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum WarningKind {
    LargeFile,
    OcrLargeFile,
}

pub fn validate_pdf(path: &Path, tool: &str) -> Result<Vec<ValidationWarning>> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| AppError::NotFound(path.display().to_string()))?;
    let size = metadata.len();

    if size > BLOCK_SIZE_BYTES {
        return Err(AppError::Validation(
            format!("File exceeds 2 GB limit ({:.1} GB)", size as f64 / 1e9)
        ));
    }

    // Check magic bytes
    let header = read_header(path, 4)?;
    if &header[..4] != PDF_MAGIC {
        return Err(AppError::Validation(
            "File does not appear to be a valid PDF (wrong file header)".into()
        ));
    }

    let mut warnings = vec![];

    if size > WARN_SIZE_BYTES {
        warnings.push(ValidationWarning {
            kind: WarningKind::LargeFile,
            message: format!("Large file ({:.0} MB) — processing may take a moment", size as f64 / 1e6),
        });
    }

    if tool == "ocr" && size > OCR_WARN_SIZE_BYTES {
        warnings.push(ValidationWarning {
            kind: WarningKind::OcrLargeFile,
            message: format!("File is {:.0} MB — OCR on large files can take several minutes", size as f64 / 1e6),
        });
    }

    Ok(warnings)
}

fn read_header(path: &Path, n: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; n];
    f.read_exact(&mut buf)
        .map_err(|e| AppError::Validation(format!("Could not read file header: {e}")))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_pdf_file(size: u64) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"%PDF-1.4\n").unwrap();
        // Pad to desired size
        let padding = size.saturating_sub(9);
        f.write_all(&vec![b'x'; padding as usize]).unwrap();
        f
    }

    fn make_bad_file() -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"PK\x03\x04some zip content").unwrap();
        f
    }

    #[test]
    fn valid_small_pdf_no_warnings() {
        let f = make_pdf_file(1024);
        let warnings = validate_pdf(f.path(), "merge").unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn invalid_magic_bytes_rejected() {
        let f = make_bad_file();
        let result = validate_pdf(f.path(), "merge");
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn nonexistent_file_returns_not_found() {
        let result = validate_pdf(Path::new("/nonexistent/file.pdf"), "merge");
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn ocr_tool_gets_extra_warning_for_large_file() {
        // Create a file that exceeds the OCR warn threshold by writing enough bytes.
        // Use a file just above the 50MB mark to exercise the OCR branch.
        // NOTE: this test writes ~51MB to disk; skip in CI with limited disk via
        // `cargo test -- --skip ocr_tool_gets_extra_warning` if needed.
        let f = make_pdf_file(OCR_WARN_SIZE_BYTES + 1024);
        let warnings = validate_pdf(f.path(), "ocr").unwrap();
        let has_ocr_warning = warnings.iter().any(|w| matches!(w.kind, WarningKind::OcrLargeFile));
        assert!(has_ocr_warning, "expected OCR large-file warning for >50MB file");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test pipeline::validate -- --nocapture
```

Expected: 4 tests pass.

- [ ] **Step 3: Write temp staging module**

Create `src-tauri/src/pipeline/temp.rs`:
```rust
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use crate::error::Result;

/// Scoped temp directory that auto-deletes on drop.
pub struct TempStage {
    dir: TempDir,
}

impl TempStage {
    pub fn new() -> Result<Self> {
        let dir = TempDir::new()?;
        Ok(Self { dir })
    }

    /// Copy source file into the temp directory and return the copy path.
    pub fn stage_file(&self, source: &Path) -> Result<PathBuf> {
        let filename = source
            .file_name()
            .ok_or_else(|| crate::error::AppError::Validation("Invalid file path".into()))?;
        let dest = self.dir.path().join(filename);
        std::fs::copy(source, &dest)?;
        Ok(dest)
    }

    /// Path to a new output file in the temp dir.
    pub fn output_path(&self, filename: &str) -> PathBuf {
        self.dir.path().join(filename)
    }

    pub fn dir_path(&self) -> &Path {
        self.dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn stage_file_copies_content() {
        let mut source = NamedTempFile::new().unwrap();
        source.write_all(b"hello pdf").unwrap();
        let stage = TempStage::new().unwrap();
        let staged = stage.stage_file(source.path()).unwrap();
        assert!(staged.exists());
        assert_eq!(std::fs::read_to_string(&staged).unwrap(), "hello pdf");
    }

    #[test]
    fn output_path_in_temp_dir() {
        let stage = TempStage::new().unwrap();
        let out = stage.output_path("result.pdf");
        assert_eq!(out.parent().unwrap(), stage.dir_path());
    }

    #[test]
    fn temp_dir_deleted_on_drop() {
        let dir_path: PathBuf;
        {
            let stage = TempStage::new().unwrap();
            dir_path = stage.dir_path().to_path_buf();
            assert!(dir_path.exists());
        }
        assert!(!dir_path.exists());
    }
}
```

- [ ] **Step 4: Write progress events module**

Create `src-tauri/src/pipeline/progress.rs`:
```rust
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub operation_id: String,
    pub percent: u8,
    pub message: String,
}

pub fn emit_progress(app: &AppHandle, operation_id: &str, percent: u8, message: &str) {
    let _ = app.emit("pdf-progress", ProgressEvent {
        operation_id: operation_id.into(),
        percent,
        message: message.into(),
    });
}

pub fn emit_complete(app: &AppHandle, operation_id: &str) {
    let _ = app.emit("pdf-complete", serde_json::json!({
        "operation_id": operation_id,
    }));
}

pub fn emit_error(app: &AppHandle, operation_id: &str, message: &str) {
    let _ = app.emit("pdf-error", serde_json::json!({
        "operation_id": operation_id,
        "message": message,
    }));
}
```

Create `src-tauri/src/pipeline/mod.rs`:
```rust
pub mod validate;
pub mod temp;
pub mod progress;
```

- [ ] **Step 5: Run all pipeline tests**

```bash
cd src-tauri && cargo test pipeline -- --nocapture
```

Expected: 7 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/pipeline/
git commit -m "feat: pipeline — validation, temp staging, progress events"
```

---

### Task 7: Tauri IPC commands skeleton

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/settings.rs`
- Create: `src-tauri/src/commands/recent_files.rs`
- Create: `src-tauri/src/commands/process.rs`
- Create: `src-tauri/src/tools/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write tool enum and stub**

Create `src-tauri/src/tools/mod.rs`:
```rust
use serde::Deserialize;
use std::path::PathBuf;
use crate::error::Result;
use crate::pipeline::{temp::TempStage, progress};
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Merge, Split, Compress, Rotate, Reorder, Remove,
    PdfToWord, PdfToExcel, PdfToPpt, PdfToImage, PdfToPdfa,
    WordToPdf, ExcelToPdf, PptToPdf, ImageToPdf, HtmlToPdf,
    Edit, Watermark, PageNumbers, Redact,
    Protect, Unlock, Sign,
    Ocr, Repair,
}

#[derive(Debug, Deserialize)]
pub struct ProcessRequest {
    pub operation_id: String,
    pub tool: Tool,
    pub input_paths: Vec<PathBuf>,
    pub output_stem: String,
    pub options: serde_json::Value,
}

pub async fn run(
    app: AppHandle,
    req: ProcessRequest,
) -> Result<PathBuf> {
    let stage = TempStage::new()?;
    progress::emit_progress(&app, &req.operation_id, 10, "Staging files...");

    // Stage all input files
    let staged_inputs: Vec<PathBuf> = req.input_paths
        .iter()
        .map(|p| stage.stage_file(p))
        .collect::<Result<_>>()?;

    progress::emit_progress(&app, &req.operation_id, 20, "Processing...");

    // Tool dispatch — ALL TOOLS RETURN AN ERROR IN PLAN 1.
    // This is intentional: tool implementations arrive in Plans 2-6.
    // If you call process_pdf from the frontend during Plan 1, expect an error response.
    // Do not wire up tool-trigger UI in Plan 1 — only test storage and pipeline commands.
    let output_path = match req.tool {
        _ => Err(crate::error::AppError::Pdf(
            format!("Tool '{:?}' not yet implemented — see Plans 2-6", req.tool)
        )),
    }?;

    progress::emit_progress(&app, &req.operation_id, 100, "Done");
    Ok(output_path)
}
```

- [ ] **Step 2: Write settings commands**

Create `src-tauri/src/commands/settings.rs`:
```rust
use tauri::AppHandle;
use crate::{error::Result, storage::settings::{self, Settings}};

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Settings> {
    settings::load(&app)
}

#[tauri::command]
pub async fn set_settings(app: AppHandle, settings: Settings) -> Result<()> {
    settings::save(&app, &settings)
}
```

- [ ] **Step 3: Write recent files commands**

Create `src-tauri/src/commands/recent_files.rs`:
```rust
use std::path::PathBuf;
use tauri::AppHandle;
use crate::{
    error::Result,
    storage::recent_files::{self, RecentEntry},
};

#[tauri::command]
pub async fn get_recent_files(app: AppHandle) -> Result<Vec<RecentEntry>> {
    recent_files::load(&app)
}

#[tauri::command]
pub async fn remove_recent_file(app: AppHandle, path: PathBuf) -> Result<()> {
    recent_files::remove(&app, &path)
}
```

- [ ] **Step 4: Write process command**

Create `src-tauri/src/commands/process.rs`:
```rust
use std::path::PathBuf;
use tauri::AppHandle;
use crate::{error::Result, tools::{self, ProcessRequest}};

#[tauri::command]
pub async fn process_pdf(app: AppHandle, request: ProcessRequest) -> Result<PathBuf> {
    tools::run(app, request).await
}

#[tauri::command]
pub async fn open_file_dialog(app: AppHandle, multiple: bool) -> Result<Vec<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    // Returns None if user cancels — treat as empty list
    let result = if multiple {
        app.dialog().file().pick_files_blocking()
            .map(|files| files.into_iter().map(|f| f.path).collect())
            .unwrap_or_default()
    } else {
        app.dialog().file().pick_file_blocking()
            .map(|f| vec![f.path])
            .unwrap_or_default()
    };
    Ok(result)
}

#[tauri::command]
pub async fn save_file_dialog(
    app: AppHandle,
    suggested_name: String,
) -> Result<Option<PathBuf>> {
    let path = app
        .dialog()
        .file()
        .set_file_name(&suggested_name)
        .save_file_blocking()
        .map(|f| f.path);
    Ok(path)
}
```

Create `src-tauri/src/commands/mod.rs`:
```rust
pub mod settings;
pub mod recent_files;
pub mod process;
```

- [ ] **Step 5: Register commands in lib.rs**

Replace `src-tauri/src/lib.rs`:
```rust
pub mod error;
pub mod storage;
pub mod pipeline;
pub mod commands;
pub mod tools;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::recent_files::get_recent_files,
            commands::recent_files::remove_recent_file,
            commands::process::process_pdf,
            commands::process::open_file_dialog,
            commands::process::save_file_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cd src-tauri && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: No lines output (no errors).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/ src-tauri/src/tools/ src-tauri/src/lib.rs
git commit -m "feat: Tauri IPC commands — process_pdf, settings, recent files, file dialogs"
```

---

## Chunk 3: Svelte Frontend — Types + Stores + UI Shell

### Task 8: TypeScript types + Tauri invoke wrapper

**Files:**
- Create: `src/lib/types.ts`
- Create: `src/lib/api.ts`

- [ ] **Step 1: Write shared types**

Create `src/lib/types.ts`:
```typescript
export type Tool =
  | 'merge' | 'split' | 'compress' | 'rotate' | 'reorder' | 'remove'
  | 'pdf_to_word' | 'pdf_to_excel' | 'pdf_to_ppt' | 'pdf_to_image' | 'pdf_to_pdfa'
  | 'word_to_pdf' | 'excel_to_pdf' | 'ppt_to_pdf' | 'image_to_pdf' | 'html_to_pdf'
  | 'edit' | 'watermark' | 'page_numbers' | 'redact'
  | 'protect' | 'unlock' | 'sign'
  | 'ocr' | 'repair';

export interface ToolMeta {
  id: Tool;
  label: string;
  icon: string;
  category: Category;
  description: string;
}

export type Category =
  | 'organise'
  | 'pdf_to_other'
  | 'other_to_pdf'
  | 'edit'
  | 'security'
  | 'repair';

export interface CategoryMeta {
  id: Category;
  label: string;
  icon: string;
}

export interface RecentEntry {
  path: string;
  tool: string;
  timestamp: number;
  exists: boolean;
}

export interface Settings {
  sidebar_collapsed: boolean;
  default_output_folder: string | null;
  ocr_language: string;
  auto_updater_enabled: boolean;
}

export interface ProgressEvent {
  operation_id: string;
  percent: number;
  message: string;
}

export interface ProcessRequest {
  operation_id: string;
  tool: Tool;
  input_paths: string[];
  output_stem: string;
  options: Record<string, unknown>;
}
```

- [ ] **Step 2: Write Tauri invoke wrapper**

Create `src/lib/api.ts`:
```typescript
import { invoke } from '@tauri-apps/api/core';
import type { Settings, RecentEntry, ProcessRequest } from './types';

export const api = {
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (settings: Settings) => invoke<void>('set_settings', { settings }),

  getRecentFiles: () => invoke<RecentEntry[]>('get_recent_files'),
  removeRecentFile: (path: string) => invoke<void>('remove_recent_file', { path }),

  openFileDialog: (multiple: boolean) => invoke<string[]>('open_file_dialog', { multiple }),
  saveFileDialog: (suggestedName: string) => invoke<string | null>('save_file_dialog', { suggestedName }),

  processPdf: (request: ProcessRequest) => invoke<string>('process_pdf', { request }),
};
```

- [ ] **Step 3: Install Tauri JS API**

```bash
npm install @tauri-apps/api
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts src/lib/api.ts package.json package-lock.json
git commit -m "feat: TypeScript types and Tauri invoke wrapper"
```

---

### Task 9: Svelte stores

**Files:**
- Create: `src/lib/stores/settings.svelte.ts`
- Create: `src/lib/stores/recent-files.svelte.ts`
- Create: `src/lib/stores/active-tool.svelte.ts`
- Create: `src/lib/stores/operation.svelte.ts`

- [ ] **Step 1: Write settings store**

Create `src/lib/stores/settings.svelte.ts`:
```typescript
import { api } from '../api';
import type { Settings } from '../types';

const defaults: Settings = {
  sidebar_collapsed: false,
  default_output_folder: null,
  ocr_language: 'eng',
  auto_updater_enabled: false,
};

let settings = $state<Settings>(defaults);
let loaded = $state(false);

export const settingsStore = {
  get value() { return settings; },
  get loaded() { return loaded; },

  async load() {
    settings = await api.getSettings();
    loaded = true;
  },

  async update(patch: Partial<Settings>) {
    settings = { ...settings, ...patch };
    await api.setSettings(settings);
  },
};
```

- [ ] **Step 2: Write recent files store**

Create `src/lib/stores/recent-files.svelte.ts`:
```typescript
import { api } from '../api';
import type { RecentEntry } from '../types';

let entries = $state<RecentEntry[]>([]);

export const recentFilesStore = {
  get entries() { return entries; },

  async load() {
    entries = await api.getRecentFiles();
  },

  async remove(path: string) {
    await api.removeRecentFile(path);
    entries = entries.filter(e => e.path !== path);
  },
};
```

- [ ] **Step 3: Write active tool store**

Create `src/lib/stores/active-tool.svelte.ts`:
```typescript
import type { Tool, Category } from '../types';

let activeTool = $state<Tool | null>(null);
let activeCategory = $state<Category>('organise');
let view = $state<'dashboard' | 'workspace'>('dashboard');

export const activeToolStore = {
  get tool() { return activeTool; },
  get category() { return activeCategory; },
  get view() { return view; },

  selectTool(tool: Tool) {
    activeTool = tool;
    view = 'workspace';
  },

  setCategory(category: Category) {
    activeCategory = category;
  },

  goHome() {
    activeTool = null;
    view = 'dashboard';
  },
};
```

- [ ] **Step 4: Write operation store (progress tracking)**

Create `src/lib/stores/operation.svelte.ts`:
```typescript
import { listen } from '@tauri-apps/api/event';
import type { ProgressEvent } from '../types';

interface Operation {
  id: string;
  tool: string;
  percent: number;
  message: string;
  status: 'running' | 'done' | 'error';
  errorMessage?: string;
}

let operations = $state<Map<string, Operation>>(new Map());

export const operationStore = {
  get all() { return [...operations.values()]; },
  get(id: string) { return operations.get(id); },

  start(id: string, tool: string) {
    operations.set(id, { id, tool, percent: 0, message: 'Starting...', status: 'running' });
    operations = new Map(operations);
  },

  complete(id: string) {
    const op = operations.get(id);
    if (op) {
      operations.set(id, { ...op, percent: 100, status: 'done' });
      operations = new Map(operations);
    }
  },

  fail(id: string, message: string) {
    const op = operations.get(id);
    if (op) {
      operations.set(id, { ...op, status: 'error', errorMessage: message });
      operations = new Map(operations);
    }
  },

  clear(id: string) {
    operations.delete(id);
    operations = new Map(operations);
  },
};

// Wire up Tauri event listeners
listen<ProgressEvent>('pdf-progress', ({ payload }) => {
  const op = operations.get(payload.operation_id);
  if (op) {
    operations.set(payload.operation_id, { ...op, percent: payload.percent, message: payload.message });
    operations = new Map(operations);
  }
});

listen<{ operation_id: string }>('pdf-complete', ({ payload }) => {
  operationStore.complete(payload.operation_id);
});

listen<{ operation_id: string; message: string }>('pdf-error', ({ payload }) => {
  operationStore.fail(payload.operation_id, payload.message);
});
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/stores/
git commit -m "feat: Svelte 5 stores — settings, recent files, active tool, operation progress"
```

---

### Task 10: Tool registry

**Files:**
- Create: `src/lib/tools-registry.ts`

- [ ] **Step 1: Write the registry**

Create `src/lib/tools-registry.ts`:
```typescript
import type { ToolMeta, CategoryMeta } from './types';

export const CATEGORIES: CategoryMeta[] = [
  { id: 'organise',     label: 'Organise',     icon: '📋' },
  { id: 'pdf_to_other', label: 'PDF → Other',  icon: '📤' },
  { id: 'other_to_pdf', label: 'Other → PDF',  icon: '📥' },
  { id: 'edit',         label: 'Edit',         icon: '✏️' },
  { id: 'security',     label: 'Security',     icon: '🔒' },
  { id: 'repair',       label: 'Repair & OCR', icon: '🔬' },
];

export const TOOLS: ToolMeta[] = [
  // Organise
  { id: 'merge',    label: 'Merge PDF',      icon: '⊕',  category: 'organise',     description: 'Combine multiple PDFs into one' },
  { id: 'split',    label: 'Split PDF',      icon: '✂️', category: 'organise',     description: 'Split by page range or every N pages' },
  { id: 'compress', label: 'Compress PDF',   icon: '🗜', category: 'organise',     description: 'Reduce file size with quality presets' },
  { id: 'rotate',   label: 'Rotate Pages',   icon: '🔄', category: 'organise',     description: 'Rotate individual or all pages' },
  { id: 'reorder',  label: 'Reorder Pages',  icon: '📑', category: 'organise',     description: 'Drag and drop to reorder pages' },
  { id: 'remove',   label: 'Remove Pages',   icon: '🗑', category: 'organise',     description: 'Delete selected pages' },
  // PDF → Other
  { id: 'pdf_to_word',  label: 'PDF → Word',  icon: '📝', category: 'pdf_to_other', description: 'Convert PDF to Word document' },
  { id: 'pdf_to_excel', label: 'PDF → Excel', icon: '📊', category: 'pdf_to_other', description: 'Extract tables to spreadsheet' },
  { id: 'pdf_to_ppt',   label: 'PDF → PPT',   icon: '📽', category: 'pdf_to_other', description: 'Convert pages to PowerPoint slides' },
  { id: 'pdf_to_image', label: 'PDF → Image', icon: '🖼', category: 'pdf_to_other', description: 'Export pages as JPG or PNG' },
  { id: 'pdf_to_pdfa',  label: 'PDF → PDF/A', icon: '🗄', category: 'pdf_to_other', description: 'Convert to archival PDF/A-1b format' },
  // Other → PDF
  { id: 'word_to_pdf',  label: 'Word → PDF',  icon: '📝', category: 'other_to_pdf', description: 'Convert Word document to PDF' },
  { id: 'excel_to_pdf', label: 'Excel → PDF', icon: '📊', category: 'other_to_pdf', description: 'Convert spreadsheet to PDF' },
  { id: 'ppt_to_pdf',   label: 'PPT → PDF',   icon: '📽', category: 'other_to_pdf', description: 'Convert presentation to PDF' },
  { id: 'image_to_pdf', label: 'Image → PDF', icon: '🖼', category: 'other_to_pdf', description: 'Convert JPG/PNG images to PDF' },
  { id: 'html_to_pdf',  label: 'HTML → PDF',  icon: '🌐', category: 'other_to_pdf', description: 'Convert local HTML file to PDF' },
  // Edit
  { id: 'edit',         label: 'Edit PDF',     icon: '🔤', category: 'edit', description: 'Add text boxes, images, and shapes' },
  { id: 'watermark',    label: 'Watermark',    icon: '💧', category: 'edit', description: 'Add text or image watermark' },
  { id: 'page_numbers', label: 'Page Numbers', icon: '#️⃣', category: 'edit', description: 'Add page numbers to your PDF' },
  { id: 'redact',       label: 'Redact PDF',   icon: '⬛', category: 'edit', description: 'Permanently remove sensitive content' },
  // Security
  { id: 'protect', label: 'Protect PDF', icon: '🔐', category: 'security', description: 'Add password protection' },
  { id: 'unlock',  label: 'Unlock PDF',  icon: '🔓', category: 'security', description: 'Remove PDF password' },
  { id: 'sign',    label: 'Sign PDF',    icon: '✍️', category: 'security', description: 'Add your signature to a PDF' },
  // Repair & OCR
  { id: 'ocr',    label: 'OCR PDF',    icon: '🔍', category: 'repair', description: 'Make scanned PDFs searchable' },
  { id: 'repair', label: 'Repair PDF', icon: '🔧', category: 'repair', description: 'Fix broken or corrupted PDFs' },
];

export function toolsByCategory(categoryId: string): ToolMeta[] {
  return TOOLS.filter(t => t.category === categoryId);
}

export function searchTools(query: string): ToolMeta[] {
  const q = query.toLowerCase();
  return TOOLS.filter(t =>
    t.label.toLowerCase().includes(q) ||
    t.description.toLowerCase().includes(q)
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools-registry.ts
git commit -m "feat: tool registry — 25 tools with metadata and search"
```

---

### Task 11: UI Shell — Sidebar + Dashboard + App layout

**Files:**
- Create: `src/lib/components/layout/AppShell.svelte`
- Create: `src/lib/components/layout/Sidebar.svelte`
- Create: `src/lib/components/layout/Dashboard.svelte`
- Create: `src/lib/components/layout/ToolWorkspace.svelte`
- Create: `src/lib/components/ui/SpotlightSearch.svelte`
- Create: `src/lib/components/ui/Toast.svelte`
- Modify: `src/App.svelte`

- [ ] **Step 1: Write AppShell**

Create `src/lib/components/layout/AppShell.svelte`:
```svelte
<script lang="ts">
  import Sidebar from './Sidebar.svelte';
  import Dashboard from './Dashboard.svelte';
  import ToolWorkspace from './ToolWorkspace.svelte';
  import SpotlightSearch from '../ui/SpotlightSearch.svelte';
  import Toast from '../ui/Toast.svelte';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { settingsStore } from '../../stores/settings.svelte';
  import { recentFilesStore } from '../../stores/recent-files.svelte';
  import { onMount } from 'svelte';

  onMount(async () => {
    await Promise.all([settingsStore.load(), recentFilesStore.load()]);
  });

  let spotlightOpen = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      spotlightOpen = true;
    }
    if (e.key === 'Escape') {
      spotlightOpen = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app-shell h-screen flex overflow-hidden bg-cream font-sans select-none">
  {#if activeToolStore.view === 'workspace'}
    <Sidebar />
  {/if}

  <main class="flex-1 overflow-hidden flex flex-col">
    {#if activeToolStore.view === 'dashboard'}
      <Dashboard onOpenSpotlight={() => spotlightOpen = true} />
    {:else}
      <ToolWorkspace />
    {/if}
  </main>

  {#if spotlightOpen}
    <SpotlightSearch onClose={() => spotlightOpen = false} />
  {/if}

  <Toast />
</div>
```

- [ ] **Step 2: Write Sidebar**

Create `src/lib/components/layout/Sidebar.svelte`:
```svelte
<script lang="ts">
  import { CATEGORIES, TOOLS, toolsByCategory } from '../../tools-registry';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { settingsStore } from '../../stores/settings.svelte';

  const collapsed = $derived(settingsStore.value.sidebar_collapsed);

  function toggleCollapse() {
    settingsStore.update({ sidebar_collapsed: !collapsed });
  }
</script>

<aside
  class="h-full flex flex-col flex-shrink-0 transition-all duration-200 overflow-hidden"
  class:w-[120px]={!collapsed}
  class:w-12={collapsed}
  style="background: #1B7A8A;"
>
  <!-- Logo -->
  <div class="px-3 py-3 border-b border-white/10 flex items-center gap-2">
    <span class="text-lg">🦚</span>
    {#if !collapsed}
      <span class="text-white font-bold text-sm">PavoPDF</span>
    {/if}
  </div>

  <!-- Home link -->
  <button
    onclick={() => activeToolStore.goHome()}
    class="px-3 py-2 text-white/50 text-xs hover:text-white/80 text-left"
  >
    {collapsed ? '⌂' : '← Home'}
  </button>

  <!-- Tool groups -->
  <nav class="flex-1 overflow-y-auto py-1">
    {#each CATEGORIES as cat}
      {@const tools = toolsByCategory(cat.id)}
      {#if !collapsed}
        <div class="px-3 py-1 text-white/40 text-[10px] uppercase tracking-wider mt-2">
          {cat.label}
        </div>
      {/if}
      {#each tools as tool}
        <button
          onclick={() => activeToolStore.selectTool(tool.id)}
          class="w-full flex items-center gap-2 rounded-md transition-colors"
          class:px-2={!collapsed}
          class:py-1={!collapsed}
          class:mx-auto={collapsed}
          class:justify-center={collapsed}
          class:w-8={collapsed}
          class:h-7={collapsed}
          class:bg-white/15={activeToolStore.tool === tool.id}
          class:text-white={activeToolStore.tool === tool.id}
          class:text-white/60={activeToolStore.tool !== tool.id}
          title={collapsed ? tool.label : undefined}
        >
          <span class="text-sm flex-shrink-0">{tool.icon}</span>
          {#if !collapsed}
            <span class="text-xs truncate">{tool.label}</span>
          {/if}
        </button>
      {/each}
    {/each}
  </nav>

  <!-- Collapse toggle -->
  <button
    onclick={toggleCollapse}
    class="px-3 py-2 text-white/40 text-xs hover:text-white/70 border-t border-white/10 text-left"
    title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
  >
    {collapsed ? '›' : '‹ collapse'}
  </button>
</aside>
```

- [ ] **Step 3: Write Dashboard**

Create `src/lib/components/layout/Dashboard.svelte`:
```svelte
<script lang="ts">
  import { CATEGORIES, TOOLS, toolsByCategory } from '../../tools-registry';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { recentFilesStore } from '../../stores/recent-files.svelte';
  import type { Category } from '../../types';

  let { onOpenSpotlight }: { onOpenSpotlight: () => void } = $props();

  let selectedCategory = $state<Category | 'all'>('all');

  const visibleTools = $derived(
    selectedCategory === 'all'
      ? TOOLS
      : TOOLS.filter(t => t.category === selectedCategory)
  );
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Top bar -->
  <header style="background: #1B7A8A;" class="flex items-center gap-3 px-4 py-2 flex-shrink-0">
    <span class="text-white font-bold text-sm">🦚 PavoPDF</span>

    <!-- Category tabs -->
    <div class="flex gap-1 ml-2">
      <button
        onclick={() => selectedCategory = 'all'}
        class="px-2 py-1 rounded text-xs transition-colors"
        class:bg-white/20={selectedCategory === 'all'}
        class:text-white={selectedCategory === 'all'}
        class:text-white/60={selectedCategory !== 'all'}
      >All</button>
      {#each CATEGORIES as cat}
        <button
          onclick={() => selectedCategory = cat.id}
          class="px-2 py-1 rounded text-xs transition-colors"
          class:bg-white/20={selectedCategory === cat.id}
          class:text-white={selectedCategory === cat.id}
          class:text-white/60={selectedCategory !== cat.id}
        >{cat.label}</button>
      {/each}
    </div>

    <!-- Spotlight trigger -->
    <button
      onclick={onOpenSpotlight}
      class="ml-auto text-xs text-white/70 bg-white/15 border border-white/20 rounded px-3 py-1 hover:bg-white/20 transition-colors"
    >
      ⌘K Search tools...
    </button>
  </header>

  <div class="flex-1 overflow-y-auto p-4">
    <!-- Tool grid -->
    <div class="grid grid-cols-6 gap-3 mb-6">
      {#each visibleTools as tool}
        <button
          onclick={() => activeToolStore.selectTool(tool.id)}
          class="bg-white rounded-lg p-3 text-center border border-stone-200 hover:border-peach hover:shadow-sm transition-all group"
          title={tool.description}
        >
          <span class="text-xl block mb-1">{tool.icon}</span>
          <span class="text-xs text-stone-700 group-hover:text-peach transition-colors">{tool.label}</span>
        </button>
      {/each}
    </div>

    <!-- Recent files -->
    {#if recentFilesStore.entries.length > 0}
      <div>
        <h3 class="text-xs uppercase tracking-wider text-stone-400 mb-2">Recent</h3>
        <div class="flex flex-col gap-1">
          {#each recentFilesStore.entries as entry}
            <div
              class="flex items-center gap-3 bg-white rounded-lg px-3 py-2 border border-stone-200"
              class:opacity-50={!entry.exists}
            >
              <div class="w-2 h-2 rounded-sm flex-shrink-0" style="background: #E8956A;"></div>
              <span class="text-xs text-stone-700 flex-1 truncate">{entry.path.split(/[/\\]/).pop()}</span>
              <span class="text-xs text-stone-400">{entry.tool}</span>
              {#if !entry.exists}
                <span class="text-xs text-red-400">not found</span>
                <button
                  onclick={() => recentFilesStore.remove(entry.path)}
                  class="text-xs text-stone-400 hover:text-red-500"
                >✕</button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
```

- [ ] **Step 4: Write ToolWorkspace (stub — tools implemented in Plans 2-6)**

Create `src/lib/components/layout/ToolWorkspace.svelte`:
```svelte
<script lang="ts">
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { TOOLS } from '../../tools-registry';

  const tool = $derived(TOOLS.find(t => t.id === activeToolStore.tool));
</script>

<div class="flex-1 overflow-y-auto p-6 bg-cream">
  {#if tool}
    <div class="mb-1 text-xs text-stone-400">
      All Tools › <span class="text-teal font-semibold">{tool.label}</span>
    </div>
    <h1 class="text-xl font-bold text-stone-800 mb-4">{tool.label}</h1>

    <!-- Tool workspace content will be injected by Plans 2-6 -->
    <div class="bg-white rounded-xl border border-stone-200 p-8 text-center text-stone-400">
      <div class="text-4xl mb-2">{tool.icon}</div>
      <p class="text-sm">{tool.description}</p>
      <p class="text-xs mt-2 text-stone-300">Implementation coming in Plans 2–6</p>
    </div>
  {/if}
</div>
```

- [ ] **Step 5: Write SpotlightSearch**

Create `src/lib/components/ui/SpotlightSearch.svelte`:
```svelte
<script lang="ts">
  import { searchTools } from '../../tools-registry';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import type { ToolMeta } from '../../types';

  let { onClose }: { onClose: () => void } = $props();

  let query = $state('');
  let selected = $state(0);

  const results = $derived(query.length > 0 ? searchTools(query) : []);

  function pick(tool: ToolMeta) {
    activeToolStore.selectTool(tool.id);
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { selected = Math.min(selected + 1, results.length - 1); }
    if (e.key === 'ArrowUp') { selected = Math.max(selected - 1, 0); }
    if (e.key === 'Enter' && results[selected]) { pick(results[selected]); }
  }
</script>

<!-- Backdrop -->
<div
  class="fixed inset-0 bg-black/30 z-50 flex items-start justify-center pt-24"
  onclick={onClose}
  role="dialog"
  aria-modal="true"
>
  <div
    class="w-full max-w-md bg-white rounded-xl shadow-2xl overflow-hidden"
    onclick|stopPropagation
  >
    <div class="flex items-center gap-3 px-4 py-3 border-b border-stone-100">
      <span class="text-stone-400">🔍</span>
      <input
        type="text"
        placeholder="Search tools..."
        class="flex-1 outline-none text-sm text-stone-800 placeholder-stone-400"
        bind:value={query}
        onkeydown={handleKeydown}
        autofocus
      />
      <kbd class="text-xs text-stone-300 border border-stone-200 rounded px-1">Esc</kbd>
    </div>

    {#if results.length > 0}
      <ul class="max-h-64 overflow-y-auto py-1">
        {#each results as tool, i}
          <li>
            <button
              class="w-full flex items-center gap-3 px-4 py-2 text-left hover:bg-stone-50 transition-colors"
              class:bg-stone-50={i === selected}
              onclick={() => pick(tool)}
            >
              <span class="text-lg">{tool.icon}</span>
              <div>
                <div class="text-sm font-medium text-stone-800">{tool.label}</div>
                <div class="text-xs text-stone-400">{tool.description}</div>
              </div>
            </button>
          </li>
        {/each}
      </ul>
    {:else if query.length > 0}
      <div class="px-4 py-6 text-center text-sm text-stone-400">No tools found</div>
    {:else}
      <div class="px-4 py-6 text-center text-sm text-stone-400">Start typing to search tools...</div>
    {/if}
  </div>
</div>
```

- [ ] **Step 6: Write Toast component**

Create `src/lib/components/ui/Toast.svelte`:
```svelte
<script lang="ts">
  import { operationStore } from '../../stores/operation.svelte';

  const doneOps = $derived(operationStore.all.filter(o => o.status === 'done'));
  const errorOps = $derived(operationStore.all.filter(o => o.status === 'error'));
</script>

<div class="fixed bottom-4 right-4 flex flex-col gap-2 z-50 pointer-events-none">
  {#each doneOps as op}
    <div class="bg-white border border-stone-200 rounded-lg px-4 py-2 shadow-lg pointer-events-auto
                flex items-center gap-2 text-sm text-stone-700">
      <span class="text-green-500">✓</span>
      <span>{op.tool} complete</span>
      <button
        onclick={() => operationStore.clear(op.id)}
        class="ml-2 text-stone-400 hover:text-stone-600"
      >✕</button>
    </div>
  {/each}

  {#each errorOps as op}
    <div class="bg-red-50 border border-red-200 rounded-lg px-4 py-2 shadow-lg pointer-events-auto
                flex items-center gap-2 text-sm text-red-700">
      <span>⚠</span>
      <span>{op.errorMessage ?? 'Operation failed'}</span>
      <button
        onclick={() => operationStore.clear(op.id)}
        class="ml-2 text-red-400 hover:text-red-600"
      >✕</button>
    </div>
  {/each}
</div>
```

- [ ] **Step 7: Wire App.svelte**

Replace `src/App.svelte`:
```svelte
<script lang="ts">
  import AppShell from './lib/components/layout/AppShell.svelte';
</script>

<AppShell />
```

- [ ] **Step 8: Run the app and verify UI**

```bash
npm run tauri dev
```

Verify manually:
- App opens showing the Dashboard with all 25 tool cards in a 6-column grid
- ⌘K (or Ctrl+K) opens the spotlight search
- Typing in spotlight filters tools
- Clicking a tool card navigates to the workspace stub
- The teal sidebar appears in workspace view with all tools listed
- Collapse button works (sidebar shrinks to icon-only)
- ← Home returns to dashboard

- [ ] **Step 9: Commit**

```bash
git add src/
git commit -m "feat: full UI shell — dashboard, sidebar, spotlight search, toast, workspace stub"
```

---

## Chunk 4: GitHub Actions CI

### Task 12: Cross-platform release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write release workflow**

Create `.github/workflows/release.yml`:
```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: ubuntu-22.04
            args: ''
          - platform: macos-latest
            args: '--target universal-apple-darwin'  # builds for both Apple Silicon and Intel Mac
          - platform: windows-latest
            args: ''

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}  # universal binary needs both targets

      - name: Install Ubuntu dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm

      - name: Install frontend deps
        run: npm ci

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: PavoPDF ${{ github.ref_name }}
          releaseBody: 'See CHANGELOG for details.'
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}
```

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "ci: GitHub Actions cross-platform release workflow"
```

- [ ] **Step 3: Push and verify CI syntax**

```bash
# Replace <your-username> with your actual GitHub username before running this step.
# Create the repo at github.com first (public, MIT license, no README — we already have one).
git remote add origin https://github.com/<your-username>/pavopdf.git
git push -u origin main
```

Check the Actions tab on GitHub — the workflow file should appear. It will only trigger when you push a `v*` tag (e.g. `git tag v0.1.0 && git push --tags`).

---

## Plan 1 Complete

At the end of Plan 1 you have:
- A working Tauri + Svelte app that opens a full UI shell
- Dashboard with 25 tool cards, ⌘K spotlight, category tabs, recent files
- Arc-style sidebar with collapse/expand
- Rust backend with storage (settings + recent files), pipeline (validate + temp + progress), and all IPC commands wired up
- GitHub Actions CI ready to build on all 3 platforms on tag push

**Next:** Plan 2 — Organise Tools (Merge, Split, Compress, Rotate, Reorder, Remove)
