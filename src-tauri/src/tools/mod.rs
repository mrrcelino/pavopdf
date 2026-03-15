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

    // Tool dispatch — ALL TOOLS RETURN AN ERROR IN PLAN 1.
    // This is intentional: tool implementations arrive in Plans 2-6.
    // If you call process_pdf from the frontend during Plan 1, expect an error response.
    // Do not wire up tool-trigger UI in Plan 1 — only test storage and pipeline commands.
    #[allow(unreachable_patterns)]
    let output_path = match req.tool {
        _ => Err(crate::error::AppError::Pdf(
            format!("Tool '{:?}' not yet implemented — see Plans 2-6", req.tool)
        )),
    }?;

    progress::emit_progress(&app, &req.operation_id, 100, "Done");
    Ok(output_path)
}
