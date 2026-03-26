# PavoPDF — Design Specification
**Date:** 2026-03-15
**Status:** Approved

---

## Overview

PavoPDF is a free, open-source, cross-platform desktop PDF tool. It is a privacy-first alternative to ilovepdf.com — all PDF processing happens locally on the user's machine. No user files, document content, or usage data is ever sent to any server. The app works with no internet connection. MIT license.

- **Website:** pavopdf.com
- **Repository:** GitHub (public, MIT license)
- **Distribution:** GitHub Releases + website download buttons
- **"Offline" definition:** User file content never touches a network interface. The optional auto-updater performs a version-number check only (no file data transmitted) and is disabled by default; users enable it in Settings and can disable it at compile time.

---

## Tech Stack

| Layer | Choice | Rationale |
|---|---|---|
| App framework | Tauri 2 | Smallest bundle, native security model, no bundled Chromium |
| Backend language | Rust | Memory safety, performance, excellent PDF crate ecosystem |
| Frontend framework | Svelte 5 | Compiles to vanilla JS, no runtime overhead, silky animations |
| Styling | Tailwind CSS | Utility-first, fast to build custom design systems |
| PDF engine (render/view) | pdfium-render | Google's production PDF engine (Chrome/Edge), Apache 2.0 |
| PDF structural ops | lopdf | Pure Rust, direct PDF object tree access; used for all writes/mutations |
| PDF creation | printpdf | Pure Rust PDF writer; used for Office→PDF output |
| XML parsing (.pptx) | quick-xml | Zero-copy XML parser for .pptx (ZIP+XML) parsing |
| OCR | Tesseract (bundled binary) | Apache 2.0, ~35MB with English lang data (estimate) |
| Office read | docx-rs + calamine | .docx/.xlsx parsing in pure Rust |
| Image ops | image crate | Pure Rust raster image processing |
| Async runtime | Tokio | Non-blocking processing — UI never freezes on heavy ops |

---

## Platform Support

| Platform | Installer format |
|---|---|
| Windows | `.exe` (NSIS installer) |
| macOS | `.dmg` |
| Ubuntu / Debian | `.AppImage` + `.deb` |

All three built via GitHub Actions CI on every tagged release.

**macOS note:** Notarization via Apple Gatekeeper requires a paid Apple Developer Program account ($99/year). CI is configured to notarize when signing secrets are present. Without them, unsigned `.dmg` builds are functional — users open them via right-click → Open to bypass Gatekeeper warning. Notarization is a post-launch step.

**Bundle size estimate (approximate, uncompressed):** pdfium ~60MB · Tesseract + eng.traineddata ~35MB · app ~25MB · total ~120MB. These are estimates; actual sizes vary by platform and should be verified during implementation.

---

## Design System

### Color Palette

| Role | Color | Hex |
|---|---|---|
| Sidebar background | Teal | `#1B7A8A` |
| Primary action / CTA | Peach | `#E8956A` |
| Highlight / amber accent | Amber | `#D4A017` |
| Main background | Warm off-white | `#F9F5F0` |
| Sidebar text | White | `#FFFFFF` |

### UI States — Three-State Progressive Disclosure

**State 1 — Dashboard (home screen)**
- Top bar: app logo + ⌘K / Ctrl+K spotlight search (searches all 25 tool names)
- Category tabs: All / Organise / PDF→Other / Other→PDF / Edit / Security / Repair & OCR
- Tool grid: all tools for selected category as clickable cards
- Recent files list: last 20 operations, stored locally. Grayed-out with "File not found" badge if file no longer exists; click to remove from list.

**State 2 — Tool Workspace (Arc-style labeled sidebar)**
- 120px labeled sidebar grouped by category; active tool highlighted
- "← Home" link at top
- Main workspace: tool title, breadcrumb, drag-and-drop file zone, tool-specific options, peach CTA button

**State 3 — Focused Mode (icon-only sidebar)**
- 48px icon-only sidebar with category icon tooltips on hover
- Toggle with ⌘\\ (macOS) / Ctrl+\\ (Windows/Linux)
- Workspace content unchanged

### Accessibility
Keyboard accessibility (tab order, focus rings, keyboard shortcuts) is **out of scope for v1**. Explicitly deferred to v2.

---

## Feature Set — 25 Tools across 6 Categories

