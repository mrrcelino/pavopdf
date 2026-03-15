use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager; // provides AppHandle::path()
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
    // Case-insensitive dedup for Windows path compatibility.
    let norm = entry.path.to_string_lossy().to_lowercase();
    entries.retain(|e| e.path.to_string_lossy().to_lowercase() != norm);
    entries.insert(0, entry);
    entries.truncate(MAX_RECENT);
    save_entries(app_handle, &entries)
}

pub fn remove(app_handle: &tauri::AppHandle, path: &Path) -> Result<()> {
    // Case-insensitive comparison for Windows path compatibility.
    let norm = path.to_string_lossy().to_lowercase();
    let mut entries = load(app_handle)?;
    entries.retain(|e| e.path.to_string_lossy().to_lowercase() != norm);
    save_entries(app_handle, &entries)
}

fn save_entries(app_handle: &tauri::AppHandle, entries: &[RecentEntry]) -> Result<()> {
    let path = recent_path(app_handle)?;
    let contents = serde_json::to_string_pretty(entries)
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
    fn truncate_to_max_recent() {
        // Simulate what push() does: dedup + prepend + truncate
        let mut entries: Vec<RecentEntry> = (0..25)
            .map(|i| RecentEntry {
                path: format!("/tmp/file{i}.pdf").into(),
                tool: "merge".into(),
                timestamp: i as u64,
                exists: false,
            })
            .collect();
        let new_entry = RecentEntry {
            path: "/tmp/new.pdf".into(),
            tool: "split".into(),
            timestamp: 999,
            exists: false,
        };
        // Simulate push logic
        entries.retain(|e| e.path != new_entry.path);
        entries.insert(0, new_entry);
        entries.truncate(MAX_RECENT);
        assert_eq!(entries.len(), MAX_RECENT);
        assert_eq!(entries[0].path, std::path::PathBuf::from("/tmp/new.pdf"));
        assert_eq!(entries[0].tool, "split");
    }

    #[test]
    fn push_deduplicates_same_path() {
        let mut entries = vec![
            RecentEntry { path: "/tmp/a.pdf".into(), tool: "merge".into(), timestamp: 1, exists: false },
            RecentEntry { path: "/tmp/b.pdf".into(), tool: "split".into(), timestamp: 2, exists: false },
        ];
        let dup = RecentEntry { path: "/tmp/a.pdf".into(), tool: "compress".into(), timestamp: 3, exists: false };
        // Simulate push dedup
        entries.retain(|e| e.path != dup.path);
        entries.insert(0, dup);
        assert_eq!(entries.len(), 2, "duplicate should be removed before insert");
        assert_eq!(entries[0].path, std::path::PathBuf::from("/tmp/a.pdf"));
        assert_eq!(entries[0].tool, "compress", "new entry should replace old one");
    }
}
