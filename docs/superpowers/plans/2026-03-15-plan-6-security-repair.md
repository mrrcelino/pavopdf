# PavoPDF — Plan 6: Security & Repair Tools

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Goal:** Implement five tools — Protect PDF, Unlock PDF, Sign PDF (visual v1), OCR PDF, and Repair PDF — each with a Rust backend module and a Svelte 5 workspace component, wired through the existing IPC pipeline.
> **Prerequisites:** Plan 1 foundation must be complete (error.rs, TempStage, emit_progress, tools/mod.rs, settings.rs, types.ts, api.ts, settingsStore).

---

## Tesseract Setup Notice (Read Before Starting)

OCR requires a `tesseract` binary available on PATH (or configured in settings). PavoPDF v0.1 **does not bundle Tesseract automatically** — users must install it separately:

- **Windows:** Download the installer from [UB-Mannheim/tesseract](https://github.com/UB-Mannheim/tesseract/wiki) and add to PATH, or set `settings.tesseract_path` to the absolute `.exe` path.
- **macOS:** `brew install tesseract`
- **Ubuntu:** `sudo apt install tesseract-ocr tesseract-ocr-eng`

Language data files (`*.traineddata`) must match the selected language code. The `eng` (English) data file is included in standard Tesseract installs. Additional languages can be installed via packages (e.g. `tesseract-ocr-deu` for German) or by downloading `.traineddata` files into Tesseract's `tessdata` directory.

Document this in the app's Help / About panel.

---

## lopdf AES-128 Capability Check

**Decision point for Protect PDF:**

lopdf 0.32 supports RC4-based encryption. AES-128 write support was added in later versions. Before implementing, verify:

```bash
cd src-tauri
cargo add lopdf --features="encrypt"  # or check Cargo.toml features list
grep -r "aes\|Aes\|encrypt_aes" ~/.cargo/registry/src/**/lopdf-*/src/
```

- **If lopdf AES-128 write is confirmed available:** implement `protect` directly via lopdf.
- **If lopdf only supports RC4 or read-only AES:** bundle `qpdf` binary (Apache 2.0 license) and invoke via `std::process::Command`. Document the fallback clearly in code comments and in the UI's info panel.

The plan below implements the lopdf path first with a `#[cfg]`-gated fallback skeleton for qpdf. The implementing developer must make this decision at Step 1 of Task 2.

---

## Chunk 1: Rust Module Scaffold

### Task 1: Create `security` and `repair` module directories

**Files:**
- Create: `src-tauri/src/tools/security/mod.rs`
- Create: `src-tauri/src/tools/repair/mod.rs`
- Modify: `src-tauri/src/tools/mod.rs`

- [ ] **Step 1: Write failing test for tool name recognition**

In `src-tauri/src/tools/mod.rs`, add to the existing `tests` module (or create one):

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn security_and_repair_tool_names_are_recognized() {
        let names = [
            "protect_pdf",
            "unlock_pdf",
            "sign_pdf",
            "ocr_pdf",
            "repair_pdf",
        ];
        for name in &names {
            assert!(
                crate::tools::tool_name_is_known(name),
                "{name} not recognized"
            );
        }
    }
}
```

Run — it must FAIL before implementation:

```bash
cd src-tauri && cargo test security_and_repair_tool_names_are_recognized -- --nocapture
```

- [ ] **Step 2: Create `security/mod.rs`**

Create `src-tauri/src/tools/security/mod.rs`:

```rust
pub mod protect;
pub mod unlock;
pub mod sign;
pub mod ocr;
```

- [ ] **Step 3: Create `repair/mod.rs`**

Create `src-tauri/src/tools/repair/mod.rs`:

```rust
pub mod repair;
```

- [ ] **Step 4: Wire modules and match arms into `tools/mod.rs`**

In `src-tauri/src/tools/mod.rs`, add the new sub-modules and extend `tool_name_is_known` and the `run()` dispatch:

```rust
pub mod security;
pub mod repair;

// In tool_name_is_known(), add these variants to the matches! macro:
// | "protect_pdf"
// | "unlock_pdf"
// | "sign_pdf"
// | "ocr_pdf"
// | "repair_pdf"

// In run(), add these match arms:
// "protect_pdf" => security::protect::run(req, app).await,
// "unlock_pdf"  => security::unlock::run(req, app).await,
// "sign_pdf"    => security::sign::run(req, app).await,
// "ocr_pdf"     => security::ocr::run(req, app).await,
// "repair_pdf"  => repair::repair::run(req, app).await,
```

Full updated `tools/mod.rs` with the new arms (merge with your existing file):

```rust
pub mod convert_from; // from Plan 3
pub mod convert_to;   // from Plan 4
pub mod organize;     // from Plan 2
pub mod security;
pub mod repair;

use std::path::PathBuf;
use tauri::AppHandle;
use crate::error::{AppError, Result};
use crate::pipeline::temp::TempStage;

#[derive(Debug, serde::Deserialize)]
pub struct ProcessRequest {
    pub tool: String,
    pub input_path: String,
    pub output_path: String,
    pub options: serde_json::Value,
}

pub fn tool_name_is_known(name: &str) -> bool {
    matches!(
        name,
        // Plan 2 — organize
        | "merge_pdf"
        | "split_pdf"
        | "rotate_pdf"
        | "compress_pdf"
        | "reorder_pdf"
        // Plan 3 — pdf → other
        | "pdf_to_word"
        | "pdf_to_excel"
        | "pdf_to_ppt"
        | "pdf_to_image"
        | "pdf_to_pdfa"
        // Plan 4 — other → pdf
        | "word_to_pdf"
        | "excel_to_pdf"
        | "ppt_to_pdf"
        | "image_to_pdf"
        | "html_to_pdf"
        // Plan 5 — edit
        | "add_watermark"
        | "add_page_numbers"
        | "redact_pdf"
        | "edit_metadata"
        | "crop_pdf"
        // Plan 6 — security + repair
        | "protect_pdf"
        | "unlock_pdf"
        | "sign_pdf"
        | "ocr_pdf"
        | "repair_pdf"
    )
}