### Organise (6 tools)

| Tool | Engine | Notes |
|---|---|---|
| Merge PDF | lopdf | Combine multiple files in user-defined order. Multi-file input shows a list with drag-to-reorder. |
| Split PDF | lopdf | Split by page range or every N pages. Split by file size deferred to v2. |
| Compress PDF | pdfium-render + image + lopdf | Three presets: **Small file** (72 DPI image downsampling), **Balanced** (150 DPI), **High quality** (300 DPI). Estimated output size shown instantly as rough percentage of input size (Small ≈ 70% reduction, Balanced ≈ 50%, High ≈ 20%); no pre-processing pass. Pipeline: (1) pdfium-render rasterizes each page; (2) image crate re-encodes at target DPI; (3) lopdf reconstructs the PDF with downsampled image data. |
| Rotate Pages | lopdf | Per-page or all-pages rotation (90°, 180°, 270°) |
| Reorder Pages | lopdf | Drag-and-drop thumbnail reordering. Thumbnails lazy-loaded (viewport-only, 96px height, 4 concurrent Tokio tasks). Documents > 100 pages show "Load more" at bottom. |
| Remove Pages | lopdf | Select pages via thumbnail view. Same lazy-loading rules as Reorder. |

### PDF → Other (5 tools)

| Tool | Engine | Notes |
|---|---|---|
| PDF → Word | pdfium-render + docx-rs | Text blocks and images extracted via pdfium; reflowed into .docx using docx-rs. Best-effort: preserves text and paragraph structure, not pixel-perfect layout. Limitation disclosed in UI before processing. |
| PDF → Excel | pdfium-render + calamine | Heuristic table detection on text blocks; outputs .xlsx. Non-tabular content not included. |
| PDF → PowerPoint | pdfium-render | Each PDF page rendered as a high-res image and inserted as a slide in a .pptx file (ZIP+XML written via quick-xml). Slides are image-based, not editable text. |
| PDF → JPG / PNG | pdfium-render + image | Each page at user-selected DPI (72 / 150 / 300). Individual files or single zip. |
| PDF → PDF/A | pdfium-render | Converts to PDF/A-1b. Pre-conversion check warns if source contains: embedded JavaScript, encryption, transparency effects, or non-embedded fonts. User can proceed (best-effort conversion) or cancel. |

### Other → PDF (5 tools)

| Tool | Engine | Notes |
|---|---|---|
| Word → PDF | docx-rs + printpdf | docx-rs parses .docx content (text, paragraphs, images); printpdf writes the PDF. Basic layout (text flow + images); not pixel-perfect for complex formatting. Limitation disclosed. |
| Excel → PDF | calamine + printpdf | calamine parses .xlsx; printpdf renders table layout. |
| PPT → PDF | quick-xml + printpdf | .pptx parsed as ZIP+XML via quick-xml; each slide rendered as a page in printpdf. Text positions and content extracted from slide XML and placed using printpdf. **Limitation (disclosed in UI):** non-Latin scripts, complex font styling, shapes, backgrounds, and animations may not render correctly. For complex presentations, consider exporting from the original app instead. |
| JPG / PNG → PDF | image crate + lopdf | One image per page; page size matches image dimensions. Multi-file input with drag-to-reorder (same as Merge). |
| HTML → PDF | Tauri WebView + pdfium-render | Local HTML file loaded in a hidden Tauri WebView, printed to PDF via pdfium. Local files only — no external URLs fetched. Tauri's capability config blocks all network access for this WebView. If a local HTML file references external resources (e.g., CDN stylesheets), those resources fail silently and the page renders without them. User is warned in the UI that external resources will not load. |

### Edit (4 tools)

| Tool | Engine | Scope |
|---|---|---|
| Edit PDF | pdfium-render | Annotation-based only: add text boxes, insert images, draw shapes, highlight text. Does **not** modify existing PDF text in-place. |
| Watermark | pdfium-render + lopdf | Text or image watermark with opacity, position, rotation. |
| Page Numbers | lopdf | Font, size, position (header/footer, left/center/right), start number. |
| Redact PDF | lopdf (custom content-stream parser) | **Permanent redaction:** user draws boxes over areas to redact. Implementation: a custom Rust module parses the PDF content stream operator-by-operator (Tj, TJ, Do, and related text/image drawing operators), removes operators whose bounding coordinates fall within each redaction box, then draws a filled black rectangle directly into the content stream over each redacted area. Image content within redaction boxes is replaced with a solid fill block. This is irreversible — a confirmation dialog is shown. The output file contains no recoverable text or image data in redacted regions. This requires a custom content-stream parser; lopdf provides raw stream access but no built-in coordinate-aware operator filter. |

