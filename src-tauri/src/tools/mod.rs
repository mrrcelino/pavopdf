pub mod organise;
pub mod convert_from;
pub mod convert_to;
pub mod edit;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::Result;
use tauri::AppHandle;

#[derive(Debug, Deserialize, Serialize)]
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
    #[allow(unreachable_patterns)]
    let output_path = match req.tool {
        Tool::Merge    => organise::merge::run(app, req).await,
        Tool::Split    => organise::split::run(app, req).await,
        Tool::Compress => organise::compress::run(app, req).await,
        Tool::Rotate   => organise::rotate::run(app, req).await,
        Tool::Reorder  => organise::reorder::run(app, req).await,
        Tool::Remove   => organise::remove::run(app, req).await,
        // Plan 3
        Tool::PdfToWord  => convert_from::to_word::run(app, req).await,
        Tool::PdfToExcel => convert_from::to_excel::run(app, req).await,
        Tool::PdfToPpt   => convert_from::to_ppt::run(app, req).await,
        Tool::PdfToImage => convert_from::to_image::run(app, req).await,
        Tool::PdfToPdfa  => convert_from::to_pdfa::run(app, req).await,
        // Plan 4
        Tool::WordToPdf  => convert_to::from_word::run(app, req).await,
        Tool::ExcelToPdf => convert_to::from_excel::run(app, req).await,
        Tool::PptToPdf   => convert_to::from_ppt::run(app, req).await,
        Tool::ImageToPdf => convert_to::from_image::run(app, req).await,
        Tool::HtmlToPdf  => convert_to::from_html::run(app, req).await,
        // Plan 5
        Tool::Edit        => edit::metadata::run(app, req).await,
        Tool::Watermark   => edit::watermark::run(app, req).await,
        Tool::PageNumbers => edit::page_numbers::run(app, req).await,
        Tool::Redact      => edit::redact::run(app, req).await,
        _ => Err(crate::error::AppError::Pdf(
            format!("Tool '{:?}' not yet implemented — see Plan 6", req.tool)
        )),
    }?;

    Ok(output_path)
}
