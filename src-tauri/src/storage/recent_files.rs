use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;
use crate::error::{AppError, Result};

const MAX_RECENT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub tool: String,
    pub timestamp: u64,
    pub exists: bool,
}

impl RecentEntry {
    pub fn new(path: PathBuf, tool: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let exists = path.exists();
        Self { path, tool, timestamp, exists }
    }
}

pub fn recent_path(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("recent_files.json"))
}

pub fn load(app_handle: &tauri::AppHandle) -> Result<Vec<RecentEntry>> {
    let path = recent_path(app_handle)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let contents = std::fs::read_to_string(&path)?;
    let mut entries: Vec<RecentEntry> = serde_json::from_str(&contents)
        .map_err(|e| AppError::Io(format!("Failed to parse recent files: {e}")))?;
    // Refresh exists flag on load
    for entry in &mut entries {
        entry.exists = entry.path.exists();
    }
    Ok(entries)
}

pub fn push(app_handle: &tauri::AppHandle, entry: RecentEntry) -> Result<()> {
    let mut entries = load(app_handle)?;
    // Remove any existing entry for the same path
    entries.retain(|e| e.path != entry.path);
    entries.insert(0, entry);
    entries.truncate(MAX_RECENT);
    save_entries(app_handle, &entries)
}

pub fn remove(app_handle: &tauri::AppHandle, path: &Path) -> Result<()> {
    let mut entries = load(app_handle)?;
    entries.retain(|e| e.path != path);
    save_entries(app_handle, &entries)
}

fn save_entries(app_handle: &tauri::AppHandle, entries: &[RecentEntry]) -> Result<()> {
    let path = recent_path(app_handle)?;
    let contents = serde_json::to_string_pretty(entries)
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::write(&path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_entry_marks_nonexistent_path() {
        let entry = RecentEntry::new("/nonexistent/path/file.pdf".into(), "merge".into());
        assert!(!entry.exists);
    }

    #[test]
    fn recent_entry_roundtrip() {
        let entry = RecentEntry {
            path: "/tmp/test.pdf".into(),
            tool: "compress".into(),
            timestamp: 1700000000,
            exists: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: RecentEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.tool, "compress");
        assert_eq!(restored.timestamp, 1700000000);
    }

    #[test]
    fn max_recent_enforced() {
        let entries: Vec<RecentEntry> = (0..25)
            .map(|i| RecentEntry {
                path: format!("/tmp/file{i}.pdf").into(),
                tool: "merge".into(),
                timestamp: i as u64,
                exists: false,
            })
            .collect();
        let mut truncated = entries;
        truncated.truncate(MAX_RECENT);
        assert_eq!(truncated.len(), 20);
    }
}