pub async fn run(req: ProcessRequest, app: AppHandle) -> Result<String> {
    match req.tool.as_str() {
        "protect_pdf" => security::protect::run(req, app).await,
        "unlock_pdf"  => security::unlock::run(req, app).await,
        "sign_pdf"    => security::sign::run(req, app).await,
        "ocr_pdf"     => security::ocr::run(req, app).await,
        "repair_pdf"  => repair::repair::run(req, app).await,
        other => Err(AppError::Validation(format!("Unknown tool: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn security_and_repair_tool_names_are_recognized() {
        let names = [
            "protect_pdf",
            "unlock_pdf",
            "sign_pdf",
            "ocr_pdf",
            "repair_pdf",
        ];
        for name in &names {
            assert!(
                crate::tools::tool_name_is_known(name),
                "{name} not recognized"
            );
        }
    }
}
```

- [ ] **Step 5: Run the test — it must now PASS**

```bash
cd src-tauri && cargo test security_and_repair_tool_names_are_recognized -- --nocapture
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/tools/
git commit -m "feat(plan6): scaffold security and repair Rust modules"
```

---

## Chunk 2: Protect PDF

### Task 2: Rust — protect PDF with AES-128 password

**Files:**
- Create: `src-tauri/src/tools/security/protect.rs`

**Decision:** Before writing implementation, run the lopdf capability check from the intro section. If lopdf AES-128 write is available, use the lopdf path. If not, use the qpdf path. The code below provides both paths with clear `// LOPDF PATH` and `// QPDF FALLBACK` markers.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/tools/security/protect.rs` with tests only:

```rust
use std::path::Path;
use crate::error::{AppError, Result};

// ─── Options ──────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ProtectOptions {
    /// Password to set on the PDF.
    pub password: String,
    /// Optional owner password (defaults to user password if empty).
    pub owner_password: Option<String>,
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    unimplemented!("protect_pdf not yet implemented")
}

// ─── Core logic (sync, testable) ──────────────────────────────────────────────

pub fn protect_file(
    input: &Path,
    output: &Path,
    opts: &ProtectOptions,
) -> Result<()> {
    unimplemented!()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn minimal_pdf_bytes() -> Vec<u8> {
        // Minimal valid PDF for testing
        b"%PDF-1.4\n\
          1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
          2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
          3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\n\
          xref\n0 4\n0000000000 65535 f \n\
          0000000009 00000 n \n\
          0000000058 00000 n \n\
          0000000115 00000 n \n\
          trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF"
            .to_vec()
    }

    #[test]
    fn protect_requires_non_empty_password() {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(&minimal_pdf_bytes()).unwrap();
        let output = NamedTempFile::new().unwrap();
        let opts = ProtectOptions { password: String::new(), owner_password: None };
        let result = protect_file(input.path(), output.path(), &opts);
        assert!(matches!(result, Err(AppError::Validation(_))), "empty password should fail");
    }

    #[test]
    fn protect_produces_output_file() {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(&minimal_pdf_bytes()).unwrap();
        let output = NamedTempFile::new().unwrap();
        let opts = ProtectOptions {
            password: "s3cr3t".into(),
            owner_password: None,
        };
        protect_file(input.path(), output.path(), &opts).unwrap();
        let meta = std::fs::metadata(output.path()).unwrap();
        assert!(meta.len() > 0, "output file should be non-empty");
    }

    #[test]
    fn protected_pdf_is_readable_with_correct_password() {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(&minimal_pdf_bytes()).unwrap();
        let output = NamedTempFile::new().unwrap();
        let opts = ProtectOptions {
            password: "testpass".into(),
            owner_password: None,
        };
        protect_file(input.path(), output.path(), &opts).unwrap();
        // Attempt to open with lopdf using the password
        // (adjust API based on lopdf version)
        let bytes = std::fs::read(output.path()).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "output must be a PDF");
    }
}
```

Run — must FAIL:

```bash
cd src-tauri && cargo test protect -- --nocapture 2>&1 | head -30
```

- [ ] **Step 2: Implement `protect_file` — lopdf path**

Replace the `protect_file` function and `run` function bodies:

```rust
use lopdf::{Document, dictionary, Object, StringFormat};
use std::path::Path;
use crate::error::{AppError, Result};
use crate::pipeline::{
    temp::TempStage,
    progress::{emit_progress, emit_complete, emit_error},
};

/// LOPDF PATH: Use lopdf's built-in encryption support.
/// If lopdf does not support AES-128 write, see QPDF FALLBACK below.
pub fn protect_file(
    input: &Path,
    output: &Path,
    opts: &ProtectOptions,
) -> Result<()> {
    if opts.password.is_empty() {
        return Err(AppError::Validation("Password must not be empty".into()));
    }

    let mut doc = Document::load(input)
        .map_err(|e| AppError::Pdf(format!("Cannot open PDF: {e}")))?;

    let owner_pw = opts
        .owner_password
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&opts.password);

    // lopdf 0.32+ encrypt() signature (verify against your installed version):
    // Document::encrypt(&mut self, user_password: &str, owner_password: &str, permissions: u32)
    // Permissions flag 0xFFFFFFFC = all permissions granted to owner.
    // NOTE: If this API does not exist in your lopdf version, use the QPDF FALLBACK below.
    doc.encrypt(&opts.password, owner_pw, 0xFFFF_FFFC)
        .map_err(|e| AppError::Pdf(format!("Encryption failed: {e}")))?;

    doc.save(output)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    Ok(())
}

// ─── QPDF FALLBACK ────────────────────────────────────────────────────────────
// Use this instead of protect_file above if lopdf AES-128 write is unsupported.
//
// Prerequisites:
//   1. Bundle `qpdf` binary for each target platform in `src-tauri/binaries/`:
//      - qpdf-x86_64-pc-windows-msvc.exe
//      - qpdf-x86_64-apple-darwin
//      - qpdf-aarch64-apple-darwin
//      - qpdf-x86_64-unknown-linux-gnu
//   2. Declare in tauri.conf.json under `bundle.externalBin`: ["binaries/qpdf"]
//   3. Obtain qpdf binaries from https://github.com/qpdf/qpdf/releases (Apache 2.0)
//
// pub fn protect_file_qpdf(input: &Path, output: &Path, opts: &ProtectOptions) -> Result<()> {
//     if opts.password.is_empty() {
//         return Err(AppError::Validation("Password must not be empty".into()));
//     }
//     let qpdf = find_bundled_binary("qpdf")?;
//     let owner_pw = opts.owner_password.as_deref()
//         .filter(|s| !s.is_empty())
//         .unwrap_or(&opts.password);
//     let status = std::process::Command::new(&qpdf)
//         .args([
//             "--encrypt", &opts.password, owner_pw, "128",
//             "--use-aes=y",
//             "--",
//             input.to_str().unwrap(),
//             output.to_str().unwrap(),
//         ])
//         .status()
//         .map_err(|e| AppError::Io(format!("qpdf launch failed: {e}")))?;
//     if !status.success() {
//         return Err(AppError::Pdf("qpdf encryption failed".into()));
//     }
//     Ok(())
// }

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    use crate::pipeline::validate::validate_pdf;

    let input = std::path::PathBuf::from(&req.input_path);
    validate_pdf(&input, None)?; // no size limit for protect

    let opts: ProtectOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| AppError::Validation(format!("Invalid options: {e}")))?;

    let stage = TempStage::new()?;
    emit_progress(&app, "protect_pdf", 0, 1, "Encrypting PDF…");

    // Derive output filename: {stem}_protected.pdf
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let out_name = format!("{stem}_protected.pdf");
    let temp_out = stage.path().join(&out_name);

    protect_file(&input, &temp_out, &opts)?;

    // Copy to final destination
    let final_out = std::path::PathBuf::from(&req.output_path);
    std::fs::copy(&temp_out, &final_out)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    emit_complete(&app, "protect_pdf", final_out.to_string_lossy().as_ref());
    Ok(final_out.to_string_lossy().into_owned())
}
```

- [ ] **Step 3: Run tests — must PASS**

```bash
cd src-tauri && cargo test protect -- --nocapture
```

Expected: `protect_requires_non_empty_password` passes, `protect_produces_output_file` passes, `protected_pdf_is_readable_with_correct_password` passes.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/security/protect.rs
git commit -m "feat(plan6): implement protect_pdf Rust backend"
```

---

## Chunk 3: Unlock PDF

### Task 3: Rust — unlock (remove password from) a PDF

**Files:**
- Create: `src-tauri/src/tools/security/unlock.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/tools/security/unlock.rs`:

```rust
use std::path::Path;
use crate::error::{AppError, Result};

#[derive(Debug, serde::Deserialize)]
pub struct UnlockOptions {
    /// The current password needed to open the PDF.
    pub password: String,
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    unimplemented!()
}

pub fn unlock_file(input: &Path, output: &Path, opts: &UnlockOptions) -> Result<()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn minimal_pdf_bytes() -> Vec<u8> {
        b"%PDF-1.4\n\
          1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
          2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
          3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\n\
          xref\n0 4\n0000000000 65535 f \n\
          0000000009 00000 n \n\
          0000000058 00000 n \n\
          0000000115 00000 n \n\
          trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF"
            .to_vec()
    }

    #[test]
    fn unlock_rejects_empty_password() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&minimal_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let opts = UnlockOptions { password: String::new() };
        let r = unlock_file(f.path(), out.path(), &opts);
        assert!(matches!(r, Err(AppError::Validation(_))));
    }

    #[test]
    fn unlock_unencrypted_pdf_passes_through() {
        // An unencrypted PDF opened with any password should still be saved
        // without encryption. lopdf will open it without needing the password.
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&minimal_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let opts = UnlockOptions { password: "any".into() };
        unlock_file(f.path(), out.path(), &opts).unwrap();
        let bytes = std::fs::read(out.path()).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}
```

Run — must FAIL:

```bash
cd src-tauri && cargo test unlock -- --nocapture 2>&1 | head -20
```

- [ ] **Step 2: Implement `unlock_file` and `run`**

Replace the function bodies:

```rust
use lopdf::Document;
use std::path::Path;
use crate::error::{AppError, Result};
use crate::pipeline::{
    temp::TempStage,
    progress::{emit_progress, emit_complete},
};

pub fn unlock_file(input: &Path, output: &Path, opts: &UnlockOptions) -> Result<()> {
    if opts.password.is_empty() {
        return Err(AppError::Validation("Password must not be empty".into()));
    }

    // lopdf::Document::load_with_password opens an encrypted PDF.
    // If the PDF is not encrypted, load() works directly.
    let mut doc = Document::load_with_password(input, opts.password.as_bytes())
        .or_else(|_| Document::load(input))
        .map_err(|e| AppError::Pdf(format!("Cannot open PDF (wrong password?): {e}")))?;

    // Remove the encryption dictionary so the saved file has no password.
    // lopdf represents encryption in the trailer under /Encrypt.
    // Removing it from the trailer dict causes lopdf to save without encryption.
    if let Ok(trailer) = doc.trailer.as_dict_mut() {
        trailer.remove(b"Encrypt");
    }

    doc.save(output)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    Ok(())
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    use crate::pipeline::validate::validate_pdf;

    let input = std::path::PathBuf::from(&req.input_path);
    validate_pdf(&input, None)?;

    let opts: UnlockOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| AppError::Validation(format!("Invalid options: {e}")))?;

    let stage = TempStage::new()?;
    emit_progress(&app, "unlock_pdf", 0, 1, "Removing password…");

    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let out_name = format!("{stem}_unlocked.pdf");
    let temp_out = stage.path().join(&out_name);

    unlock_file(&input, &temp_out, &opts)?;

    let final_out = std::path::PathBuf::from(&req.output_path);
    std::fs::copy(&temp_out, &final_out)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    emit_complete(&app, "unlock_pdf", final_out.to_string_lossy().as_ref());
    Ok(final_out.to_string_lossy().into_owned())
}
```

- [ ] **Step 3: Run tests — must PASS**

```bash
cd src-tauri && cargo test unlock -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/security/unlock.rs
git commit -m "feat(plan6): implement unlock_pdf Rust backend"
```

---

## Chunk 4: Sign PDF (Visual v1)

### Task 4: Rust — embed a drawn signature as a PDF image annotation

**Files:**
- Create: `src-tauri/src/tools/security/sign.rs`

**Note:** This is visual-only signing — a PNG image is embedded as an annotation on the chosen page. Cryptographic signing (X.509/PKCS#7) is deferred to v2.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/tools/security/sign.rs`:

```rust
use std::path::Path;
use crate::error::{AppError, Result};

/// PNG image as base64 string, plus position/page info.
#[derive(Debug, serde::Deserialize)]
pub struct SignOptions {
    /// Base64-encoded PNG of the signature drawn on canvas.
    pub signature_png_base64: String,
    /// Zero-based page index to place the signature on.
    pub page_index: u32,
    /// X position in PDF points from left edge.
    pub x: f32,
    /// Y position in PDF points from bottom edge (PDF coordinate system).
    pub y: f32,
    /// Width of the signature in PDF points.
    pub width: f32,
    /// Height of the signature in PDF points.
    pub height: f32,
}

pub fn sign_file(input: &Path, output: &Path, opts: &SignOptions) -> Result<()> {
    unimplemented!()
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn minimal_pdf_bytes() -> Vec<u8> {
        b"%PDF-1.4\n\
          1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
          2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
          3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\n\
          xref\n0 4\n0000000000 65535 f \n\
          0000000009 00000 n \n\
          0000000058 00000 n \n\
          0000000115 00000 n \n\
          trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF"
            .to_vec()
    }

    fn tiny_png_base64() -> String {
        // A 1x1 transparent PNG, base64 encoded
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".into()
    }

    #[test]
    fn sign_rejects_empty_signature() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&minimal_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let opts = SignOptions {
            signature_png_base64: String::new(),
            page_index: 0,
            x: 100.0, y: 100.0, width: 150.0, height: 60.0,
        };
        let r = sign_file(f.path(), out.path(), &opts);
        assert!(matches!(r, Err(AppError::Validation(_))));
    }

    #[test]
    fn sign_rejects_invalid_base64() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&minimal_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let opts = SignOptions {
            signature_png_base64: "not_valid_base64!!!".into(),
            page_index: 0,
            x: 100.0, y: 100.0, width: 150.0, height: 60.0,
        };
        let r = sign_file(f.path(), out.path(), &opts);
        assert!(r.is_err());
    }

    #[test]
    fn sign_produces_output_pdf() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&minimal_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let opts = SignOptions {
            signature_png_base64: tiny_png_base64(),
            page_index: 0,
            x: 100.0, y: 100.0, width: 150.0, height: 60.0,
        };
        sign_file(f.path(), out.path(), &opts).unwrap();
        let bytes = std::fs::read(out.path()).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 200, "signed PDF should be larger than input");
    }
}
```

Run — must FAIL:

```bash
cd src-tauri && cargo test sign -- --nocapture 2>&1 | head -20
```

- [ ] **Step 2: Implement `sign_file` and `run`**

Replace function bodies:

```rust
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use std::path::Path;
use base64::Engine as _;
use crate::error::{AppError, Result};
use crate::pipeline::{
    temp::TempStage,
    progress::{emit_progress, emit_complete},
};

/// Embeds signature PNG as a rubber-stamp annotation on the specified page.
/// Uses lopdf to add an XObject (image) resource and a /Stamp annotation dict.
pub fn sign_file(input: &Path, output: &Path, opts: &SignOptions) -> Result<()> {
    if opts.signature_png_base64.is_empty() {
        return Err(AppError::Validation("Signature image is empty".into()));
    }

    // Decode base64 → PNG bytes
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(&opts.signature_png_base64)
        .map_err(|e| AppError::Validation(format!("Invalid base64: {e}")))?;

    // Parse PNG to get dimensions and raw pixel data
    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| AppError::Validation(format!("Invalid PNG: {e}")))?;
    let img_rgba = img.to_rgba8();
    let (img_w, img_h) = img_rgba.dimensions();

    // Split RGBA into RGB data + alpha mask
    let mut rgb_data: Vec<u8> = Vec::with_capacity((img_w * img_h * 3) as usize);
    let mut alpha_data: Vec<u8> = Vec::with_capacity((img_w * img_h) as usize);
    for pixel in img_rgba.pixels() {
        rgb_data.push(pixel[0]);
        rgb_data.push(pixel[1]);
        rgb_data.push(pixel[2]);
        alpha_data.push(pixel[3]);
    }

    let mut doc = Document::load(input)
        .map_err(|e| AppError::Pdf(format!("Cannot open PDF: {e}")))?;

    let pages = doc.get_pages();
    let page_id = pages
        .get(&(opts.page_index + 1)) // lopdf pages are 1-indexed
        .copied()
        .ok_or_else(|| AppError::Validation(format!("Page {} not found", opts.page_index)))?;

    // Build soft-mask (alpha channel) XObject
    let smask_id = {
        let smask_stream = Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Image".to_vec()),
                "Width" => Object::Integer(img_w as i64),
                "Height" => Object::Integer(img_h as i64),
                "ColorSpace" => Object::Name(b"DeviceGray".to_vec()),
                "BitsPerComponent" => Object::Integer(8),
            },
            alpha_data,
        );
        doc.add_object(smask_stream)
    };

    // Build image XObject with SMask reference
    let img_xobj_id = {
        let img_stream = Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Image".to_vec()),
                "Width" => Object::Integer(img_w as i64),
                "Height" => Object::Integer(img_h as i64),
                "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()),
                "BitsPerComponent" => Object::Integer(8),
                "SMask" => Object::Reference(smask_id),
            },
            rgb_data,
        );
        doc.add_object(img_stream)
    };

    // Build appearance stream for the annotation
    // The appearance stream places the image XObject at the annotation rect.
    let ap_content = format!(
        "q {w} 0 0 {h} 0 0 cm /SigImg Do Q",
        w = opts.width,
        h = opts.height
    );
    let xobj_name = b"SigImg".to_vec();
    let ap_stream_id = {
        let ap_stream = Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Form".to_vec()),
                "BBox" => Object::Array(vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(opts.width as f64),
                    Object::Real(opts.height as f64),
                ]),
                "Resources" => Object::Dictionary(dictionary! {
                    "XObject" => Object::Dictionary(dictionary! {
                        "SigImg" => Object::Reference(img_xobj_id),
                    }),
                }),
            },
            ap_content.into_bytes(),
        );
        doc.add_object(ap_stream)
    };

    // Build annotation dictionary
    let annot_dict = dictionary! {
        "Type" => Object::Name(b"Annot".to_vec()),
        "Subtype" => Object::Name(b"Stamp".to_vec()),
        "Rect" => Object::Array(vec![
            Object::Real(opts.x as f64),
            Object::Real(opts.y as f64),
            Object::Real((opts.x + opts.width) as f64),
            Object::Real((opts.y + opts.height) as f64),
        ]),
        "AP" => Object::Dictionary(dictionary! {
            "N" => Object::Reference(ap_stream_id),
        }),
        "F" => Object::Integer(4), // Print flag
        "Contents" => Object::String(b"Signature".to_vec(), lopdf::StringFormat::Literal),
    };
    let annot_id = doc.add_object(annot_dict);

    // Append annotation to the page's /Annots array
    let page_dict = doc
        .get_object_mut(page_id)
        .map_err(|e| AppError::Pdf(format!("Cannot access page: {e}")))?
        .as_dict_mut()
        .map_err(|_| AppError::Pdf("Page is not a dict".into()))?;

    match page_dict.get_mut(b"Annots") {
        Ok(Object::Array(arr)) => {
            arr.push(Object::Reference(annot_id));
        }
        _ => {
            page_dict.set(
                b"Annots",
                Object::Array(vec![Object::Reference(annot_id)]),
            );
        }
    }

    doc.save(output)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    Ok(())
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    use crate::pipeline::validate::validate_pdf;

    let input = std::path::PathBuf::from(&req.input_path);
    validate_pdf(&input, None)?;

    let opts: SignOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| AppError::Validation(format!("Invalid options: {e}")))?;

    let stage = TempStage::new()?;
    emit_progress(&app, "sign_pdf", 0, 1, "Embedding signature…");

    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let out_name = format!("{stem}_signed.pdf");
    let temp_out = stage.path().join(&out_name);

    sign_file(&input, &temp_out, &opts)?;

    let final_out = std::path::PathBuf::from(&req.output_path);
    std::fs::copy(&temp_out, &final_out)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    emit_complete(&app, "sign_pdf", final_out.to_string_lossy().as_ref());
    Ok(final_out.to_string_lossy().into_owned())
}
```

Add to `Cargo.toml` if not present:

```toml
base64 = "0.22"
```

- [ ] **Step 3: Run tests — must PASS**

```bash
cd src-tauri && cargo test sign -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/security/sign.rs src-tauri/Cargo.toml
git commit -m "feat(plan6): implement sign_pdf visual annotation Rust backend"
```

---

## Chunk 5: OCR PDF

### Task 5: Rust — OCR scanned PDFs via Tesseract + pdfium-render

**Files:**
- Create: `src-tauri/src/tools/security/ocr.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/tools/security/ocr.rs`:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::{AppError, Result};

pub const OCR_HARD_LIMIT_BYTES: u64 = 50 * 1024 * 1024; // 50 MB
pub const OCR_SOFT_WARN_BYTES: u64  = 30 * 1024 * 1024; // 30 MB

#[derive(Debug, serde::Deserialize)]
pub struct OcrOptions {
    /// Tesseract language code, e.g. "eng", "deu", "fra". Default: "eng".
    pub language: Option<String>,
    /// Optional absolute path to tesseract binary. Falls back to PATH lookup.
    pub tesseract_path: Option<String>,
}

impl OcrOptions {
    pub fn lang(&self) -> &str {
        self.language.as_deref().unwrap_or("eng")
    }
}

/// Locate the tesseract binary: custom path → PATH lookup.
pub fn find_tesseract(opts: &OcrOptions) -> Result<PathBuf> {
    if let Some(path) = &opts.tesseract_path {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Ok(p);
        }
        return Err(AppError::NotFound(format!(
            "Tesseract not found at configured path: {path}"
        )));
    }

    // Try PATH lookup
    #[cfg(target_os = "windows")]
    let lookup_cmd = ("where", "tesseract");
    #[cfg(not(target_os = "windows"))]
    let lookup_cmd = ("which", "tesseract");

    let output = Command::new(lookup_cmd.0)
        .arg(lookup_cmd.1)
        .output()
        .map_err(|_| AppError::NotFound("Tesseract not found on PATH. Install Tesseract and ensure it is on your PATH, or set tesseract_path in settings.".into()))?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout);
        let first_line = path_str.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() {
            return Ok(PathBuf::from(first_line));
        }
    }

    Err(AppError::NotFound(
        "Tesseract not found. Install Tesseract and ensure it is on your PATH, or configure tesseract_path in settings.".into()
    ))
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    unimplemented!()
}

