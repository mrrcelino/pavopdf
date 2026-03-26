<p align="center">
  <img src="https://pavopdf.com/logo.svg" alt="PavoPDF" width="80" height="80" />
</p>

<h1 align="center">PavoPDF</h1>

<p align="center">
  <strong>Free, open-source, offline PDF toolkit for Windows, macOS, and Linux.</strong>
</p>

<p align="center">
  <a href="https://pavopdf.com">Download</a> &nbsp;&middot;&nbsp;
  <a href="#features">Features</a> &nbsp;&middot;&nbsp;
  <a href="#building-from-source">Build</a> &nbsp;&middot;&nbsp;
  <a href="#contributing">Contribute</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/license/mrrcelino/pavopdf?style=flat-square" alt="License" />
  <img src="https://img.shields.io/badge/tests-174%20passed-brightgreen?style=flat-square" alt="Tests" />
  <img src="https://img.shields.io/badge/tools-25-blue?style=flat-square" alt="Tools" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=flat-square" alt="Platform" />
</p>

---

PavoPDF is a desktop application that handles all your PDF needs without uploading files to the cloud. Every operation runs locally on your machine — your documents never leave your computer.

Built with **Tauri 2** (Rust) + **Svelte 5** + **Tailwind CSS**.

## Features

### Organise

| Tool | Description |
|------|-------------|
| **Merge** | Combine multiple PDFs into one. Drag to reorder. |
| **Split** | Extract page ranges or split every N pages. |
| **Compress** | Reduce file size with small / balanced / high-quality presets. |
| **Rotate** | Rotate pages by 90°, 180°, or 270°. |
| **Reorder** | Rearrange pages in any order. |
| **Remove** | Delete specific pages from a PDF. |

### Convert from PDF

| Tool | Description |
|------|-------------|
| **PDF → Word** | Extract text into a formatted `.docx` file. |
| **PDF → Excel** | Convert tables to `.xlsx` spreadsheets. |
| **PDF → PowerPoint** | Generate `.pptx` slides from PDF pages. |
| **PDF → Image** | Export pages as PNG or JPEG at configurable DPI. |
| **PDF → PDF/A** | Best-effort PDF/A conformance conversion. |

### Convert to PDF

| Tool | Description |
|------|-------------|
| **Word → PDF** | Convert `.docx` documents to PDF. |
| **Excel → PDF** | Convert `.xlsx` / `.xls` / `.ods` spreadsheets to PDF. |
| **PowerPoint → PDF** | Convert `.pptx` presentations to PDF. |
| **Image → PDF** | Combine images into a multi-page PDF (fit-to-image or A4). |
| **HTML → PDF** | Convert `.html` files to PDF with text extraction. |

### Edit

| Tool | Description |
|------|-------------|
| **Edit Metadata** | Set or clear title, author, subject, keywords, creator. |
| **Watermark** | Add rotated text watermarks with configurable opacity. |
| **Page Numbers** | Add "Page X of Y" to every page (bottom center/left/right). |
| **Redact** | Draw black rectangles over sensitive areas (visual redaction). |

### Security & Repair

| Tool | Description |
|------|-------------|
| **Protect** | Password-protect a PDF (encryption planned for v0.2). |
| **Unlock** | Remove password protection with the correct password. |
| **Sign** | Draw a signature on a canvas and embed it on any page (transparent PNG with alpha). |
| **OCR** | Make scanned PDFs searchable via Tesseract (with pdftoppm fallback). |
| **Repair** | Fix corrupted PDFs through lenient re-parsing and structure normalization. |

## Download

