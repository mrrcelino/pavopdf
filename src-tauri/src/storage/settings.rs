use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager; // provides AppHandle::path()
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub sidebar_collapsed: bool,
    pub default_output_folder: Option<PathBuf>,
    pub ocr_language: String,
    pub auto_updater_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            default_output_folder: None,
            ocr_language: "eng".into(),
            auto_updater_enabled: false,
        }
    }
}

pub fn settings_path(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

pub fn load(app_handle: &tauri::AppHandle) -> Result<Settings> {
    let path = settings_path(app_handle)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let contents = std::fs::read_to_string(&path)?;
    match serde_json::from_str(&contents) {
        Ok(s) => Ok(s),
        Err(_) => {
            // Corrupt settings file — return defaults rather than failing.
            // The next save() will overwrite with valid JSON.
            Ok(Settings::default())
        }
    }
}

pub fn save(app_handle: &tauri::AppHandle, settings: &Settings) -> Result<()> {
    let path = settings_path(app_handle)?;
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Io(e.to_string()))?;
    // Atomic write: write to temp file then rename to avoid partial writes.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &contents)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: load(), save(), and settings_path() require a live tauri::AppHandle
    // and cannot be unit-tested without a running Tauri app. Verified via
    // integration testing when the full app runs. Tests below cover the pure logic.

    #[test]
    fn default_settings_are_valid() {
        let s = Settings::default();
        assert!(!s.sidebar_collapsed);
        assert!(!s.auto_updater_enabled);
        assert_eq!(s.ocr_language, "eng");
    }

    #[test]
    fn settings_roundtrip_json() {
        let s = Settings {
            sidebar_collapsed: true,
            default_output_folder: Some("/tmp/out".into()),
            ocr_language: "eng".into(),
            auto_updater_enabled: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sidebar_collapsed, true);
        assert_eq!(restored.ocr_language, "eng");
    }
}