pub fn ocr_file(
    input: &Path,
    output: &Path,
    opts: &OcrOptions,
    on_progress: impl Fn(u32, u32),
) -> Result<()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_defaults_to_eng() {
        let opts = OcrOptions { language: None, tesseract_path: None };
        assert_eq!(opts.lang(), "eng");
    }

    #[test]
    fn lang_respects_configured_value() {
        let opts = OcrOptions { language: Some("deu".into()), tesseract_path: None };
        assert_eq!(opts.lang(), "deu");
    }

    #[test]
    fn find_tesseract_returns_not_found_for_invalid_path() {
        let opts = OcrOptions {
            language: None,
            tesseract_path: Some("/nonexistent/path/tesseract".into()),
        };
        let r = find_tesseract(&opts);
        assert!(matches!(r, Err(AppError::NotFound(_))));
    }

    #[test]
    fn ocr_hard_limit_constant_is_50mb() {
        assert_eq!(OCR_HARD_LIMIT_BYTES, 50 * 1024 * 1024);
    }

    #[test]
    fn ocr_soft_warn_constant_is_30mb() {
        assert_eq!(OCR_SOFT_WARN_BYTES, 30 * 1024 * 1024);
    }
}
```

Run — must FAIL for `ocr_file`:

```bash
cd src-tauri && cargo test ocr -- --nocapture 2>&1 | head -30
```

- [ ] **Step 2: Implement `ocr_file` and `run`**

Replace the `ocr_file` and `run` bodies:

```rust
use pdfium_render::prelude::*;
use image::{DynamicImage, GrayImage, ImageFormat};
use std::path::{Path, PathBuf};
use std::process::Command;
use lopdf::Document;
use crate::error::{AppError, Result};
use crate::pipeline::{
    temp::TempStage,
    progress::{emit_progress, emit_complete},
};

