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
