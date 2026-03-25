use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::ImageFormat;
use lopdf::Document;
use pdfium_render::prelude::*;
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::pipeline::validate::validate_pdf;

#[derive(Serialize)]
pub struct ThumbnailResponse {
    pub page: u32,
    pub data_url: String,
}

#[tauri::command]
pub fn get_page_count(path: PathBuf) -> Result<usize> {
    validate_pdf(&path, "rotate")?;

    let document = Document::load(&path)
        .map_err(|e| AppError::Pdf(format!("Failed to load PDF: {e}")))?;

    Ok(document.get_pages().len())
}

#[tauri::command]
pub fn render_page_thumbnail(
    path: PathBuf,
    page: u32,
    width: u32,
    height: u32,
) -> Result<ThumbnailResponse> {
    validate_pdf(&path, "rotate")?;

    if page == 0 {
        return Err(AppError::Validation(
            "Page numbers are 1-based".into(),
        ));
    }
    if width == 0 || height == 0 {
        return Err(AppError::Validation(
            "Thumbnail width and height must be greater than 0".into(),
        ));
    }

    let pdfium = catch_unwind(AssertUnwindSafe(Pdfium::default))
        .map_err(|_| AppError::Pdf("Failed to initialize Pdfium runtime".into()))?;
    let document = pdfium
        .load_pdf_from_file(&path, None)
        .map_err(|e| AppError::Pdf(format!("Failed to render PDF: {e}")))?;
    let pages = document.pages();
    let page_index = page - 1;

    if page_index as usize >= pages.len() as usize {
        return Err(AppError::Validation(format!(
            "Page {page} is out of range"
        )));
    }
    if page_index > u16::MAX as u32 {
        return Err(AppError::Validation(
            "Thumbnail rendering currently supports page numbers up to 65536".into(),
        ));
    }

    let page_ref = pages
        .get(page_index as u16)
        .map_err(|e| AppError::Pdf(format!("Failed to access page {page}: {e}")))?;
    let bitmap = page_ref
        .render_with_config(
            &PdfRenderConfig::new().set_target_size(width as i32, height as i32),
        )
        .map_err(|e| AppError::Pdf(format!("Failed to render page {page}: {e}")))?;
    let image = bitmap.as_image();

    let mut png_bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .map_err(|e| AppError::Pdf(format!("Failed to encode thumbnail: {e}")))?;

    Ok(ThumbnailResponse {
        page,
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(png_bytes)),
    })
}
