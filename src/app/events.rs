use std::sync::Arc;

use slint::{Model, ModelRc, VecModel};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::i18n::t;
use crate::ssh::{format_mtime, format_size, SessionEvent};
use crate::terminal::engine::TerminalEngine;

use super::models::term_spans_model;
use super::sidebar::{push_ring, refresh_sidebar, selected_iface};
use super::terminal_render::{compute_find_matches, selection_rects};
use super::tunnels::refresh_tunnel_panel;
use super::types::{
    ConnectionStore, LocalSnap, NetHist, SftpHandles, SftpManualNav, TabStatuses, TermBuffers,
    TunnelStore,
};
use super::{
    AppWindow, SftpEntry, SftpTreeNode, TabInfo, TermMatch, TermSpan, TerminalState, TransferInfo,
};

pub(super) fn upsert_transfer_record(
    transfers: ModelRc<TransferInfo>,
    id: String,
    name: String,
    is_upload: bool,
    transferred: u64,
    total: u64,
    state: u8,
) {
    let detail = match state {
        2 => t("失败", "Failed").to_string(),
        1 => t("已完成", "Done").to_string(),
        _ => {
            if total > 0 {
                format!("{}/{}", format_size(transferred), format_size(total))
            } else {
                format_size(transferred)
            }
        }
    };
    let percent = if state == 1 {
        1.0
    } else if total > 0 {
        (transferred as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let rec = TransferInfo {
        id: id.clone().into(),
        name: name.into(),
        detail: detail.into(),
        percent,
        state: state as i32,
        is_upload,
    };
    if let Some(model) = transfers.as_any().downcast_ref::<VecModel<TransferInfo>>() {
        let mut found = None;
        for i in 0..model.row_count() {
            if let Some(row) = model.row_data(i) {
                if row.id.as_str() == id.as_str() {
                    found = Some(i);
                    break;
                }
            }
        }
        match found {
            Some(i) => model.set_row_data(i, rec),
            None => model.insert(0, rec),
        }
    }
}

pub(super) fn spawn_shell_event_pump(
    weak: slint::Weak<AppWindow>,
    tab_id: String,
    events: UnboundedReceiver<SessionEvent>,
    generation: u64,
    connections: ConnectionStore,
    bufs: TermBuffers,
    sftp_handles: SftpHandles,
    sftp_manual_nav: SftpManualNav,
    runtime: Arc<Runtime>,
    tab_statuses: TabStatuses,
    local_snap: LocalSnap,
    local_net_hist: NetHist,
    tunnels: TunnelStore,
) {
    std::thread::spawn(move || {
        let mut shell_rx = events;
        let mut cwd_debounce: Option<tokio::task::JoinHandle<()>> = None;
        loop {
            match shell_rx.blocking_recv() {
                None => break,
                Some(shell_evt) => {
                    if !connections
                        .lock()
                        .unwrap()
                        .is_current_generation(&tab_id, generation)
                    {
                        break;
                    }
                    let tunnel_session = match &shell_evt {
                        SessionEvent::Connected | SessionEvent::Closed(_) => {
                            connections.lock().unwrap().session(&tab_id)
                        }
                        _ => None,
                    };
                    match &shell_evt {
                        SessionEvent::Connected => {
                            connections
                                .lock()
                                .unwrap()
                                .mark_connected(&tab_id, generation);
                            if let Some(session) = tunnel_session.clone() {
                                tunnels
                                    .lock()
                                    .unwrap()
                                    .start_enabled_for_session(runtime.handle(), session);
                            }
                        }
                        SessionEvent::Closed(reason) => {
                            if let Some(session) = tunnel_session {
                                tunnels.lock().unwrap().stop_for_session(&session.id);
                            }
                            connections.lock().unwrap().mark_closed(
                                &tab_id,
                                generation,
                                reason.clone(),
                            );
                        }
                        _ => {}
                    }

                    if let SessionEvent::CwdChanged(ref cwd) = shell_evt {
                        let is_manual = sftp_manual_nav
                            .lock()
                            .ok()
                            .and_then(|m| m.get(&tab_id).copied())
                            .unwrap_or(false);
                        if !is_manual {
                            if let Some(prev) = cwd_debounce.take() {
                                prev.abort();
                            }
                            let cwd = cwd.clone();
                            let sftp_h = sftp_handles.clone();
                            let tid = tab_id.clone();
                            cwd_debounce = Some(runtime.spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                if let Ok(handles) = sftp_h.lock() {
                                    if let Some(h) = handles.get(&tid) {
                                        h.list_dir(cwd);
                                    }
                                }
                            }));
                        }
                    }
                    let weak_evt = weak.clone();
                    let tid = tab_id.clone();
                    let bufs_evt = bufs.clone();
                    let st_evt = tab_statuses.clone();
                    let lc_evt = local_snap.clone();
                    let nh_evt = local_net_hist.clone();
                    let conn_evt = connections.clone();
                    let tunnels_evt = tunnels.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak_evt.upgrade() {
                            apply_session_event_to_window(
                                &win, &tid, shell_evt, &bufs_evt, &st_evt, &lc_evt, &nh_evt,
                            );
                            refresh_tunnel_panel(&win, &conn_evt, &tunnels_evt);
                        }
                    });
                }
            }
        }
    });
}

