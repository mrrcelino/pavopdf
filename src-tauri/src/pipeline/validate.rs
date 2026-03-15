use std::path::Path;
use crate::error::{AppError, Result};

const PDF_MAGIC: &[u8] = b"%PDF";
const WARN_SIZE_BYTES: u64 = 500 * 1024 * 1024;   // 500 MB
const BLOCK_SIZE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB
const OCR_WARN_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB

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
        let padding = size.saturating_sub(9);
        if padding > 0 {
            // Write in chunks to avoid huge stack allocations
            let chunk = vec![b'x'; 65536.min(padding as usize)];
            let mut remaining = padding;
            while remaining > 0 {
                let to_write = remaining.min(chunk.len() as u64) as usize;
                f.write_all(&chunk[..to_write]).unwrap();
                remaining -= to_write as u64;
            }
        }
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
        // Writes ~51MB to disk — the OCR warning threshold is 50MB
        let f = make_pdf_file(OCR_WARN_SIZE_BYTES + 1024);
        let warnings = validate_pdf(f.path(), "ocr").unwrap();
        let has_ocr_warning = warnings.iter().any(|w| matches!(w.kind, WarningKind::OcrLargeFile));
        assert!(has_ocr_warning, "expected OCR large-file warning for >50MB file");
    }
}