### Security (3 tools)

| Tool | Engine | Scope |
|---|---|---|
| Protect PDF | lopdf | 128-bit AES encryption. Owner password + optional user password. Print / copy / edit permissions. **Implementation note:** lopdf AES-128 write support must be verified against the chosen stable release before committing to this approach. If lopdf does not support AES-128 write, fallback is to bundle `qpdf` binary (Apache 2.0) for encryption operations only. |
| Unlock PDF | lopdf | Removes password from a PDF the user already has the password for. If the password entered is wrong, lopdf returns an error; user sees an error toast and can retry. Temp dir cleaned on failure. |
| Sign PDF | pdfium-render | **Visual signature only (v1):** user draws or uploads a signature image, embedded as a page annotation. Cryptographic digital signing (PAdES / CAdES with certificate infrastructure) is out of scope for v1. |

### Repair & OCR (2 tools)

| Tool | Engine | Scope |
|---|---|---|
| OCR PDF | Tesseract (bundled) | Each page rasterized via pdfium-render, run through Tesseract, invisible text layer embedded into output PDF. Makes scanned PDFs searchable and copy-pasteable. English only in v1. Additional language packs downloadable in v2. Warning shown for files over 50MB due to processing time. |
| Repair PDF | lopdf | Targets: broken / missing cross-reference table, truncated EOF marker, invalid object stream headers. Reconstructs xref from raw object offsets. Does not recover fully encrypted or severely corrupted files. |

---

## PDF Processing Pipeline

Every operation follows the same pipeline:

1. **File Input** — User drags files onto workspace or uses native OS file picker (Tauri dialog API). Svelte emits file paths to Rust via Tauri IPC.

2. **Validation** — Rust checks file headers using magic bytes (not just extension), PDF version compatibility, and file size.
   - Global: soft warning at 500MB, hard block at 2GB.
   - OCR tool: additional warning at 50MB.
   - Password-protected PDFs that require a password for the current operation prompt the user for it before validation continues.

3. **Temp Staging** — Files copied to a session-scoped temp directory (`tempfile` crate, unique ID per operation). Original files are never modified.

4. **Processing** — Tool-specific Rust engine runs in a Tokio async task. Progress events emitted to Svelte every ~100ms for live progress bar. UI thread never blocked. A **Cancel** button is shown during processing. On cancel: Tokio task aborted, temp dir immediately deleted, no output written, user returned to workspace.

5. **Success Notification** — On completion, a success toast shows the output filename and relevant stats (compression ratio, page count, output file size, etc.).

6. **Save & Cleanup** — Native OS save dialog opens first, pre-populated with a suggested filename (see Output Filename Convention below). File is written only after the user confirms the save path. If the user dismisses the dialog, the operation is treated as cancelled: no file is written, temp dir is deleted, recent files list is not updated. After confirmed save: Rust writes output to the user-confirmed path, deletes temp dir, updates recent files JSON.

**Error path:** If processing fails at any step, an error toast is shown with a human-readable message. The temp dir is cleaned. No output is written. The recent files list is not updated. The user remains on the workspace and can retry.

**Concurrency:** Only one active processing operation per workspace. If a user navigates to a different tool while an operation is running, the active operation continues in the background and a progress indicator appears in the sidebar. Starting a second operation from a different workspace tab is allowed (parallel Tokio tasks with separate temp dirs). Running the same tool twice simultaneously is blocked with a "Operation already running" notice.

### Output Filename Convention

Suggested filename = `{original_stem}_{tool}.{ext}`. Examples:
- `report.pdf` merged → `report_merged.pdf`
- `document.pdf` compressed → `document_compressed.pdf`
- `scan.pdf` OCR'd → `scan_ocr.pdf`
- `photo.jpg` converted → `photo.pdf`

