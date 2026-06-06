use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Model, SharedString, VecModel};

use super::platform::cursor_pos;
use super::types::SftpHandles;
use super::{AppWindow, SftpManualNav, TerminalState};

pub(super) fn wire_sftp_callbacks(
    window: &AppWindow,
    sftp_handles: SftpHandles,
    sftp_manual_nav: SftpManualNav,
) {
    // Navigate to a remote path (or ".." to go up one level).
    {
        let sftp_handles = sftp_handles.clone();
        let sftp_manual_nav = sftp_manual_nav.clone();
        let weak = window.as_weak();
        window.on_sftp_navigate(move |tab_id: SharedString, path: SharedString| {
            let tab_id = tab_id.to_string();
            let resolved = if path.as_str() == ".." {
                let current = weak.upgrade().and_then(|w| {
                    let terminals_rc = w.get_terminals();
                    let terminals = terminals_rc
                        .as_any()
                        .downcast_ref::<VecModel<TerminalState>>()?;
                    for i in 0..terminals.row_count() {
                        if let Some(row) = terminals.row_data(i) {
                            if row.id.as_str() == tab_id {
                                return Some(row.sftp_path.to_string());
                            }
                        }
                    }
                    None
                });
                parent_path(&current.unwrap_or_else(|| "/".to_string()))
            } else {
                path.to_string()
            };
            // Any manual navigation stops cd auto-follow.
            sftp_manual_nav.lock().unwrap().insert(tab_id.clone(), true);
            if let Ok(handles) = sftp_handles.lock() {
                if let Some(h) = handles.get(&tab_id) {
                    h.list_dir(resolved);
                }
            }
        });
    }

    // Download a remote file.  If a download folder is preset in settings, save
    // straight there; otherwise fall back to a native folder picker.
    {
        let sftp_handles = sftp_handles.clone();
        let weak = window.as_weak();
        window.on_sftp_download(move |tab_id: SharedString, remote_path: SharedString| {
            let tab_id = tab_id.to_string();
            let remote_path = remote_path.to_string();
            let preset = weak
                .upgrade()
                .map(|w| w.get_download_dir().to_string())
                .unwrap_or_default();
            if !preset.is_empty() {
                if let Ok(handles) = sftp_handles.lock() {
                    if let Some(h) = handles.get(&tab_id) {
                        h.download(remote_path, preset);
                    }
                }
                return;
            }
            let sftp_handles = sftp_handles.clone();
            std::thread::spawn(move || {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    let local_dir = dir.to_string_lossy().to_string();
                    if let Ok(handles) = sftp_handles.lock() {
                        if let Some(h) = handles.get(&tab_id) {
                            h.download(remote_path, local_dir);
                        }
                    }
                }
            });
        });
    }

    // Upload a local file into the current remote directory.
    {
        let sftp_handles = sftp_handles.clone();
        window.on_sftp_upload_clicked(move |tab_id: SharedString, remote_dir: SharedString| {
            let tab_id = tab_id.to_string();
            let remote_dir = remote_dir.to_string();
            let sftp_handles = sftp_handles.clone();
            std::thread::spawn(move || {
                if let Some(file) = rfd::FileDialog::new().pick_file() {
                    let local = file.to_string_lossy().to_string();
                    if let Ok(handles) = sftp_handles.lock() {
                        if let Some(h) = handles.get(&tab_id) {
                            h.upload(local, remote_dir);
                        }
                    }
                }
            });
        });
    }

    // Refresh the current directory listing.
    {
        let sftp_handles = sftp_handles.clone();
        window.on_sftp_refresh(move |tab_id: SharedString, path: SharedString| {
            let tab_id = tab_id.to_string();
            let path = path.to_string();
            if let Ok(handles) = sftp_handles.lock() {
                if let Some(h) = handles.get(&tab_id) {
                    h.list_dir(path);
                }
            }
        });
    }

    // Toggle tree node expand/collapse and navigate to that directory.
    {
        let sftp_handles = sftp_handles.clone();
        let sftp_manual_nav = sftp_manual_nav.clone();
        window.on_sftp_tree_expand(move |tab_id: SharedString, path: SharedString| {
            let tab_id = tab_id.to_string();
            let path = path.to_string();
            // Manual tree navigation stops cd auto-follow.
            sftp_manual_nav.lock().unwrap().insert(tab_id.clone(), true);
            if let Ok(handles) = sftp_handles.lock() {
                if let Some(h) = handles.get(&tab_id) {
                    h.toggle_tree_node(path.clone());
                    h.list_dir(path);
                }
            }
        });
    }

    // Context menu → 删除 a remote file.
    {
        let sftp_handles = sftp_handles.clone();
        window.on_sftp_delete(move |tab_id: SharedString, path: SharedString| {
            if let Ok(handles) = sftp_handles.lock() {
                if let Some(h) = handles.get(tab_id.as_str()) {
                    h.delete(path.to_string());
                }
            }
        });
    }

    // Context menu → 查看 (open read-only) / 编辑 (open + auto-reupload).
    {
        let sftp_handles = sftp_handles.clone();
        window.on_sftp_view(move |tab_id: SharedString, path: SharedString| {
            if let Ok(handles) = sftp_handles.lock() {
                if let Some(h) = handles.get(tab_id.as_str()) {
                    h.open_temp(path.to_string(), false);
                }
            }
        });
    }
    {
        let sftp_handles = sftp_handles.clone();
        window.on_sftp_edit(move |tab_id: SharedString, path: SharedString| {
            if let Ok(handles) = sftp_handles.lock() {
                if let Some(h) = handles.get(tab_id.as_str()) {
                    h.open_temp(path.to_string(), true);
                }
            }
        });
    }
}