pub(super) fn spawn_sftp_event_pump(
    weak: slint::Weak<AppWindow>,
    tab_id: String,
    events: UnboundedReceiver<SessionEvent>,
    bufs: TermBuffers,
    tab_statuses: TabStatuses,
    local_snap: LocalSnap,
    local_net_hist: NetHist,
) {
    std::thread::spawn(move || {
        let mut sftp_rx = events;
        loop {
            match sftp_rx.blocking_recv() {
                None => break,
                Some(sftp_evt) => {
                    let weak_s = weak.clone();
                    let tid = tab_id.clone();
                    let bufs_s = bufs.clone();
                    let st_s = tab_statuses.clone();
                    let lc_s = local_snap.clone();
                    let nh_s = local_net_hist.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak_s.upgrade() {
                            apply_session_event_to_window(
                                &win, &tid, sftp_evt, &bufs_s, &st_s, &lc_s, &nh_s,
                            );
                        }
                    });
                }
            }
        }
    });
}

/// Apply a session event to the live UI models. Must be called on the Slint
/// event loop thread.
pub(super) fn apply_session_event_to_window(
    win: &AppWindow,
    tab_id: &str,
    event: SessionEvent,
    bufs: &TermBuffers,
    statuses: &TabStatuses,
    local: &LocalSnap,
    local_net_hist: &NetHist,
) {
    let tabs_rc = win.get_tabs();
    let terminals_rc = win.get_terminals();
    // `ModelRc::as_any` lets us downcast to the concrete `VecModel<T>`.
    let tabs = tabs_rc
        .as_any()
        .downcast_ref::<VecModel<TabInfo>>()
        .expect("tabs model must be a VecModel");
    let terminals = terminals_rc
        .as_any()
        .downcast_ref::<VecModel<TerminalState>>()
        .expect("terminals model must be a VecModel");

    let update_terminal = |mutator: &dyn Fn(&mut TerminalState)| {
        for i in 0..terminals.row_count() {
            if let Some(mut row) = terminals.row_data(i) {
                if row.id.as_str() == tab_id {
                    mutator(&mut row);
                    terminals.set_row_data(i, row);
                    break;
                }
            }
        }
    };
    let update_tab = |mutator: &dyn Fn(&mut TabInfo)| {
        for i in 0..tabs.row_count() {
            if let Some(mut row) = tabs.row_data(i) {
                if row.id.as_str() == tab_id {
                    mutator(&mut row);
                    tabs.set_row_data(i, row);
                    break;
                }
            }
        }
    };

    match event {
        SessionEvent::Status(status) => {
            update_terminal(&|t| t.status = status.clone().into());
        }
        SessionEvent::Output(chunk) => {
            // Feed raw bytes into the vt100 parser. vt100 correctly handles
            // cursor movement, \r + line-redraw (readline), \x1b[K (erase to
            // EOL), alternate-screen switching, and all VT100/xterm sequences.
            // We then split the rendered screen at cursor_position() so Slint
            // can insert the blinking "█" at the exact cursor cell.
            let built = {
                let mut map = bufs.lock().unwrap();
                if let Some(buf) = map.get_mut(tab_id) {
                    // Capture scrolled-off lines into history, then render the
                    // current view (live or scrolled-back).
                    TerminalEngine::ingest(buf, chunk.as_bytes());
                    let cols = buf.parser.screen().size().1;
                    let b = TerminalEngine::render(buf);
                    let displayed_text = buf.displayed_text();
                    let matches = compute_find_matches(&displayed_text, &buf.find_query);
                    let sel = match buf.sel {
                        Some((sr, sc, er, ec)) => selection_rects(sr, sc, er, ec, cols),
                        None => Vec::new(),
                    };
                    Some((b, matches, sel))
                } else {
                    None
                }
            };
            if let Some((b, matches, sel)) = built {
                let spans_model: ModelRc<TermSpan> = term_spans_model(b.spans);
                let matches_model: ModelRc<TermMatch> =
                    ModelRc::from(std::rc::Rc::new(VecModel::from(matches)));
                let sel_model: ModelRc<TermMatch> =
                    ModelRc::from(std::rc::Rc::new(VecModel::from(sel)));
                let (cur_row, cur_col, rows_used, is_alt, mouse_reporting) = (
                    b.cursor_row,
                    b.cursor_col,
                    b.rows_used,
                    b.is_alt,
                    b.mouse_reporting,
                );
                update_terminal(&|t| {
                    t.spans = spans_model.clone();
                    t.cursor_row = cur_row;
                    t.cursor_col = cur_col;
                    t.rows_used = rows_used;
                    t.is_alt_screen = is_alt;
                    t.mouse_reporting = mouse_reporting;
                    t.find_matches = matches_model.clone();
                    t.selection = sel_model.clone();
                });
            }
        }
        SessionEvent::Connected => {
            update_tab(&|t| t.connected = true);
            update_terminal(&|t| t.status = crate::i18n::t("已连接", "Connected").into());
            if let Some(st) = statuses.lock().unwrap().get_mut(tab_id) {
                st.state = 1;
            }
            if win.get_active_tab_id().as_str() == tab_id {
                refresh_sidebar(win, statuses, local, local_net_hist);
            }
        }
        SessionEvent::Closed(reason) => {
            update_tab(&|t| t.connected = false);
            update_terminal(&|t| {
                t.status = format!("{} — {reason}", crate::i18n::t("已断开", "Disconnected")).into()
            });
            if let Some(st) = statuses.lock().unwrap().get_mut(tab_id) {
                st.state = 2;
            }
            if win.get_active_tab_id().as_str() == tab_id {
                refresh_sidebar(win, statuses, local, local_net_hist);
            }
        }
        SessionEvent::ResourceStats {
            cpu_percent,
            mem_used_kib,
            mem_total_kib,
            swap_used_kib,
            swap_total_kib,
            net,
            disks,
        } => {
            if let Some(st) = statuses.lock().unwrap().get_mut(tab_id) {
                st.cpu = cpu_percent;
                st.mem_used_kib = mem_used_kib;
                st.mem_total_kib = mem_total_kib;
                st.swap_used_kib = swap_used_kib;
                st.swap_total_kib = swap_total_kib;
                st.net = net;
                st.disks = disks;
                // A sample means the channel is alive → treat as connected.
                if st.state != 1 {
                    st.state = 1;
                }
                // Append the selected interface's total rate to its sparkline.
                let (_, rx, tx) = selected_iface(st);
                push_ring(&mut st.net_hist, (rx + tx) as f32);
            }
            if win.get_active_tab_id().as_str() == tab_id {
                refresh_sidebar(win, statuses, local, local_net_hist);
            }
        }

        // --- SFTP events ---------------------------------------------------
        SessionEvent::CwdChanged(path) => {
            // Just update the displayed path; the pump thread already sent
            // SftpCommand::ListDir so a SftpEntries event is inbound.
            update_terminal(&|t| {
                t.sftp_path = path.clone().into();
                t.sftp_loading = true;
            });
        }
        SessionEvent::SftpEntries { path, entries } => {
            let slint_entries: Vec<SftpEntry> = entries
                .iter()
                .map(|e| SftpEntry {
                    name: e.name.clone().into(),
                    full_path: e.full_path.clone().into(),
                    is_dir: e.is_dir,
                    size: if e.is_dir {
                        "".into()
                    } else {
                        format_size(e.size).into()
                    },
                    modified: format_mtime(e.modified).into(),
                })
                .collect();
            let model = ModelRc::from(std::rc::Rc::new(VecModel::from(slint_entries)));
            update_terminal(&|t| {
                t.sftp_path = path.clone().into();
                t.sftp_entries = model.clone();
                t.sftp_loading = false;
            });
        }
        SessionEvent::SftpStatus(msg) => {
            update_terminal(&|t| t.sftp_status = msg.clone().into());
        }
        SessionEvent::SftpTreeUpdate(nodes) => {
            let slint_nodes: Vec<SftpTreeNode> = nodes
                .iter()
                .map(|n| SftpTreeNode {
                    path: n.path.clone().into(),
                    name: n.name.clone().into(),
                    depth: n.depth as i32,
                    expanded: n.expanded,
                    has_children: n.has_children,
                })
                .collect();
            let model = ModelRc::from(std::rc::Rc::new(VecModel::from(slint_nodes)));
            update_terminal(&|t| t.sftp_tree_nodes = model.clone());
        }
        SessionEvent::SftpTransfer {
            id,
            name,
            is_upload,
            transferred,
            total,
            state,
            msg: _,
        } => {
            upsert_transfer_record(
                win.get_transfers(),
                id,
                name,
                is_upload,
                transferred,
                total,
                state,
            );
        }
    }
}