For multi-file operations (Merge, JPG→PDF): stem is taken from the **first file** in the input list. E.g., merging `report.pdf` + `invoice.pdf` → `report_merged.pdf`.

Pre-populated in the native save dialog. User can change before saving.

---

## File & Data Management

- **Recent files** — JSON list in OS app config dir (`AppData/Roaming/PavoPDF` / `~/Library/Application Support/PavoPDF` / `~/.config/pavopdf`). Max 20 entries (path, tool used, timestamp). Missing paths shown grayed out in the dashboard; clicking them offers "Remove from list".
- **Settings** — Same dir: sidebar collapsed state, default output folder, OCR language, auto-updater enabled/disabled.
- **No database** — All persistence is flat JSON files.
- **Temp files** — `tempfile` crate, unique per operation, cleaned on completion, failure, cancellation, or next app launch.
- **File size limits** — Global: warn 500MB, hard block 2GB. OCR: additional warn 50MB.

---

## System Architecture

```
┌─────────────────────────────────────────────┐
│  Frontend (Svelte 5 + Tailwind)             │
│  Rendered in OS native WebView              │
│  Dashboard · Workspaces · Progress · Toast  │
└──────────────┬──────────────────────────────┘
               │ Tauri IPC (invoke / events)
               │ process_pdf(tool, inputs, opts)
               │ ← progress events (~100ms)
               │ ← cancel signal
┌──────────────▼──────────────────────────────┐
│  Tauri Commands Layer (Rust)                │
│  #[tauri::command] async fn process_pdf     │
│  open_file_dialog · save_output · recent    │
└──────────┬──────────────┬───────────────────┘
           │              │
┌──────────▼──────┐  ┌────▼──────────────────┐
│  PDF Engine     │  │  Bundled Tools         │
│  pdfium-render  │  │  tesseract binary      │
│  lopdf          │  │  eng.traineddata       │
│  printpdf       │  └───────────────────────┘
│  docx-rs        │
│  calamine       │
│  quick-xml      │
│  image crate    │
└──────────┬──────┘
           │ read/write temp files
┌──────────▼──────────────────────────────────┐
│  Local File System                          │
│  User files · Temp dir · App config (JSON)  │
└─────────────────────────────────────────────┘
```

---

## Distribution & Packaging

- **GitHub Actions CI** — on every git tag, builds all three platform targets and uploads to GitHub Releases
- **Auto-updater** — Tauri built-in updater performs version-number check only (no file data transmitted). Disabled by default; user enables in Settings. Can be disabled at compile time via feature flag.
- **macOS** — Notarization optional (requires $99/yr Apple Developer account). CI supports it via secrets. Unsigned builds functional via right-click workaround.
- **Windows** — Code signing recommended for SmartScreen; added post-launch.
- **Bundle size** — ~120MB estimated installed (figures should be verified per platform during build setup).
- **Website (pavopdf.com)** — Static SvelteKit site: platform download buttons, feature list, GitHub link.

---

## Project Structure (planned)

```
pavopdf/
├── src/                        # Svelte frontend
│   ├── lib/
│   │   ├── components/         # UI components
│   │   ├── stores/             # Svelte stores (recent files, settings)
│   │   └── tools/              # Per-tool Svelte workspace components
│   └── app.css                 # Tailwind entry
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/           # Tauri IPC commands
│   │   └── tools/              # PDF processing modules
│   │       ├── organise/       # merge, split, compress, rotate, reorder, remove
│   │       ├── convert_from/   # pdf→word, excel, ppt, jpg, pdf-a
│   │       ├── convert_to/     # word→pdf, excel, ppt, jpg→pdf, html→pdf
│   │       ├── edit/           # edit, watermark, page_numbers, redact
│   │       ├── security/       # protect, unlock, sign
│   │       └── repair/         # ocr, repair
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/
│   └── superpowers/specs/
└── .github/
    └── workflows/
        └── release.yml         # Cross-platform build + release CI
```

---

## Out of Scope (v1)

- Dark mode
- Additional OCR languages beyond English (v2 — user-downloadable lang packs)
- Batch processing queue UI
- PDF form creation
- In-place text editing of existing PDF content
- Cryptographic digital signing (PAdES / CAdES)
- Full output preview before saving
- Split PDF by file size
- Keyboard accessibility / WCAG compliance
- Mobile / web version
- Cloud sync