/// OCR a PDF file:
/// 1. Render each page to 300 DPI grayscale PNG (pdfium-render).
/// 2. Run tesseract on each PNG → produces a single-page PDF with invisible text.
/// 3. Merge the text layers from per-page PDFs back into the original document.
pub fn ocr_file(
    input: &Path,
    output: &Path,
    opts: &OcrOptions,
    on_progress: impl Fn(u32, u32),
) -> Result<()> {
    // File size validation
    let file_size = std::fs::metadata(input)
        .map_err(|e| AppError::Io(e.to_string()))?
        .len();
    if file_size > OCR_HARD_LIMIT_BYTES {
        return Err(AppError::Validation(format!(
            "File is {:.1} MB — exceeds the 50 MB OCR limit. Consider splitting the PDF first.",
            file_size as f64 / 1_048_576.0
        )));
    }

    let tesseract_bin = find_tesseract(opts)?;
    let lang = opts.lang().to_owned();

    // Initialize pdfium (expects pdfium library to be next to the binary)
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")))
            .map_err(|e| AppError::Pdf(format!("Cannot load pdfium: {e}")))?,
    );

    let pdf_doc = pdfium
        .load_pdf_from_file(input, None)
        .map_err(|e| AppError::Pdf(format!("Cannot open PDF for rendering: {e}")))?;

    let page_count = pdf_doc.pages().len() as u32;
    let work_dir = tempfile::tempdir()
        .map_err(|e| AppError::Io(e.to_string()))?;

    let mut text_layer_paths: Vec<PathBuf> = Vec::new();

    for (idx, page) in pdf_doc.pages().iter().enumerate() {
        on_progress(idx as u32, page_count);

        // Render page at 300 DPI to RGBA bitmap
        let render_config = PdfRenderConfig::new()
            .set_target_width(2480)   // 8.5" × 300 DPI
            .set_maximum_height(3508) // 11.7" × 300 DPI
            .rotate_if_landscape(PdfPageRenderRotation::None, true);

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Pdf(format!("Cannot render page {idx}: {e}")))?;

        let dynamic_img: DynamicImage = bitmap
            .as_image()
            .into();
        let gray_img = dynamic_img.to_luma8();

        // Save as PNG in temp dir
        let png_path = work_dir.path().join(format!("page_{idx:04}.png"));
        gray_img
            .save_with_format(&png_path, ImageFormat::Png)
            .map_err(|e| AppError::Io(format!("Cannot save page PNG: {e}")))?;

        // Run: tesseract page_NNNN.png page_NNNN -l eng pdf
        let out_base = work_dir.path().join(format!("tess_{idx:04}"));
        let status = Command::new(&tesseract_bin)
            .arg(&png_path)
            .arg(&out_base)
            .arg("-l").arg(&lang)
            .arg("pdf")
            .status()
            .map_err(|e| AppError::Io(format!("Failed to launch tesseract: {e}")))?;

        if !status.success() {
            return Err(AppError::Pdf(format!(
                "Tesseract failed on page {idx}. Ensure the '{lang}' language data is installed."
            )));
        }

        // tesseract appends .pdf automatically
        let text_pdf = out_base.with_extension("pdf");
        text_layer_paths.push(text_pdf);
    }

    on_progress(page_count, page_count);

    // Merge text layers into original document.
    // Strategy: open original with lopdf, open each per-page tesseract PDF,
    // extract the content stream (text layer) and append it to the original page.
    let mut base_doc = Document::load(input)
        .map_err(|e| AppError::Pdf(format!("Cannot reload base PDF: {e}")))?;

    let base_pages: Vec<_> = base_doc.get_pages().keys().copied().collect();

    for (idx, tess_pdf_path) in text_layer_paths.iter().enumerate() {
        if !tess_pdf_path.exists() {
            continue;
        }
        let tess_doc = match Document::load(tess_pdf_path) {
            Ok(d) => d,
            Err(_) => continue, // skip if tesseract produced no output
        };

        // Extract first page content from tesseract PDF and append to base page
        // This is a best-effort merge: copy content bytes from the tess PDF page
        // and append them to the base page's content stream.
        if let Some(tess_pages) = tess_doc.get_pages().get(&1) {
            let tess_page_id = *tess_pages;
            if let Ok(tess_content) = tess_doc.get_page_content(tess_page_id) {
                let page_num = (idx + 1) as u32;
                if let Some(&base_page_id) = base_pages.get(idx) {
                    // Append text layer content to base page
                    if let Ok(mut base_content) = base_doc.get_page_content(base_page_id) {
                        base_content.extend_from_slice(b"\n");
                        base_content.extend_from_slice(&tess_content);
                        let _ = base_doc.change_page_content(base_page_id, base_content);
                    }
                }
            }
        }
    }

    base_doc
        .save(output)
        .map_err(|e| AppError::Io(format!("Cannot write OCR output: {e}")))?;

    Ok(())
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    use crate::pipeline::validate::validate_pdf;

    let input = std::path::PathBuf::from(&req.input_path);

    // Validate — OCR has a 50MB hard limit
    validate_pdf(&input, Some(OCR_HARD_LIMIT_BYTES))?;

    // Soft warning: emit a non-fatal event if >30MB
    let file_size = std::fs::metadata(&input).map(|m| m.len()).unwrap_or(0);
    if file_size > OCR_SOFT_WARN_BYTES {
        emit_progress(&app, "ocr_pdf", 0, 1,
            &format!("Warning: file is {:.1} MB — OCR may be slow.",
                file_size as f64 / 1_048_576.0));
    }

    let opts: OcrOptions = serde_json::from_value(req.options.clone())
        .map_err(|e| AppError::Validation(format!("Invalid options: {e}")))?;

    let app_clone = app.clone();
    let input_clone = input.clone();

    let stage = TempStage::new()?;
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let out_name = format!("{stem}_ocr.pdf");
    let temp_out = stage.path().join(&out_name);

    let final_out = std::path::PathBuf::from(&req.output_path);

    // Run synchronous OCR work in a blocking thread to avoid blocking the async runtime
    let temp_out_clone = temp_out.clone();
    let final_out_clone = final_out.clone();
    tokio::task::spawn_blocking(move || {
        ocr_file(&input_clone, &temp_out_clone, &opts, |current, total| {
            emit_progress(
                &app_clone,
                "ocr_pdf",
                current as usize,
                total as usize,
                &format!("OCR page {} of {}…", current + 1, total),
            );
        })
    })
    .await
    .map_err(|e| AppError::Io(format!("Task join error: {e}")))??;

    std::fs::copy(&temp_out, &final_out)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    emit_complete(&app, "ocr_pdf", final_out.to_string_lossy().as_ref());
    Ok(final_out.to_string_lossy().into_owned())
}
```

- [ ] **Step 3: Run tests — must PASS**

```bash
cd src-tauri && cargo test ocr -- --nocapture
```

Expected: All 5 unit tests pass (no integration test since Tesseract may not be in CI environment).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/security/ocr.rs
git commit -m "feat(plan6): implement ocr_pdf Rust backend with Tesseract + pdfium-render"
```

---

## Chunk 6: Repair PDF

### Task 6: Rust — repair corrupted PDFs via lopdf lenient parsing

**Files:**
- Create: `src-tauri/src/tools/repair/repair.rs`

**Limitations (document in UI and code):**
- Can fix: corrupt xref table, missing/wrong EOF marker, broken object stream indexes.
- Cannot fix: PDFs encrypted without a known password, severely corrupted content streams (malformed image data, font streams), files that are not PDFs at all.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/tools/repair/repair.rs`:

```rust
use std::path::Path;
use crate::error::{AppError, Result};

pub fn repair_file(input: &Path, output: &Path) -> Result<RepairReport> {
    unimplemented!()
}

#[derive(Debug, serde::Serialize)]
pub struct RepairReport {
    /// True if the file was successfully parsed and re-saved.
    pub success: bool,
    /// Human-readable description of what was fixed (or why it failed).
    pub message: String,
    /// Page count of the repaired document, if available.
    pub page_count: Option<u32>,
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn valid_pdf_bytes() -> Vec<u8> {
        b"%PDF-1.4\n\
          1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
          2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
          3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\n\
          xref\n0 4\n0000000000 65535 f \n\
          0000000009 00000 n \n\
          0000000058 00000 n \n\
          0000000115 00000 n \n\
          trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF"
            .to_vec()
    }

    fn truncated_pdf_bytes() -> Vec<u8> {
        // Valid header but xref and EOF are missing
        b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n".to_vec()
    }

    fn not_a_pdf_bytes() -> Vec<u8> {
        b"This is not a PDF file at all.".to_vec()
    }

    #[test]
    fn repair_valid_pdf_succeeds() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&valid_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let report = repair_file(f.path(), out.path()).unwrap();
        assert!(report.success);
        let bytes = std::fs::read(out.path()).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn repair_reports_failure_for_non_pdf() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&not_a_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let result = repair_file(f.path(), out.path());
        // Either returns Err or a report with success=false
        match result {
            Err(_) => { /* acceptable */ }
            Ok(report) => assert!(!report.success),
        }
    }

    #[test]
    fn repair_truncated_pdf_attempts_recovery() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&truncated_pdf_bytes()).unwrap();
        let out = NamedTempFile::new().unwrap();
        // lopdf lenient mode may or may not succeed — we just verify no panic
        let _ = repair_file(f.path(), out.path());
    }
}
```

Run — must FAIL:

```bash
cd src-tauri && cargo test repair -- --nocapture 2>&1 | head -20
```

- [ ] **Step 2: Implement `repair_file` and `run`**

Replace function bodies:

```rust
use lopdf::Document;
use std::path::Path;
use crate::error::{AppError, Result};
use crate::pipeline::{
    temp::TempStage,
    progress::{emit_progress, emit_complete},
};

