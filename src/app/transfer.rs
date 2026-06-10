use std::rc::Rc;
use std::sync::Arc;

use anyhow::{Context, Result};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::config::{ConfigStore, Session};
use crate::file_transfer::{default_local_dir, list_local_dir, resolve_local_path, LocalFileEntry};
use crate::i18n::t;
use crate::sftp::{spawn_sftp, SftpHandle};
use crate::ssh::{format_mtime, format_size, RemoteEntry, SessionEvent};

use super::models::{active_session_or_hint, set_terminal_row};
use super::sftp_panel::parent_path;
use super::types::{TransferWindowState, TransferWindows};
use super::{SftpEntry, TransferWindow};

pub(super) fn open_transfer_window(
    session: Session,
    runtime: Arc<Runtime>,
    preferred_local_dir: String,
    transfer_windows: TransferWindows,
) -> Result<()> {
    let window = TransferWindow::new().context("failed to build transfer window")?;
    window.set_session_title(
        format!(
            "{}  {}@{}:{}",
            t("文件传输", "File transfer"),
            session.user,
            session.host,
            session.port
        )
        .into(),
    );
    window.set_remote_path("/".into());
    window.set_remote_status(t("SFTP 连接中...", "SFTP connecting...").into());
    window.set_remote_loading(true);
    window.set_remote_entries(ModelRc::from(Rc::new(VecModel::<SftpEntry>::default())));
    refresh_transfer_local(&window, default_local_dir(&preferred_local_dir));

    let (sftp_tx, sftp_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
    let sftp = Rc::new(spawn_sftp(runtime.handle(), session, sftp_tx));
    wire_transfer_window_callbacks(&window, sftp.clone());
    spawn_transfer_sftp_event_pump(window.as_weak(), sftp_rx);
    window.window().on_close_requested({
        let sftp = sftp.clone();
        move || {
            sftp.close();
            slint::CloseRequestResponse::HideWindow
        }
    });
    window.show().context("failed to show transfer window")?;
    transfer_windows.borrow_mut().push(TransferWindowState {
        _window: window,
        _sftp: sftp,
    });
    Ok(())
}

pub(super) fn wire_transfer_toolbar_callbacks(
    window: &super::AppWindow,
    connections: super::types::ConnectionStore,
    runtime: Arc<Runtime>,
    store: Rc<std::cell::RefCell<ConfigStore>>,
    transfer_windows: TransferWindows,
) {
    let weak = window.as_weak();
    window.on_open_transfer_window(move || {
        let Some(w) = weak.upgrade() else { return };
        let Some((active, session)) = active_session_or_hint(&w, &connections) else {
            return;
        };
        let local_dir = store.borrow().download_dir().to_string();
        match open_transfer_window(
            session,
            runtime.clone(),
            local_dir,
            transfer_windows.clone(),
        ) {
            Ok(()) => {}
            Err(err) => set_terminal_row(&w, &active, |row| {
                row.status =
                    format!("{}: {err:#}", t("打开文件传输失败", "Open transfer failed")).into();
            }),
        }
    });
}

pub(super) fn wire_transfer_window_callbacks(window: &TransferWindow, sftp: Rc<SftpHandle>) {
    {
        let weak = window.as_weak();
        window.on_local_navigate(move |target: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let current = w.get_local_path().to_string();
            refresh_transfer_local(&w, resolve_local_path(&current, target.as_str()));
        });
    }
    {
        let weak = window.as_weak();
        window.on_local_refresh(move || {
            let Some(w) = weak.upgrade() else { return };
            let current = w.get_local_path().to_string();
            refresh_transfer_local(&w, current);
        });
    }
    {
        let weak = window.as_weak();
        let sftp = sftp.clone();
        window.on_upload_local(move |local: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let remote_dir = w.get_remote_path().to_string();
            w.set_remote_status(format!("{} {}", t("上传", "Uploading"), local.as_str()).into());
            sftp.upload(local.to_string(), remote_dir);
        });
    }
    {
        let weak = window.as_weak();
        let sftp = sftp.clone();
        window.on_remote_navigate(move |target: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let current = w.get_remote_path().to_string();
            let target = if target.as_str() == ".." {
                parent_path(&current)
            } else {
                target.to_string()
            };
            w.set_remote_loading(true);
            w.set_remote_status(format!("{} {}...", t("加载", "Loading"), target).into());
            sftp.list_dir(target);
        });
    }
    {
        let weak = window.as_weak();
        let sftp = sftp.clone();
        window.on_remote_refresh(move || {
            let Some(w) = weak.upgrade() else { return };
            let path = w.get_remote_path().to_string();
            w.set_remote_loading(true);
            w.set_remote_status(format!("{} {}...", t("加载", "Loading"), path).into());
            sftp.list_dir(path);
        });
    }
    {
        let weak = window.as_weak();
        let sftp = sftp.clone();
        window.on_download_remote(move |remote: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let local_dir = w.get_local_path().to_string();
            w.set_remote_status(format!("{} {}", t("下载", "Downloading"), remote.as_str()).into());
            sftp.download(remote.to_string(), local_dir);
        });
    }
    {
        let weak = window.as_weak();
        window.on_close_window(move || {
            sftp.close();
            if let Some(w) = weak.upgrade() {
                let _ = w.hide();
            }
        });
    }
}