/// The active terminal tab's current SFTP directory ("" if unknown).
pub(super) fn active_sftp_path(win: &AppWindow, tab_id: &str) -> String {
    let model = win.get_terminals();
    if let Some(m) = model.as_any().downcast_ref::<VecModel<TerminalState>>() {
        for i in 0..m.row_count() {
            if let Some(row) = m.row_data(i) {
                if row.id.as_str() == tab_id {
                    return row.sftp_path.to_string();
                }
            }
        }
    }
    String::new()
}

/// Current mouse cursor position in physical screen pixels (Windows).
/// Handle an OS file drop: if it landed over the SFTP file-list area of the
/// active session tab, upload the file to that tab's current remote directory.
#[cfg(windows)]
pub(super) fn handle_file_drop(win: &AppWindow, sftp_handles: &SftpHandles, path: String) {
    let active = win.get_active_tab_id().to_string();
    if active == "welcome" {
        return;
    }
    let w = win.window();
    let scale = w.scale_factor().max(0.01);
    let size = w.size(); // physical
    let Some(inner) = w.with_winit_window(|ww| ww.inner_position().ok()).flatten() else {
        return;
    };
    let Some((cx, cy)) = cursor_pos() else {
        return;
    };
    // Drop point in logical client coordinates.
    let client_x = (cx - inner.x) as f32 / scale;
    let client_y = (cy - inner.y) as f32 / scale;
    let w_logical = size.width as f32 / scale;
    let h_logical = size.height as f32 / scale;
    let h_sftp = win.get_sftp_panel_height();

    // File-list box (logical): right of the sidebar(220)+tree(160)+sep(1),
    // below the SFTP toolbar(30)+header(20)+sep(1), above the status bar(18).
    let zone_left = 381.0_f32;
    let zone_top = h_logical - h_sftp + 51.0;
    let zone_bottom = h_logical - 18.0;
    if client_x < zone_left || client_x > w_logical || client_y < zone_top || client_y > zone_bottom
    {
        return; // dropped outside the file list — ignore
    }

    let dir = active_sftp_path(win, &active);
    if dir.is_empty() {
        return;
    }
    if let Ok(handles) = sftp_handles.lock() {
        if let Some(h) = handles.get(&active) {
            h.upload(path, dir);
        }
    }
}

#[cfg(not(windows))]
pub(super) fn handle_file_drop(_win: &AppWindow, _sftp_handles: &SftpHandles, _path: String) {}

/// Return the parent directory of `path`.
/// "/a/b/c" → "/a/b", "/a" → "/", "/" → "/"
pub(super) fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    match trimmed.rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => trimmed[..i].to_string(),
        None => "/".to_string(),
    }
}
