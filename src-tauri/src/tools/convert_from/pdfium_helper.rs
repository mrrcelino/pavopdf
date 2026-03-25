use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use pdfium_render::prelude::*;

use crate::error::{AppError, Result};

/// Initialise pdfium from the system library. Wraps the call in
/// `catch_unwind` so a missing / incompatible binary does not abort
/// the entire process.
pub fn load_pdfium() -> Result<Pdfium> {
    catch_unwind(AssertUnwindSafe(Pdfium::default))
        .map_err(|_| AppError::Pdf("Failed to initialise Pdfium runtime".into()))
}

/// Open a PDF file using an already-loaded Pdfium instance.
pub fn open_pdf<'a>(
    pdfium: &'a Pdfium,
    path: &Path,
    password: Option<&'a str>,
) -> Result<PdfDocument<'a>> {
    pdfium
        .load_pdf_from_file(path, password)
        .map_err(|e| AppError::Pdf(format!("Failed to open PDF: {e}")))
}