Pre-built installers are available at **[pavopdf.com](https://pavopdf.com)**:

| Platform | Format |
|----------|--------|
| Windows | `.msi` / `.exe` |
| macOS | `.dmg` |
| Linux | `.AppImage` / `.deb` |

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.77+
- [Tauri CLI](https://tauri.app/start/): `cargo install tauri-cli`

### Optional (for OCR)

- [Tesseract OCR](https://github.com/UB-Mannheim/tesseract/wiki) on PATH
- [poppler-utils](https://poppler.freedesktop.org/) (provides `pdftoppm` for PDF-to-image rasterization)

### Steps

```bash
# Clone the repo
git clone https://github.com/mrrcelino/pavopdf.git
cd pavopdf

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

The built installer will be in `src-tauri/target/release/bundle/`.

### Running Tests

```bash
# Rust tests (174 tests)
cd src-tauri && cargo test

# Svelte type check
npm run check
```

## Architecture

```
pavopdf/
├── src/                          # Svelte 5 frontend
│   └── lib/
│       ├── api.ts                # Tauri IPC wrapper
│       ├── stores/               # Svelte 5 rune stores
│       ├── components/
│       │   ├── layout/           # Shell, sidebar, workspace router
│       │   └── tools/
│       │       ├── organise/     # 6 tool UIs
│       │       ├── convert_from/ # 5 converter UIs
│       │       ├── convert_to/   # 5 converter UIs
│       │       ├── edit/         # 4 tool UIs
│       │       ├── security/     # 3 tool UIs
│       │       └── repair/       # 2 tool UIs
│       └── types.ts
│
├── src-tauri/                    # Rust backend
│   └── src/
│       ├── tools/
│       │   ├── mod.rs            # ProcessRequest, Tool enum, dispatch
│       │   ├── organise/         # merge, split, compress, rotate, reorder, remove
│       │   ├── convert_from/     # to_word, to_excel, to_ppt, to_image, to_pdfa
│       │   ├── convert_to/       # from_word, from_excel, from_ppt, from_image, from_html
│       │   ├── edit/             # metadata, watermark, page_numbers, redact
│       │   ├── security/         # protect, unlock, sign
│       │   └── repair/           # ocr, repair
│       ├── pipeline/             # TempStage, progress events, validation
│       └── error.rs              # AppError enum
│
└── docs/                         # Design specs and implementation plans
```

### Key Design Decisions

- **Offline-first**: All processing happens locally via Rust. No network calls, no telemetry.
- **lopdf 0.31**: Pinned for stability. Manual implementations where higher-level APIs are missing (e.g., merge via object renumbering, watermark via content stream injection).
- **Immutable patterns**: Svelte 5 runes with immutable array/object updates throughout.
- **Output next to input**: All tools save output files alongside the input (not in temp directories) to avoid auto-deletion race conditions.
- **Transparent signatures**: PNG signatures retain their alpha channel via SMask, so signatures appear as ink on top of the page — not opaque white rectangles.

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop shell | [Tauri 2](https://tauri.app/) |
| Backend | Rust + [lopdf](https://crates.io/crates/lopdf) + [printpdf](https://crates.io/crates/printpdf) + [image](https://crates.io/crates/image) |
| Frontend | [Svelte 5](https://svelte.dev/) + [Tailwind CSS](https://tailwindcss.com/) |
| PDF parsing | lopdf 0.31 (structural mutations) |
| PDF rendering | pdfium-render (page thumbnails) |
| Document parsing | docx-rs (Word), calamine (Excel), quick-xml + zip (PowerPoint) |
| OCR | Tesseract (external binary) |

## Known Limitations

- **Protect PDF**: lopdf 0.31 does not support PDF encryption natively. The current implementation saves a clean copy. True AES-128 encryption is planned for v0.2 (via lopdf upgrade or qpdf integration).
- **Redact**: Visual-only redaction. Black rectangles are drawn over regions, but underlying text bytes are not stripped. For forensic-grade redaction, additional content stream processing is needed.
- **OCR**: Requires Tesseract installed separately. Falls back to a pdftoppm pipeline if Tesseract can't process the PDF directly.
- **Convert to/from**: Text-based extraction. Complex layouts, images embedded in documents, and CSS styling in HTML are not fully preserved.

## Contributing

Contributions are welcome! Please:

1. Fork the repo
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Write tests for new functionality
4. Ensure `cargo test` and `npm run check` pass
5. Open a pull request

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<p align="center">
  <sub>Built with Rust and Svelte. Made offline by design.</sub>
  <br />
  <sub>Crafted with <a href="https://claude.ai">Claude</a> by Anthropic.</sub>
</p>