pub(super) fn refresh_transfer_local(window: &TransferWindow, path: impl AsRef<std::path::Path>) {
    match list_local_dir(path) {
        Ok((path, entries)) => {
            window.set_local_path(path.into());
            window.set_local_entries(local_entries_model(entries));
            window.set_local_status(t("本地目录就绪", "Local directory ready").into());
        }
        Err(err) => {
            window.set_local_status(
                format!(
                    "{}: {err:#}",
                    t("读取本地目录失败", "Read local directory failed")
                )
                .into(),
            );
        }
    }
}

pub(super) fn local_entries_model(entries: Vec<LocalFileEntry>) -> ModelRc<SftpEntry> {
    let rows: Vec<SftpEntry> = entries
        .into_iter()
        .map(|entry| SftpEntry {
            name: entry.name.into(),
            full_path: entry.full_path.into(),
            is_dir: entry.is_dir,
            size: if entry.is_dir {
                "".into()
            } else {
                format_size(entry.size).into()
            },
            modified: if entry.modified == 0 {
                "".into()
            } else {
                format_mtime(entry.modified.min(u32::MAX as u64) as u32).into()
            },
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn remote_entries_model(entries: Vec<RemoteEntry>) -> ModelRc<SftpEntry> {
    let rows: Vec<SftpEntry> = entries
        .into_iter()
        .map(|entry| SftpEntry {
            name: entry.name.into(),
            full_path: entry.full_path.into(),
            is_dir: entry.is_dir,
            size: if entry.is_dir {
                "".into()
            } else {
                format_size(entry.size).into()
            },
            modified: format_mtime(entry.modified).into(),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn spawn_transfer_sftp_event_pump(
    weak: slint::Weak<TransferWindow>,
    events: UnboundedReceiver<SessionEvent>,
) {
    std::thread::spawn(move || {
        let mut rx = events;
        while let Some(event) = rx.blocking_recv() {
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = weak.upgrade() else { return };
                apply_transfer_event_to_window(&window, event);
            });
        }
    });
}

pub(super) fn apply_transfer_event_to_window(window: &TransferWindow, event: SessionEvent) {
    match event {
        SessionEvent::SftpEntries { path, entries } => {
            window.set_remote_path(path.into());
            window.set_remote_entries(remote_entries_model(entries));
            window.set_remote_loading(false);
        }
        SessionEvent::SftpStatus(msg) => {
            window.set_remote_status(msg.into());
            window.set_remote_loading(false);
        }
        SessionEvent::SftpTransfer {
            name,
            is_upload,
            transferred,
            total,
            state,
            ..
        } => {
            let detail = if state == 1 {
                t("完成", "Done").to_string()
            } else if state == 2 {
                t("失败", "Failed").to_string()
            } else if total > 0 {
                format!("{}/{}", format_size(transferred), format_size(total))
            } else {
                format_size(transferred)
            };
            let action = if is_upload {
                t("上传", "Uploading")
            } else {
                t("下载", "Downloading")
            };
            window.set_remote_status(format!("{action} {name}: {detail}").into());
            if state == 1 && !is_upload {
                let local = window.get_local_path().to_string();
                refresh_transfer_local(window, local);
            }
        }
        _ => {}
    }
}
