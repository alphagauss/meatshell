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
use super::types::{TransferRemoteTab, TransferWindowState, TransferWindows};
use super::{SftpEntry, TransferRemoteTabInfo, TransferWindow};

pub(super) fn open_transfer_window(
    session: Session,
    runtime: Arc<Runtime>,
    preferred_local_dir: String,
    transfer_windows: TransferWindows,
) -> Result<()> {
    if let Some(existing) = transfer_windows.borrow().as_ref() {
        remember_active_remote_path(&existing.window, &existing.remote_tabs);
        ensure_transfer_remote_tab(
            &existing.window,
            existing.remote_tabs.clone(),
            session,
            runtime.clone(),
        );
        existing
            .window
            .show()
            .context("failed to show transfer window")?;
        return Ok(());
    }

    let window = TransferWindow::new().context("failed to build transfer window")?;
    window.set_session_title(t("文件传输", "File transfer").into());
    window.set_active_remote_tab_id("".into());
    window.set_remote_tabs(ModelRc::from(Rc::new(
        VecModel::<TransferRemoteTabInfo>::default(),
    )));
    window.set_remote_path("/".into());
    window.set_remote_status(t("SFTP 连接中...", "SFTP connecting...").into());
    window.set_remote_loading(true);
    window.set_remote_entries(ModelRc::from(Rc::new(VecModel::<SftpEntry>::default())));
    refresh_transfer_local(&window, default_local_dir(&preferred_local_dir));

    let remote_tabs = Rc::new(std::cell::RefCell::new(Vec::new()));
    wire_transfer_window_callbacks(&window, remote_tabs.clone(), runtime.clone());
    ensure_transfer_remote_tab(&window, remote_tabs.clone(), session, runtime.clone());
    window.window().on_close_requested({
        let weak = window.as_weak();
        move || {
            if let Some(w) = weak.upgrade() {
                let _ = w.hide();
            }
            slint::CloseRequestResponse::HideWindow
        }
    });
    window.show().context("failed to show transfer window")?;
    *transfer_windows.borrow_mut() = Some(TransferWindowState {
        window,
        remote_tabs,
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

fn transfer_tab_title(session: &Session) -> String {
    if session.name.trim().is_empty() {
        format!("{}@{}", session.user, session.host)
    } else {
        session.name.clone()
    }
}

fn empty_entries_model() -> ModelRc<SftpEntry> {
    ModelRc::from(Rc::new(VecModel::<SftpEntry>::default()))
}

fn sync_transfer_remote_tabs(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
) {
    let rows: Vec<TransferRemoteTabInfo> = remote_tabs
        .borrow()
        .iter()
        .map(|tab| TransferRemoteTabInfo {
            id: tab.id.clone().into(),
            title: tab.title.clone().into(),
            connected: tab.connected,
        })
        .collect();
    window.set_remote_tabs(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn remember_active_remote_path(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
) {
    let active = window.get_active_remote_tab_id().to_string();
    if active.is_empty() {
        return;
    }
    let path = window.get_remote_path().to_string();
    if path.is_empty() {
        return;
    }
    if let Some(tab) = remote_tabs
        .borrow_mut()
        .iter_mut()
        .find(|tab| tab.id == active)
    {
        tab.remote_path = path;
    }
}

fn set_active_remote_path(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
    path: String,
) {
    let active = window.get_active_remote_tab_id().to_string();
    if let Some(tab) = remote_tabs
        .borrow_mut()
        .iter_mut()
        .find(|tab| tab.id == active)
    {
        tab.remote_path = path;
    }
}

fn active_transfer_sftp(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
) -> Option<Rc<SftpHandle>> {
    let active = window.get_active_remote_tab_id().to_string();
    remote_tabs
        .borrow()
        .iter()
        .find(|tab| tab.id == active)
        .map(|tab| tab.sftp.clone())
}

fn show_transfer_tab(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
    reload: bool,
) {
    let active = window.get_active_remote_tab_id().to_string();
    let tab = remote_tabs
        .borrow()
        .iter()
        .find(|tab| tab.id == active)
        .map(|tab| (tab.remote_path.clone(), tab.sftp.clone()));
    let Some((path, sftp)) = tab else {
        window.set_remote_path("".into());
        window.set_remote_entries(empty_entries_model());
        window.set_remote_status(t("没有远程标签页", "No remote tab").into());
        window.set_remote_loading(false);
        return;
    };

    window.set_remote_path(path.clone().into());
    window.set_remote_entries(empty_entries_model());
    window.set_remote_loading(true);
    if reload {
        window.set_remote_status(format!("{} {}...", t("加载", "Loading"), path).into());
        sftp.list_dir(path);
    } else {
        window.set_remote_status(t("SFTP 连接中...", "SFTP connecting...").into());
    }
}

fn ensure_transfer_remote_tab(
    window: &TransferWindow,
    remote_tabs: Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
    session: Session,
    runtime: Arc<Runtime>,
) {
    let tab_id = session.id.clone();
    if remote_tabs.borrow().iter().any(|tab| tab.id == tab_id) {
        window.set_active_remote_tab_id(tab_id.into());
        sync_transfer_remote_tabs(window, &remote_tabs);
        show_transfer_tab(window, &remote_tabs, true);
        return;
    }

    let (sftp_tx, sftp_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
    let sftp = Rc::new(spawn_sftp(runtime.handle(), session.clone(), sftp_tx));
    remote_tabs.borrow_mut().push(TransferRemoteTab {
        id: tab_id.clone(),
        title: transfer_tab_title(&session),
        session,
        sftp,
        remote_path: "/".to_string(),
        connected: true,
    });
    spawn_transfer_sftp_event_pump(window.as_weak(), tab_id.clone(), sftp_rx);
    window.set_active_remote_tab_id(tab_id.into());
    sync_transfer_remote_tabs(window, &remote_tabs);
    show_transfer_tab(window, &remote_tabs, false);
}

fn close_transfer_tab(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
    tab_id: String,
) {
    remember_active_remote_path(window, remote_tabs);
    let active = window.get_active_remote_tab_id().to_string();
    let mut tabs = remote_tabs.borrow_mut();
    let Some(pos) = tabs.iter().position(|tab| tab.id == tab_id) else {
        return;
    };
    let removed = tabs.remove(pos);
    let removed_active = active == removed.id;
    removed.sftp.close();
    let next_active = if removed_active {
        tabs.first().map(|tab| tab.id.clone())
    } else {
        None
    };
    let has_tabs = !tabs.is_empty();
    drop(tabs);

    sync_transfer_remote_tabs(window, remote_tabs);
    if !has_tabs {
        window.set_active_remote_tab_id("".into());
        show_transfer_tab(window, remote_tabs, false);
    } else if let Some(next_active) = next_active {
        window.set_active_remote_tab_id(next_active.into());
        show_transfer_tab(window, remote_tabs, true);
    }
}

fn reconnect_transfer_tab(
    window: &TransferWindow,
    remote_tabs: &Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
    runtime: Arc<Runtime>,
    tab_id: String,
) {
    remember_active_remote_path(window, remote_tabs);
    let active = window.get_active_remote_tab_id().to_string();
    let reconnect = {
        let mut tabs = remote_tabs.borrow_mut();
        let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        let path = if active == tab.id {
            window.get_remote_path().to_string()
        } else {
            tab.remote_path.clone()
        };
        let path = if path.is_empty() {
            "/".to_string()
        } else {
            path
        };
        tab.remote_path = path.clone();
        tab.sftp.close();

        let (sftp_tx, sftp_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
        let sftp = Rc::new(spawn_sftp(runtime.handle(), tab.session.clone(), sftp_tx));
        tab.sftp = sftp.clone();
        tab.connected = true;
        (tab.id.clone(), path, sftp, sftp_rx)
    };

    let (tab_id, path, sftp, sftp_rx) = reconnect;
    spawn_transfer_sftp_event_pump(window.as_weak(), tab_id.clone(), sftp_rx);
    sync_transfer_remote_tabs(window, remote_tabs);
    if active == tab_id {
        window.set_remote_path(path.clone().into());
        window.set_remote_entries(empty_entries_model());
        window.set_remote_loading(true);
        window.set_remote_status(t("SFTP 重连中...", "SFTP reconnecting...").into());
    }
    sftp.list_dir(path);
}

pub(super) fn wire_transfer_window_callbacks(
    window: &TransferWindow,
    remote_tabs: Rc<std::cell::RefCell<Vec<TransferRemoteTab>>>,
    runtime: Arc<Runtime>,
) {
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
        let remote_tabs = remote_tabs.clone();
        window.on_upload_local(move |local: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let Some(sftp) = active_transfer_sftp(&w, &remote_tabs) else {
                return;
            };
            let remote_dir = w.get_remote_path().to_string();
            remember_active_remote_path(&w, &remote_tabs);
            w.set_remote_status(format!("{} {}", t("上传", "Uploading"), local.as_str()).into());
            sftp.upload(local.to_string(), remote_dir);
        });
    }
    {
        let weak = window.as_weak();
        let remote_tabs = remote_tabs.clone();
        window.on_remote_navigate(move |target: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let Some(sftp) = active_transfer_sftp(&w, &remote_tabs) else {
                return;
            };
            let current = w.get_remote_path().to_string();
            let target = if target.as_str() == ".." {
                parent_path(&current)
            } else {
                target.to_string()
            };
            set_active_remote_path(&w, &remote_tabs, target.clone());
            w.set_remote_loading(true);
            w.set_remote_status(format!("{} {}...", t("加载", "Loading"), target).into());
            sftp.list_dir(target);
        });
    }
    {
        let weak = window.as_weak();
        let remote_tabs = remote_tabs.clone();
        window.on_remote_refresh(move || {
            let Some(w) = weak.upgrade() else { return };
            let Some(sftp) = active_transfer_sftp(&w, &remote_tabs) else {
                return;
            };
            let path = w.get_remote_path().to_string();
            set_active_remote_path(&w, &remote_tabs, path.clone());
            w.set_remote_loading(true);
            w.set_remote_status(format!("{} {}...", t("加载", "Loading"), path).into());
            sftp.list_dir(path);
        });
    }
    {
        let weak = window.as_weak();
        let remote_tabs = remote_tabs.clone();
        window.on_download_remote(move |remote: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let Some(sftp) = active_transfer_sftp(&w, &remote_tabs) else {
                return;
            };
            let local_dir = w.get_local_path().to_string();
            w.set_remote_status(format!("{} {}", t("下载", "Downloading"), remote.as_str()).into());
            sftp.download(remote.to_string(), local_dir);
        });
    }
    {
        let weak = window.as_weak();
        let remote_tabs = remote_tabs.clone();
        window.on_remote_tab_selected(move |tab_id: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            if w.get_active_remote_tab_id().as_str() == tab_id.as_str() {
                return;
            }
            remember_active_remote_path(&w, &remote_tabs);
            w.set_active_remote_tab_id(tab_id);
            sync_transfer_remote_tabs(&w, &remote_tabs);
            show_transfer_tab(&w, &remote_tabs, true);
        });
    }
    {
        let weak = window.as_weak();
        let remote_tabs = remote_tabs.clone();
        window.on_remote_tab_closed(move |tab_id: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            close_transfer_tab(&w, &remote_tabs, tab_id.to_string());
        });
    }
    {
        let weak = window.as_weak();
        let remote_tabs = remote_tabs.clone();
        let runtime = runtime.clone();
        window.on_remote_tab_reconnect(move |tab_id: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            reconnect_transfer_tab(&w, &remote_tabs, runtime.clone(), tab_id.to_string());
        });
    }
    {
        let weak = window.as_weak();
        window.on_close_window(move || {
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
    tab_id: String,
    events: UnboundedReceiver<SessionEvent>,
) {
    std::thread::spawn(move || {
        let mut rx = events;
        while let Some(event) = rx.blocking_recv() {
            let weak = weak.clone();
            let tab_id = tab_id.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = weak.upgrade() else { return };
                if window.get_active_remote_tab_id().as_str() != tab_id {
                    return;
                }
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
