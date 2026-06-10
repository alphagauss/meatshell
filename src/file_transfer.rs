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

pub fn open_local_path(path: &str) -> Result<()> {
    open_with_os(path)
}

pub fn edit_local_path(path: &str) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("notepad.exe")
            .arg(path)
            .spawn()
            .with_context(|| format!("open notepad for {path}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if let Some(editor) = std::env::var_os("VISUAL").or_else(|| std::env::var_os("EDITOR")) {
            std::process::Command::new(editor)
                .arg(path)
                .spawn()
                .with_context(|| format!("open editor for {path}"))?;
            Ok(())
        } else {
            open_with_os(path)
        }
    }
}

pub fn rename_local_path(path: &str, new_name: &str) -> Result<()> {
    let name = clean_new_name(new_name)?;
    let path = Path::new(path);
    let parent = path
        .parent()
        .context("local path has no parent directory")?;
    std::fs::rename(path, parent.join(name)).with_context(|| format!("rename {}", path.display()))
}

fn clean_new_name(new_name: &str) -> Result<&str> {
    let name = new_name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        anyhow::bail!("invalid file name");
    }
    Ok(name)
}

#[cfg(windows)]
fn open_with_os(path: &str) -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: isize,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    let to_wide = |s: &str| -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let op = to_wide("open");
    let file = to_wide(path);
    let result = unsafe {
        ShellExecuteW(
            0,
            op.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result <= 32 {
        anyhow::bail!("open failed with ShellExecuteW code {result}");
    }
    Ok(())
}

#[cfg(not(windows))]
fn open_with_os(path: &str) -> Result<()> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .with_context(|| format!("open {path}"))?;
    Ok(())
}