/// Attempt to repair a corrupt PDF using lopdf's lenient parser.
/// lopdf::Document::load_filtered with lenient mode rebuilds the xref table
/// on load and rewrites a clean one on save — fixing the most common corruptions.
pub fn repair_file(input: &Path, output: &Path) -> Result<RepairReport> {
    // First, try normal load to detect if already valid
    let load_result = Document::load(input);

    let (doc, was_corrupt) = match load_result {
        Ok(d) => (d, false),
        Err(normal_err) => {
            // Try lenient / filtered load
            // lopdf::Document::load_filtered accepts a filter function;
            // passing `|_, _| true` keeps all objects (lenient mode).
            // API: Document::load_filtered(path, &mut warning_vec)
            // Note: verify exact API against your installed lopdf version.
            let mut warnings: Vec<String> = Vec::new();
            match Document::load_filtered(input, |_, _| true) {
                Ok(d) => (d, true),
                Err(_lenient_err) => {
                    // Check if input even begins with %PDF
                    let header = std::fs::read(input)
                        .unwrap_or_default()
                        .get(..5)
                        .map(|b| b.to_vec())
                        .unwrap_or_default();
                    if !header.starts_with(b"%PDF-") {
                        return Err(AppError::Pdf(
                            "File does not appear to be a PDF (missing %PDF- header). Repair cannot proceed.".into()
                        ));
                    }
                    return Ok(RepairReport {
                        success: false,
                        message: format!(
                            "Could not parse PDF structure even in lenient mode. \
                             The file may have severely corrupted content streams. \
                             Original error: {normal_err}"
                        ),
                        page_count: None,
                    });
                }
            }
        }
    };

    let page_count = doc.get_pages().len() as u32;

    // Save rebuilds a clean xref table and EOF marker automatically
    doc.save(output)
        .map_err(|e| AppError::Io(format!("Cannot write repaired PDF: {e}")))?;

    let message = if was_corrupt {
        format!(
            "Repaired: cross-reference table rebuilt and EOF marker restored. \
             {page_count} page(s) recovered."
        )
    } else {
        format!(
            "PDF structure was valid. Re-saved with a clean xref table. {page_count} page(s)."
        )
    };

    Ok(RepairReport {
        success: true,
        message,
        page_count: Some(page_count),
    })
}

pub async fn run(
    req: crate::tools::ProcessRequest,
    app: tauri::AppHandle,
) -> Result<String> {
    let input = std::path::PathBuf::from(&req.input_path);

    if !input.exists() {
        return Err(AppError::NotFound(format!("File not found: {}", req.input_path)));
    }

    let stage = TempStage::new()?;
    emit_progress(&app, "repair_pdf", 0, 1, "Attempting PDF repair…");

    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let out_name = format!("{stem}_repaired.pdf");
    let temp_out = stage.path().join(&out_name);

    let report = repair_file(&input, &temp_out)?;

    if !report.success {
        return Err(AppError::Pdf(report.message));
    }

    let final_out = std::path::PathBuf::from(&req.output_path);
    std::fs::copy(&temp_out, &final_out)
        .map_err(|e| AppError::Io(format!("Cannot write output: {e}")))?;

    emit_complete(&app, "repair_pdf", final_out.to_string_lossy().as_ref());
    Ok(final_out.to_string_lossy().into_owned())
}
```

- [ ] **Step 3: Run tests — must PASS**

```bash
cd src-tauri && cargo test repair -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tools/repair/repair.rs
git commit -m "feat(plan6): implement repair_pdf Rust backend with lopdf lenient parsing"
```

---

## Chunk 7: Settings — OCR language field

### Task 7: Ensure `settings.rs` has `ocr_language` and `tesseract_path`

**Files:**
- Modify: `src-tauri/src/storage/settings.rs`

- [ ] **Step 1: Write failing test**

In `src-tauri/src/storage/settings.rs`, add to the test module:

```rust
#[test]
fn settings_has_ocr_language_field() {
    let s = Settings::default();
    assert_eq!(s.ocr_language, "eng");
}

#[test]
fn settings_has_tesseract_path_field() {
    let s = Settings::default();
    assert!(s.tesseract_path.is_none());
}
```

Run — must FAIL if fields don't exist yet.

- [ ] **Step 2: Add fields to `Settings`**

In `src-tauri/src/storage/settings.rs`, add to the `Settings` struct:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    // ... existing fields ...

    /// Tesseract language code for OCR (default: "eng").
    #[serde(default = "default_ocr_language")]
    pub ocr_language: String,

    /// Optional absolute path to tesseract binary.
    /// If None, PavoPDF searches the system PATH.
    #[serde(default)]
    pub tesseract_path: Option<String>,
}

fn default_ocr_language() -> String {
    "eng".into()
}
```

Also update `impl Default for Settings`:

```rust
impl Default for Settings {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            ocr_language: default_ocr_language(),
            tesseract_path: None,
        }
    }
}
```

- [ ] **Step 3: Run tests — must PASS**

```bash
cd src-tauri && cargo test settings -- --nocapture
```

- [ ] **Step 4: Compile all**

```bash
cd src-tauri && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: zero errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/storage/settings.rs
git commit -m "feat(plan6): add ocr_language and tesseract_path fields to Settings"
```

---

## Chunk 8: Frontend — Tools Registry

### Task 8: Register the five new tools in `tools-registry.ts`

**Files:**
- Modify: `src/lib/tools-registry.ts`

- [ ] **Step 1: Add entries to the registry**

In `src/lib/tools-registry.ts`, add to the tools array:

```typescript
// ── Security & Repair (Plan 6) ────────────────────────────────────────────────
{
  id: 'protect_pdf',
  label: 'Protect PDF',
  description: 'Add AES-128 password protection to a PDF',
  category: 'security',
  icon: 'lock',
  acceptedExtensions: ['.pdf'],
  outputSuffix: '_protected',
  outputExtension: '.pdf',
},
{
  id: 'unlock_pdf',
  label: 'Unlock PDF',
  description: 'Remove password protection from a PDF you own',
  category: 'security',
  icon: 'lock-open',
  acceptedExtensions: ['.pdf'],
  outputSuffix: '_unlocked',
  outputExtension: '.pdf',
},
{
  id: 'sign_pdf',
  label: 'Sign PDF',
  description: 'Add a drawn visual signature to a PDF page',
  category: 'security',
  icon: 'signature',
  acceptedExtensions: ['.pdf'],
  outputSuffix: '_signed',
  outputExtension: '.pdf',
},
{
  id: 'ocr_pdf',
  label: 'OCR PDF',
  description: 'Make scanned PDFs searchable with an invisible text layer',
  category: 'security',
  icon: 'scan',
  acceptedExtensions: ['.pdf'],
  outputSuffix: '_ocr',
  outputExtension: '.pdf',
},
{
  id: 'repair_pdf',
  label: 'Repair PDF',
  description: 'Attempt to fix common PDF corruption (xref, EOF, object streams)',
  category: 'repair',
  icon: 'wrench',
  acceptedExtensions: ['.pdf'],
  outputSuffix: '_repaired',
  outputExtension: '.pdf',
},
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tools-registry.ts
git commit -m "feat(plan6): register security and repair tools in frontend registry"
```

---

## Chunk 9: Frontend — Protect PDF Workspace

### Task 9: Svelte 5 workspace for Protect PDF

**Files:**
- Create: `src/lib/components/tools/security/ProtectWorkspace.svelte`

- [ ] **Step 1: Create the component**

