pub mod organise;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::Result;
use crate::pipeline::{temp::TempStage, progress};
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
    let stage = TempStage::new()?;
    progress::emit_progress(&app, &req.operation_id, 10, "Staging files...");

    // Stage all input files
    let _staged_inputs: Vec<PathBuf> = req.input_paths
        .iter()
        .map(|p| stage.stage_file(p))
        .collect::<Result<_>>()?;

    progress::emit_progress(&app, &req.operation_id, 20, "Processing...");

    // Capture values needed after the match before ownership is transferred.
    let op_id = req.operation_id.clone();
    let app2 = app.clone();

    // Tool dispatch — organise tools are wired in Plan 2; remaining tools arrive in Plans 3-6.
    #[allow(unreachable_patterns)]
    let output_path = match req.tool {
        Tool::Merge    => organise::merge::run(app, req).await,
        Tool::Split    => organise::split::run(app, req).await,
        Tool::Compress => organise::compress::run(app, req).await,
        Tool::Rotate   => organise::rotate::run(app, req).await,
        Tool::Reorder  => organise::reorder::run(app, req).await,
        Tool::Remove   => organise::remove::run(app, req).await,
        _ => Err(crate::error::AppError::Pdf(
            format!("Tool '{:?}' not yet implemented — see Plans 3-6", req.tool)
        )),
    }?;

    progress::emit_progress(&app2, &op_id, 100, "Done");
    Ok(output_path)
}
