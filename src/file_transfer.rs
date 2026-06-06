use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};

#[derive(Clone)]
pub struct LocalFileEntry {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

pub fn default_local_dir(preferred: &str) -> PathBuf {
    if !preferred.trim().is_empty() {
        let path = PathBuf::from(preferred);
        if path.is_dir() {
            return path;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
}

pub fn resolve_local_path(current: &str, target: &str) -> PathBuf {
    if target == ".." {
        return Path::new(current)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(current));
    }
    PathBuf::from(target)
}

pub fn list_local_dir(path: impl AsRef<Path>) -> Result<(String, Vec<LocalFileEntry>)> {
    let path = path.as_ref();
    let display_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let mut entries = Vec::new();
    for item in std::fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let item = item?;
        let meta = item.metadata()?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        entries.push(LocalFileEntry {
            name: item.file_name().to_string_lossy().to_string(),
            full_path: item.path().to_string_lossy().to_string(),
            is_dir: meta.is_dir(),
            size: if meta.is_file() { meta.len() } else { 0 },
            modified,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok((display_path, entries))
}