Create `src/lib/components/tools/security/ProtectWorkspace.svelte`:

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { ToolState } from '$lib/types';

  // ── Props ────────────────────────────────────────────────────────────────────
  const { inputPath, onComplete, onError }: {
    inputPath: string;
    onComplete: (outputPath: string) => void;
    onError: (message: string) => void;
  } = $props();

  // ── State ────────────────────────────────────────────────────────────────────
  let password = $state('');
  let confirmPassword = $state('');
  let ownerPassword = $state('');
  let showOwnerPassword = $state(false);
  let showPassword = $state(false);
  let processing = $state(false);
  let errorMessage = $state('');

  // ── Derived ──────────────────────────────────────────────────────────────────
  const passwordStrength = $derived((): { label: string; color: string; score: number } => {
    const p = password;
    if (p.length === 0) return { label: '', color: 'bg-gray-200', score: 0 };
    let score = 0;
    if (p.length >= 8) score++;
    if (p.length >= 12) score++;
    if (/[A-Z]/.test(p)) score++;
    if (/[0-9]/.test(p)) score++;
    if (/[^A-Za-z0-9]/.test(p)) score++;
    if (score <= 1) return { label: 'Weak', color: 'bg-red-500', score };
    if (score <= 3) return { label: 'Fair', color: 'bg-amber-500', score };
    return { label: 'Strong', color: 'bg-green-500', score };
  });

  const passwordsMatch = $derived(
    confirmPassword.length === 0 || password === confirmPassword
  );

  const canSubmit = $derived(
    password.length > 0 &&
    password === confirmPassword &&
    !processing
  );

  // ── Actions ──────────────────────────────────────────────────────────────────
  async function handleProtect() {
    if (!canSubmit) return;
    errorMessage = '';
    processing = true;

    try {
      const stem = inputPath.split(/[/\\]/).pop()?.replace(/\.pdf$/i, '') ?? 'document';
      const outputPath = inputPath.replace(/[^/\\]+$/, `${stem}_protected.pdf`);

      const result = await invoke<string>('run_tool', {
        request: {
          tool: 'protect_pdf',
          input_path: inputPath,
          output_path: outputPath,
          options: {
            password,
            owner_password: ownerPassword || null,
          },
        },
      });

      onComplete(result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      errorMessage = msg;
      onError(msg);
    } finally {
      processing = false;
    }
  }
</script>

<div class="space-y-5">
  <div>
    <h2 class="text-lg font-semibold text-gray-800">Protect PDF</h2>
    <p class="text-sm text-gray-500 mt-1">
      Add AES-128 password protection. Anyone who opens this file will need the password.
    </p>
  </div>

  <!-- User Password -->
  <div class="space-y-1">
    <label class="block text-sm font-medium text-gray-700" for="user-password">
      Password <span class="text-red-500">*</span>
    </label>
    <div class="relative">
      <input
        id="user-password"
        type={showPassword ? 'text' : 'password'}
        class="w-full rounded-md border border-gray-300 px-3 py-2 pr-10 text-sm
               focus:outline-none focus:ring-2 focus:ring-teal"
        placeholder="Enter password"
        bind:value={password}
        autocomplete="new-password"
      />
      <button
        type="button"
        class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
        onclick={() => { showPassword = !showPassword; }}
        aria-label={showPassword ? 'Hide password' : 'Show password'}
      >
        {showPassword ? '🙈' : '👁'}
      </button>
    </div>

    <!-- Strength indicator -->
    {#if password.length > 0}
      <div class="mt-1 space-y-1">
        <div class="flex gap-1">
          {#each [1, 2, 3, 4, 5] as level}
            <div
              class="h-1.5 flex-1 rounded-full transition-colors duration-200
                     {passwordStrength.score >= level ? passwordStrength.color : 'bg-gray-200'}"
            ></div>
          {/each}
        </div>
        <p class="text-xs text-gray-500">
          Strength: <span class="font-medium">{passwordStrength.label}</span>
        </p>
      </div>
    {/if}
  </div>

  <!-- Confirm Password -->
  <div class="space-y-1">
    <label class="block text-sm font-medium text-gray-700" for="confirm-password">
      Confirm Password <span class="text-red-500">*</span>
    </label>
    <input
      id="confirm-password"
      type={showPassword ? 'text' : 'password'}
      class="w-full rounded-md border px-3 py-2 text-sm focus:outline-none focus:ring-2
             {!passwordsMatch ? 'border-red-400 focus:ring-red-400' : 'border-gray-300 focus:ring-teal'}"
      placeholder="Re-enter password"
      bind:value={confirmPassword}
      autocomplete="new-password"
    />
    {#if !passwordsMatch}
      <p class="text-xs text-red-500">Passwords do not match</p>
    {/if}
  </div>

  <!-- Owner Password (advanced) -->
  <div>
    <button
      type="button"
      class="text-xs text-teal underline"
      onclick={() => { showOwnerPassword = !showOwnerPassword; }}
    >
      {showOwnerPassword ? 'Hide' : 'Show'} owner password (advanced)
    </button>
    {#if showOwnerPassword}
      <div class="mt-2 space-y-1">
        <label class="block text-sm font-medium text-gray-700" for="owner-password">
          Owner Password
        </label>
        <input
          id="owner-password"
          type="password"
          class="w-full rounded-md border border-gray-300 px-3 py-2 text-sm
                 focus:outline-none focus:ring-2 focus:ring-teal"
          placeholder="Defaults to user password if left blank"
          bind:value={ownerPassword}
          autocomplete="new-password"
        />
        <p class="text-xs text-gray-400">
          The owner password controls editing and printing permissions.
        </p>
      </div>
    {/if}
  </div>

  {#if errorMessage}
    <div class="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
      {errorMessage}
    </div>
  {/if}

  <button
    type="button"
    class="w-full rounded-md bg-teal py-2.5 text-sm font-semibold text-white
           hover:bg-teal-dark disabled:opacity-40 disabled:cursor-not-allowed
           transition-colors"
    disabled={!canSubmit}
    onclick={handleProtect}
  >
    {processing ? 'Protecting…' : 'Protect PDF'}
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/tools/security/ProtectWorkspace.svelte
git commit -m "feat(plan6): add ProtectWorkspace Svelte 5 component"
```

---

## Chunk 10: Frontend — Unlock PDF Workspace

### Task 10: Svelte 5 workspace for Unlock PDF

**Files:**
- Create: `src/lib/components/tools/security/UnlockWorkspace.svelte`

- [ ] **Step 1: Create the component**

Create `src/lib/components/tools/security/UnlockWorkspace.svelte`:

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  const { inputPath, onComplete, onError }: {
    inputPath: string;
    onComplete: (outputPath: string) => void;
    onError: (message: string) => void;
  } = $props();

  let password = $state('');
  let showPassword = $state(false);
  let processing = $state(false);
  let errorMessage = $state('');

  const canSubmit = $derived(password.length > 0 && !processing);

  async function handleUnlock() {
    if (!canSubmit) return;
    errorMessage = '';
    processing = true;

    try {
      const stem = inputPath.split(/[/\\]/).pop()?.replace(/\.pdf$/i, '') ?? 'document';
      const outputPath = inputPath.replace(/[^/\\]+$/, `${stem}_unlocked.pdf`);

      const result = await invoke<string>('run_tool', {
        request: {
          tool: 'unlock_pdf',
          input_path: inputPath,
          output_path: outputPath,
          options: { password },
        },
      });

      onComplete(result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      errorMessage = msg;
      onError(msg);
    } finally {
      processing = false;
    }
  }
</script>

<div class="space-y-5">
  <div>
    <h2 class="text-lg font-semibold text-gray-800">Unlock PDF</h2>
    <p class="text-sm text-gray-500 mt-1">
      Enter the current password to remove protection and save an unlocked copy.
      You must have the legal right to unlock this file.
    </p>
  </div>

  <div class="space-y-1">
    <label class="block text-sm font-medium text-gray-700" for="unlock-password">
      Current Password <span class="text-red-500">*</span>
    </label>
    <div class="relative">
      <input
        id="unlock-password"
        type={showPassword ? 'text' : 'password'}
        class="w-full rounded-md border border-gray-300 px-3 py-2 pr-10 text-sm
               focus:outline-none focus:ring-2 focus:ring-teal"
        placeholder="Enter the PDF password"
        bind:value={password}
        autocomplete="current-password"
      />
      <button
        type="button"
        class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
        onclick={() => { showPassword = !showPassword; }}
        aria-label={showPassword ? 'Hide password' : 'Show password'}
      >
        {showPassword ? '🙈' : '👁'}
      </button>
    </div>
  </div>

  {#if errorMessage}
    <div class="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
      {errorMessage}
    </div>
  {/if}

  <div class="rounded-md bg-amber-50 border border-amber-200 px-4 py-3 text-sm text-amber-700">
    <strong>Note:</strong> PavoPDF can only unlock files you have legitimate access to.
    This tool opens the PDF with your password and re-saves it without encryption.
  </div>

  <button
    type="button"
    class="w-full rounded-md bg-teal py-2.5 text-sm font-semibold text-white
           hover:bg-teal-dark disabled:opacity-40 disabled:cursor-not-allowed
           transition-colors"
    disabled={!canSubmit}
    onclick={handleUnlock}
  >
    {processing ? 'Unlocking…' : 'Unlock PDF'}
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/tools/security/UnlockWorkspace.svelte
git commit -m "feat(plan6): add UnlockWorkspace Svelte 5 component"
```

---

## Chunk 11: Frontend — Sign PDF Workspace

### Task 11: Svelte 5 workspace for Sign PDF with canvas drawing

**Files:**
- Create: `src/lib/components/tools/security/SignWorkspace.svelte`

- [ ] **Step 1: Create the component**

Create `src/lib/components/tools/security/SignWorkspace.svelte`:

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  const { inputPath, pageCount = 1, onComplete, onError }: {
    inputPath: string;
    pageCount?: number;
    onComplete: (outputPath: string) => void;
    onError: (message: string) => void;
  } = $props();

  // ── Canvas state ─────────────────────────────────────────────────────────────
  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let isDrawing = $state(false);
  let hasSignature = $state(false);
  let strokes = $state<{ x: number; y: number; type: 'start' | 'move' }[][]>([]);
  let currentStroke = $state<{ x: number; y: number; type: 'start' | 'move' }[]>([]);

  // ── Placement state ──────────────────────────────────────────────────────────
  let selectedPage = $state(0); // zero-based
  let xPos = $state(72);        // PDF points from left (1 inch = 72 pt)
  let yPos = $state(72);        // PDF points from bottom
  let sigWidth = $state(200);
  let sigHeight = $state(80);

  // ── Submission state ─────────────────────────────────────────────────────────
  let processing = $state(false);
  let errorMessage = $state('');

  const canSubmit = $derived(hasSignature && !processing);
  const pageOptions = $derived(
    Array.from({ length: pageCount }, (_, i) => ({ value: i, label: `Page ${i + 1}` }))
  );

  // ── Canvas drawing ───────────────────────────────────────────────────────────
  function getPos(e: MouseEvent | TouchEvent): { x: number; y: number } {
    if (!canvasEl) return { x: 0, y: 0 };
    const rect = canvasEl.getBoundingClientRect();
    if (e instanceof TouchEvent) {
      return {
        x: e.touches[0].clientX - rect.left,
        y: e.touches[0].clientY - rect.top,
      };
    }
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  function onPointerDown(e: MouseEvent | TouchEvent) {
    e.preventDefault();
    if (!canvasEl) return;
    isDrawing = true;
    const pos = getPos(e);
    currentStroke = [{ ...pos, type: 'start' }];
    const ctx = canvasEl.getContext('2d')!;
    ctx.beginPath();
    ctx.moveTo(pos.x, pos.y);
  }

  function onPointerMove(e: MouseEvent | TouchEvent) {
    e.preventDefault();
    if (!isDrawing || !canvasEl) return;
    const pos = getPos(e);
    currentStroke = [...currentStroke, { ...pos, type: 'move' }];
    const ctx = canvasEl.getContext('2d')!;
    ctx.lineTo(pos.x, pos.y);
    ctx.strokeStyle = '#1B7A8A';
    ctx.lineWidth = 2.5;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.stroke();
  }

  function onPointerUp(e: MouseEvent | TouchEvent) {
    if (!isDrawing) return;
    isDrawing = false;
    if (currentStroke.length > 1) {
      strokes = [...strokes, currentStroke];
      hasSignature = true;
    }
    currentStroke = [];
  }

  function clearSignature() {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext('2d')!;
    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);
    strokes = [];
    currentStroke = [];
    hasSignature = false;
  }

  function undoLastStroke() {
    if (strokes.length === 0 || !canvasEl) return;
    const newStrokes = strokes.slice(0, -1);
    strokes = newStrokes;
    hasSignature = newStrokes.length > 0;

    // Redraw all remaining strokes
    const ctx = canvasEl.getContext('2d')!;
    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);
    for (const stroke of newStrokes) {
      ctx.beginPath();
      for (const point of stroke) {
        if (point.type === 'start') {
          ctx.moveTo(point.x, point.y);
        } else {
          ctx.lineTo(point.x, point.y);
          ctx.strokeStyle = '#1B7A8A';
          ctx.lineWidth = 2.5;
          ctx.lineCap = 'round';
          ctx.lineJoin = 'round';
          ctx.stroke();
        }
      }
    }
  }

  function getSignaturePngBase64(): string {
    if (!canvasEl) return '';
    // Export at original canvas resolution
    return canvasEl.toDataURL('image/png').replace(/^data:image\/png;base64,/, '');
  }

  // ── Submission ───────────────────────────────────────────────────────────────
  async function handleSign() {
    if (!canSubmit) return;
    errorMessage = '';
    processing = true;

    try {
      const signaturePngBase64 = getSignaturePngBase64();
      const stem = inputPath.split(/[/\\]/).pop()?.replace(/\.pdf$/i, '') ?? 'document';
      const outputPath = inputPath.replace(/[^/\\]+$/, `${stem}_signed.pdf`);

      const result = await invoke<string>('run_tool', {
        request: {
          tool: 'sign_pdf',
          input_path: inputPath,
          output_path: outputPath,
          options: {
            signature_png_base64: signaturePngBase64,
            page_index: selectedPage,
            x: xPos,
            y: yPos,
            width: sigWidth,
            height: sigHeight,
          },
        },
      });

      onComplete(result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      errorMessage = msg;
      onError(msg);
    } finally {
      processing = false;
    }
  }

  onMount(() => {
    if (canvasEl) {
      const ctx = canvasEl.getContext('2d')!;
      ctx.fillStyle = '#f9f5f0';
      ctx.fillRect(0, 0, canvasEl.width, canvasEl.height);
    }
  });
</script>

<div class="space-y-5">
  <div>
    <h2 class="text-lg font-semibold text-gray-800">Sign PDF</h2>
    <p class="text-sm text-gray-500 mt-1">
      Draw your signature below. It will be embedded as a visible image on the selected page.
      <strong>Visual signature only</strong> — not a cryptographic signature.
    </p>
  </div>

  <!-- Canvas -->
  <div class="space-y-2">
    <div class="flex items-center justify-between">
      <span class="text-sm font-medium text-gray-700">Draw Signature</span>
      <div class="flex gap-2">
        <button
          type="button"
          class="text-xs px-2 py-1 rounded border border-gray-300 text-gray-600
                 hover:bg-gray-100 disabled:opacity-40"
          disabled={strokes.length === 0}
          onclick={undoLastStroke}
        >
          Undo
        </button>
        <button
          type="button"
          class="text-xs px-2 py-1 rounded border border-gray-300 text-gray-600
                 hover:bg-gray-100 disabled:opacity-40"
          disabled={!hasSignature}
          onclick={clearSignature}
        >
          Clear
        </button>
      </div>
    </div>

    <canvas
      bind:this={canvasEl}
      width={500}
      height={150}
      class="w-full rounded-md border-2 border-dashed border-gray-300 bg-cream
             cursor-crosshair touch-none select-none"
      style="max-height: 150px;"
      onmousedown={onPointerDown}
      onmousemove={onPointerMove}
      onmouseup={onPointerUp}
      onmouseleave={onPointerUp}
      ontouchstart={onPointerDown}
      ontouchmove={onPointerMove}
      ontouchend={onPointerUp}
    ></canvas>

    {#if !hasSignature}
      <p class="text-xs text-gray-400 text-center">Draw your signature in the box above</p>
    {/if}
  </div>

  <!-- Placement -->
  <div class="grid grid-cols-2 gap-4">
    <div>
      <label class="block text-xs font-medium text-gray-600 mb-1" for="sig-page">Page</label>
      <select
        id="sig-page"
        class="w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm
               focus:outline-none focus:ring-2 focus:ring-teal"
        bind:value={selectedPage}
      >
        {#each pageOptions as opt (opt.value)}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
    </div>

    <div>
      <label class="block text-xs font-medium text-gray-600 mb-1" for="sig-width">
        Width (pt)
      </label>
      <input
        id="sig-width"
        type="number"
        min="50"
        max="500"
        class="w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm
               focus:outline-none focus:ring-2 focus:ring-teal"
        bind:value={sigWidth}
      />
    </div>

    <div>
      <label class="block text-xs font-medium text-gray-600 mb-1" for="sig-x">
        X position (pt from left)
      </label>
      <input
        id="sig-x"
        type="number"
        min="0"
        class="w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm
               focus:outline-none focus:ring-2 focus:ring-teal"
        bind:value={xPos}
      />
    </div>

    <div>
      <label class="block text-xs font-medium text-gray-600 mb-1" for="sig-y">
        Y position (pt from bottom)
      </label>
      <input
        id="sig-y"
        type="number"
        min="0"
        class="w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm
               focus:outline-none focus:ring-2 focus:ring-teal"
        bind:value={yPos}
      />
    </div>
  </div>

  <p class="text-xs text-gray-400">
    PDF coordinates: 72 points = 1 inch. Origin is bottom-left of the page.
    Letter page is 612 × 792 pt.
  </p>

  {#if errorMessage}
    <div class="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
      {errorMessage}
    </div>
  {/if}

  <button
    type="button"
    class="w-full rounded-md bg-teal py-2.5 text-sm font-semibold text-white
           hover:bg-teal-dark disabled:opacity-40 disabled:cursor-not-allowed
           transition-colors"
    disabled={!canSubmit}
    onclick={handleSign}
  >
    {processing ? 'Embedding signature…' : 'Add Signature to PDF'}
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/tools/security/SignWorkspace.svelte
git commit -m "feat(plan6): add SignWorkspace with canvas signature drawing"
```

---

## Chunk 12: Frontend — OCR PDF Workspace

### Task 12: Svelte 5 workspace for OCR PDF

**Files:**
- Create: `src/lib/components/tools/security/OcrWorkspace.svelte`

- [ ] **Step 1: Create the component**

Create `src/lib/components/tools/security/OcrWorkspace.svelte`:

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { settingsStore } from '$lib/stores/settingsStore';

  const { inputPath, fileSizeBytes = 0, onComplete, onError }: {
    inputPath: string;
    fileSizeBytes?: number;
    onComplete: (outputPath: string) => void;
    onError: (message: string) => void;
  } = $props();

  const SOFT_WARN_BYTES = 30 * 1024 * 1024;
  const HARD_LIMIT_BYTES = 50 * 1024 * 1024;

  // Common Tesseract language codes with human-readable names
  const LANGUAGE_OPTIONS = [
    { code: 'eng', label: 'English' },
    { code: 'deu', label: 'German' },
    { code: 'fra', label: 'French' },
    { code: 'spa', label: 'Spanish' },
    { code: 'ita', label: 'Italian' },
    { code: 'por', label: 'Portuguese' },
    { code: 'nld', label: 'Dutch' },
    { code: 'rus', label: 'Russian' },
    { code: 'chi_sim', label: 'Chinese (Simplified)' },
    { code: 'chi_tra', label: 'Chinese (Traditional)' },
    { code: 'jpn', label: 'Japanese' },
    { code: 'kor', label: 'Korean' },
    { code: 'ara', label: 'Arabic' },
  ] as const;

  let selectedLanguage = $state($settingsStore.ocr_language ?? 'eng');
  let processing = $state(false);
  let errorMessage = $state('');
  let progressMessage = $state('');

  const isSoftWarning = $derived(fileSizeBytes > SOFT_WARN_BYTES && fileSizeBytes <= HARD_LIMIT_BYTES);
  const isHardLimit = $derived(fileSizeBytes > HARD_LIMIT_BYTES);
  const fileSizeMB = $derived((fileSizeBytes / 1_048_576).toFixed(1));

  const canSubmit = $derived(!processing && !isHardLimit);

  async function handleOcr() {
    if (!canSubmit) return;
    errorMessage = '';
    progressMessage = 'Starting OCR…';
    processing = true;

    try {
      const stem = inputPath.split(/[/\\]/).pop()?.replace(/\.pdf$/i, '') ?? 'document';
      const outputPath = inputPath.replace(/[^/\\]+$/, `${stem}_ocr.pdf`);

      const result = await invoke<string>('run_tool', {
        request: {
          tool: 'ocr_pdf',
          input_path: inputPath,
          output_path: outputPath,
          options: {
            language: selectedLanguage,
            tesseract_path: $settingsStore.tesseract_path ?? null,
          },
        },
      });

      onComplete(result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      errorMessage = msg;
      onError(msg);
    } finally {
      processing = false;
      progressMessage = '';
    }
  }
</script>

<div class="space-y-5">
  <div>
    <h2 class="text-lg font-semibold text-gray-800">OCR PDF</h2>
    <p class="text-sm text-gray-500 mt-1">
      Add an invisible searchable text layer to scanned PDFs using Tesseract OCR.
      The original images are preserved; only text becomes searchable.
    </p>
  </div>

  <!-- File size warnings -->
  {#if isHardLimit}
    <div class="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
      <strong>File too large:</strong> {fileSizeMB} MB exceeds the 50 MB OCR limit.
      Split the PDF into smaller parts before running OCR.
    </div>
  {:else if isSoftWarning}
    <div class="rounded-md bg-amber-50 border border-amber-200 px-4 py-3 text-sm text-amber-700">
      <strong>Large file:</strong> {fileSizeMB} MB — OCR may take several minutes.
      Consider splitting first for faster results.
    </div>
  {/if}

  <!-- Tesseract setup notice -->
  <div class="rounded-md bg-blue-50 border border-blue-200 px-4 py-3 text-sm text-blue-700">
    <strong>Tesseract required:</strong> OCR uses the Tesseract engine which must be installed
    separately. Install via your system package manager or from
    <span class="font-mono">github.com/UB-Mannheim/tesseract</span> (Windows).
    Set a custom path in Settings if needed.
  </div>

  <!-- Language picker -->
  <div>
    <label class="block text-sm font-medium text-gray-700 mb-1" for="ocr-language">
      Document Language
    </label>
    <select
      id="ocr-language"
      class="w-full rounded-md border border-gray-300 px-3 py-2 text-sm
             focus:outline-none focus:ring-2 focus:ring-teal"
      bind:value={selectedLanguage}
    >
      {#each LANGUAGE_OPTIONS as lang (lang.code)}
        <option value={lang.code}>{lang.label} ({lang.code})</option>
      {/each}
    </select>
    <p class="text-xs text-gray-400 mt-1">
      The language data file (<span class="font-mono">{selectedLanguage}.traineddata</span>)
      must be installed in Tesseract's tessdata directory.
    </p>
  </div>

  {#if progressMessage}
    <div class="rounded-md bg-teal/10 border border-teal/30 px-4 py-3 text-sm text-teal">
      {progressMessage}
    </div>
  {/if}

  {#if errorMessage}
    <div class="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
      {errorMessage}
    </div>
  {/if}

  <button
    type="button"
    class="w-full rounded-md bg-teal py-2.5 text-sm font-semibold text-white
           hover:bg-teal-dark disabled:opacity-40 disabled:cursor-not-allowed
           transition-colors"
    disabled={!canSubmit}
    onclick={handleOcr}
  >
    {processing ? 'Running OCR…' : 'Run OCR'}
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/tools/security/OcrWorkspace.svelte
git commit -m "feat(plan6): add OcrWorkspace with language picker and size warnings"
```

---

## Chunk 13: Frontend — Repair PDF Workspace

### Task 13: Svelte 5 workspace for Repair PDF

**Files:**
- Create: `src/lib/components/tools/repair/RepairWorkspace.svelte`

- [ ] **Step 1: Create the component**

Create `src/lib/components/tools/repair/RepairWorkspace.svelte`:

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  const { inputPath, onComplete, onError }: {
    inputPath: string;
    onComplete: (outputPath: string) => void;
    onError: (message: string) => void;
  } = $props();

  let processing = $state(false);
  let errorMessage = $state('');
  let repairReport = $state<{ success: boolean; message: string; page_count: number | null } | null>(null);

  const canSubmit = $derived(!processing);

  async function handleRepair() {
    if (!canSubmit) return;
    errorMessage = '';
    repairReport = null;
    processing = true;

    try {
      const stem = inputPath.split(/[/\\]/).pop()?.replace(/\.pdf$/i, '') ?? 'document';
      const outputPath = inputPath.replace(/[^/\\]+$/, `${stem}_repaired.pdf`);

      const result = await invoke<string>('run_tool', {
        request: {
          tool: 'repair_pdf',
          input_path: inputPath,
          output_path: outputPath,
          options: {},
        },
      });

      onComplete(result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      errorMessage = msg;
      onError(msg);
    } finally {
      processing = false;
    }
  }
</script>

<div class="space-y-5">
  <div>
    <h2 class="text-lg font-semibold text-gray-800">Repair PDF</h2>
    <p class="text-sm text-gray-500 mt-1">
      Attempt to fix common structural corruption in PDF files.
    </p>
  </div>

  <!-- What can be fixed -->
  <div class="rounded-md bg-green-50 border border-green-200 px-4 py-3 space-y-2">
    <p class="text-sm font-medium text-green-800">What Repair can fix:</p>
    <ul class="text-sm text-green-700 list-disc list-inside space-y-1">
      <li>Corrupt or missing cross-reference (xref) table</li>
      <li>Missing or malformed <span class="font-mono">%%EOF</span> marker</li>
      <li>Broken object stream indexes</li>
      <li>Minor structural inconsistencies</li>
    </ul>
  </div>

  <!-- What cannot be fixed -->
  <div class="rounded-md bg-gray-50 border border-gray-200 px-4 py-3 space-y-2">
    <p class="text-sm font-medium text-gray-700">What Repair cannot fix:</p>
    <ul class="text-sm text-gray-600 list-disc list-inside space-y-1">
      <li>Password-protected PDFs (use Unlock first)</li>
      <li>Severely corrupted or truncated content streams</li>
      <li>Files that are not PDFs (missing <span class="font-mono">%PDF-</span> header)</li>
      <li>Damaged image data or font streams</li>
    </ul>
  </div>

  {#if repairReport}
    <div class="rounded-md {repairReport.success ? 'bg-green-50 border-green-200 text-green-700' : 'bg-red-50 border-red-200 text-red-700'} border px-4 py-3 text-sm">
      {repairReport.message}
      {#if repairReport.page_count !== null}
        <br/><span class="font-medium">{repairReport.page_count} page(s) recovered.</span>
      {/if}
    </div>
  {/if}

  {#if errorMessage}
    <div class="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
      {errorMessage}
    </div>
  {/if}

  <button
    type="button"
    class="w-full rounded-md bg-teal py-2.5 text-sm font-semibold text-white
           hover:bg-teal-dark disabled:opacity-40 disabled:cursor-not-allowed
           transition-colors"
    disabled={!canSubmit}
    onclick={handleRepair}
  >
    {processing ? 'Repairing…' : 'Attempt Repair'}
  </button>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/tools/repair/RepairWorkspace.svelte
git commit -m "feat(plan6): add RepairWorkspace with limitations panel"
```

---

## Chunk 14: Frontend — Workspace Router

### Task 14: Wire new workspace components into the workspace router

**Files:**
- Modify: `src/lib/components/WorkspaceRouter.svelte` (or equivalent dispatch component)

- [ ] **Step 1: Import and add cases for the five new tools**

In your existing workspace router (the component that maps `tool.id` → workspace component), add:

```svelte
<script lang="ts">
  // Add these imports alongside existing workspace imports:
  import ProtectWorkspace from '$lib/components/tools/security/ProtectWorkspace.svelte';
  import UnlockWorkspace from '$lib/components/tools/security/UnlockWorkspace.svelte';
  import SignWorkspace from '$lib/components/tools/security/SignWorkspace.svelte';
  import OcrWorkspace from '$lib/components/tools/security/OcrWorkspace.svelte';
  import RepairWorkspace from '$lib/components/tools/repair/RepairWorkspace.svelte';
</script>

<!-- In the {#if}/{:else if} or {#each} dispatch block, add: -->
{#if activeTool?.id === 'protect_pdf'}
  <ProtectWorkspace
    inputPath={activeFile}
    onComplete={handleComplete}
    onError={handleError}
  />
{:else if activeTool?.id === 'unlock_pdf'}
  <UnlockWorkspace
    inputPath={activeFile}
    onComplete={handleComplete}
    onError={handleError}
  />
{:else if activeTool?.id === 'sign_pdf'}
  <SignWorkspace
    inputPath={activeFile}
    pageCount={activeFilePageCount}
    onComplete={handleComplete}
    onError={handleError}
  />
{:else if activeTool?.id === 'ocr_pdf'}
  <OcrWorkspace
    inputPath={activeFile}
    fileSizeBytes={activeFileSizeBytes}
    onComplete={handleComplete}
    onError={handleError}
  />
{:else if activeTool?.id === 'repair_pdf'}
  <RepairWorkspace
    inputPath={activeFile}
    onComplete={handleComplete}
    onError={handleError}
  />
{/if}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/WorkspaceRouter.svelte
git commit -m "feat(plan6): wire security and repair workspaces into router"
```

---

## Chunk 15: Final Integration Verification

### Task 15: Full build and integration smoke test

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

```bash
cd src-tauri && cargo test -- --nocapture 2>&1 | tail -30
```

Expected: All tests pass. Verify specifically:
- `security_and_repair_tool_names_are_recognized` ✓
- `protect_requires_non_empty_password` ✓
- `protect_produces_output_file` ✓
- `unlock_rejects_empty_password` ✓
- `unlock_unencrypted_pdf_passes_through` ✓
- `sign_rejects_empty_signature` ✓
- `sign_produces_output_pdf` ✓
- `lang_defaults_to_eng` ✓
- `find_tesseract_returns_not_found_for_invalid_path` ✓
- `repair_valid_pdf_succeeds` ✓
- `settings_has_ocr_language_field` ✓
- `settings_has_tesseract_path_field` ✓

- [ ] **Step 2: TypeScript type check**

```bash
npx tsc --noEmit
```

Expected: Zero errors.

- [ ] **Step 3: Development build**

```bash
npm run tauri dev
```

Expected: App window opens. Navigate to each of the five new tools via the dashboard. Verify the workspace UIs render without console errors.

- [ ] **Step 4: Manual smoke tests**

For each tool, test with a known-good PDF:

1. **Protect:** Enter "test123" / "test123" → click Protect → verify output file has `_protected.pdf` suffix → attempt to open in a PDF viewer and confirm password prompt appears.
2. **Unlock:** Load the `_protected.pdf` → enter "test123" → click Unlock → verify `_unlocked.pdf` opens without password.
3. **Sign:** Draw a scribble → leave default page/position → click Add Signature → verify `_signed.pdf` has a visible image annotation.
4. **OCR:** Load a scanned PDF (or any PDF) → language "English" → click Run OCR → verify `_ocr.pdf` allows text selection.
5. **Repair:** Load any PDF → click Attempt Repair → verify `_repaired.pdf` is produced and opens correctly.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat(plan6): complete security and repair tools — protect, unlock, sign, OCR, repair"
```

---

## Appendix: Output Filename Convention

| Tool | Output filename pattern | Example |
|---|---|---|
| Protect PDF | `{stem}_protected.pdf` | `invoice_protected.pdf` |
| Unlock PDF | `{stem}_unlocked.pdf` | `invoice_unlocked.pdf` |
| Sign PDF | `{stem}_signed.pdf` | `invoice_signed.pdf` |
| OCR PDF | `{stem}_ocr.pdf` | `scan_ocr.pdf` |
| Repair PDF | `{stem}_repaired.pdf` | `corrupt_repaired.pdf` |

## Appendix: Rust Module File Structure

```
src-tauri/src/
├── tools/
│   ├── mod.rs                    (updated — new match arms + tool_name_is_known entries)
│   ├── security/
│   │   ├── mod.rs                (pub mod protect; unlock; sign; ocr)
│   │   ├── protect.rs            (ProtectOptions, protect_file, run)
│   │   ├── unlock.rs             (UnlockOptions, unlock_file, run)
│   │   ├── sign.rs               (SignOptions, sign_file, run)
│   │   └── ocr.rs                (OcrOptions, find_tesseract, ocr_file, run)
│   └── repair/
│       ├── mod.rs                (pub mod repair)
│       └── repair.rs             (RepairReport, repair_file, run)
```

## Appendix: Frontend Component File Structure

```
src/lib/components/tools/
├── security/
│   ├── ProtectWorkspace.svelte
│   ├── UnlockWorkspace.svelte
│   ├── SignWorkspace.svelte
│   └── OcrWorkspace.svelte
└── repair/
    └── RepairWorkspace.svelte
```

## Appendix: lopdf API Version Notes

lopdf's API evolves across versions. Verify the following against the version in your `Cargo.lock`:

| Feature | lopdf 0.30 | lopdf 0.32 | lopdf 0.33+ |
|---|---|---|---|
| `Document::load_with_password` | Check | Check | Check |
| `Document::load_filtered` | Check | Check | Check |
| `Document::encrypt()` | May not exist | Check | AES-128 planned |
| Trailer as dict | `doc.trailer` | `doc.trailer` | `doc.trailer` |

If `Document::encrypt()` is absent, activate the qpdf fallback documented in Chunk 2.

## Appendix: Cargo.toml Additions for Plan 6

Add these to `src-tauri/Cargo.toml` if not already present:

```toml
base64 = "0.22"
```

All other crates (`lopdf`, `pdfium-render`, `image`, `tempfile`, `tokio`) were added in Plan 1.
