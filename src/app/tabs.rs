use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, Model, SharedString, VecModel};
use tokio::runtime::Runtime;

use crate::i18n::t;
use crate::sftp::spawn_sftp;
use crate::ssh::SessionEvent;

use super::events::{spawn_sftp_event_pump, spawn_shell_event_pump};
use super::models::{active_session_or_hint, set_terminal_row};
use super::sidebar::refresh_sidebar;
use super::tunnels::refresh_tunnel_panel;
use super::types::{
    ConnectionStore, LocalSnap, NetHist, SftpHandles, SftpManualNav, TabStatuses, TermBuffers,
    TunnelStore,
};
use super::{AppWindow, TabInfo, TerminalState};

pub(super) fn wire_connection_toolbar_callbacks(
    window: &AppWindow,
    tabs_model: Rc<VecModel<TabInfo>>,
    connections: ConnectionStore,
    bufs: TermBuffers,
    runtime: Arc<Runtime>,
    last_term_size: Arc<Mutex<(u32, u32)>>,
    sftp_handles: SftpHandles,
    sftp_manual_nav: SftpManualNav,
    tab_statuses: TabStatuses,
    local_snap: LocalSnap,
    local_net_hist: NetHist,
    tunnels: TunnelStore,
) {
    {
        let weak = window.as_weak();
        let tabs_model = tabs_model.clone();
        let connections = connections.clone();
        let sftp_handles = sftp_handles.clone();
        let tunnels = tunnels.clone();
        let tab_statuses = tab_statuses.clone();
        let local_snap = local_snap.clone();
        let local_net_hist = local_net_hist.clone();
        window.on_disconnect_active_tab(move || {
            let Some(w) = weak.upgrade() else { return };
            let Some((active, session)) = active_session_or_hint(&w, &connections) else {
                return;
            };

            tunnels.lock().unwrap().stop_for_session(&session.id);
            connections.lock().unwrap().disconnect(&active);
            if let Some(sftp) = sftp_handles.lock().unwrap().remove(&active) {
                sftp.close();
            }
            if let Some(st) = tab_statuses.lock().unwrap().get_mut(&active) {
                st.state = 2;
            }
            for i in 0..tabs_model.row_count() {
                if let Some(mut row) = tabs_model.row_data(i) {
                    if row.id.as_str() == active {
                        row.connected = false;
                        tabs_model.set_row_data(i, row);
                        break;
                    }
                }
            }
            set_terminal_row(&w, &active, |row| {
                row.status = t("已断开", "Disconnected").into();
            });
            refresh_sidebar(&w, &tab_statuses, &local_snap, &local_net_hist);
            refresh_tunnel_panel(&w, &connections, &tunnels);
        });
    }

    {
        let weak = window.as_weak();
        let connections = connections.clone();
        let bufs = bufs.clone();
        let runtime = runtime.clone();
        let last_term_size = last_term_size.clone();
        let sftp_handles = sftp_handles.clone();
        let sftp_manual_nav = sftp_manual_nav.clone();
        let tab_statuses = tab_statuses.clone();
        let local_snap = local_snap.clone();
        let local_net_hist = local_net_hist.clone();
        let tabs_model = tabs_model.clone();
        let tunnels = tunnels.clone();
        window.on_reconnect_active_tab(move || {
            let Some(w) = weak.upgrade() else { return };
            let Some((active, session)) = active_session_or_hint(&w, &connections) else {
                return;
            };
            tunnels.lock().unwrap().stop_for_session(&session.id);
            if let Some(sftp) = sftp_handles.lock().unwrap().remove(&active) {
                sftp.close();
            }
            sftp_manual_nav
                .lock()
                .unwrap()
                .insert(active.clone(), false);
            let (initial_cols, initial_rows) = *last_term_size.lock().unwrap();
            let launch = {
                let mut manager = connections.lock().unwrap();
                match manager.reconnect(runtime.handle(), &active, initial_cols, initial_rows) {
                    Ok(launch) => launch,
                    Err(err) => {
                        set_terminal_row(&w, &active, |row| {
                            row.status =
                                format!("{}: {err:#}", t("重连失败", "Reconnect failed")).into();
                        });
                        return;
                    }
                }
            };
            for i in 0..tabs_model.row_count() {
                if let Some(mut row) = tabs_model.row_data(i) {
                    if row.id.as_str() == active {
                        row.connected = false;
                        tabs_model.set_row_data(i, row);
                        break;
                    }
                }
            }
            if let Some(st) = tab_statuses.lock().unwrap().get_mut(&active) {
                st.state = 0;
            }
            set_terminal_row(&w, &active, |row| {
                row.status = t("重连中...", "Reconnecting...").into();
                row.sftp_status = t("SFTP 连接中...", "SFTP connecting...").into();
                row.sftp_loading = true;
            });
            refresh_sidebar(&w, &tab_statuses, &local_snap, &local_net_hist);
            refresh_tunnel_panel(&w, &connections, &tunnels);

            let (sftp_tx, sftp_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
            let sftp_handle = spawn_sftp(runtime.handle(), session, sftp_tx);
            sftp_handles
                .lock()
                .unwrap()
                .insert(active.clone(), sftp_handle);
            spawn_shell_event_pump(
                weak.clone(),
                active.clone(),
                launch.events,
                launch.generation,
                connections.clone(),
                bufs.clone(),
                sftp_handles.clone(),
                sftp_manual_nav.clone(),
                runtime.clone(),
                tab_statuses.clone(),
                local_snap.clone(),
                local_net_hist.clone(),
                tunnels.clone(),
            );
            spawn_sftp_event_pump(
                weak.clone(),
                active,
                sftp_rx,
                bufs.clone(),
                tab_statuses.clone(),
                local_snap.clone(),
                local_net_hist.clone(),
            );
        });
    }
}

pub(super) fn wire_tab_callbacks(
    window: &AppWindow,
    tabs_model: Rc<VecModel<TabInfo>>,
    terminals_model: Rc<VecModel<TerminalState>>,
    connections: ConnectionStore,
    bufs: TermBuffers,
    sftp_handles: SftpHandles,
    sftp_manual_nav: SftpManualNav,
    tunnels: TunnelStore,
) {
    {
        window.on_tab_selected(move |_id: SharedString| {
            // No-op: AppWindow.active-tab-id is updated inline in the .slint.
        });
    }

    {
        let weak = window.as_weak();
        let tabs_model = tabs_model.clone();
        let terminals_model = terminals_model.clone();
        let connections = connections.clone();
        let bufs = bufs.clone();
        let sftp_handles = sftp_handles.clone();
        let sftp_manual_nav = sftp_manual_nav.clone();
        let tunnels = tunnels.clone();
        window.on_tab_closed(move |id: SharedString| {
            let id = id.to_string();
            if id == "welcome" {
                return;
            }
            if let Some(session) = connections.lock().unwrap().session(&id) {
                tunnels.lock().unwrap().stop_for_session(&session.id);
            }
            connections.lock().unwrap().remove(&id);
            if let Some(sftp) = sftp_handles.lock().unwrap().remove(&id) {
                sftp.close();
            }
            sftp_manual_nav.lock().unwrap().remove(&id);
            bufs.lock().unwrap().remove(&id);

            let mut idx = None;
            for i in 0..tabs_model.row_count() {
                if tabs_model
                    .row_data(i)
                    .map(|r| r.id.as_str() == id)
                    .unwrap_or(false)
                {
                    idx = Some(i);
                    break;
                }
            }
            if let Some(i) = idx {
                tabs_model.remove(i);
            }
            let mut tidx = None;
            for i in 0..terminals_model.row_count() {
                if terminals_model
                    .row_data(i)
                    .map(|r| r.id.as_str() == id)
                    .unwrap_or(false)
                {
                    tidx = Some(i);
                    break;
                }
            }
            if let Some(i) = tidx {
                terminals_model.remove(i);
            }

            if let Some(w) = weak.upgrade() {
                if w.get_active_tab_id().as_str() == id {
                    w.set_active_tab_id("welcome".into());
                }
                refresh_tunnel_panel(&w, &connections, &tunnels);
            }
        });
    }

    {
        let weak = window.as_weak();
        window.on_new_tab_clicked(move || {
            if let Some(w) = weak.upgrade() {
                w.set_active_tab_id("welcome".into());
            }
        });
    }
}
