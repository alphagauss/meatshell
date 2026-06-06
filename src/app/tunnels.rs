use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, ModelRc, VecModel};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::i18n::t;
use crate::tunnel::{TunnelEvent, TunnelView};

use super::models::set_terminal_row;
use super::types::{ConnectionStore, TunnelStore};
use super::{AppWindow, TunnelRuleInfo};

pub(super) fn spawn_tunnel_event_pump(
    weak: slint::Weak<AppWindow>,
    events: UnboundedReceiver<TunnelEvent>,
    connections: ConnectionStore,
    tunnels: TunnelStore,
) {
    std::thread::spawn(move || {
        let mut rx = events;
        while let Some(event) = rx.blocking_recv() {
            tunnels.lock().unwrap().apply_event(&event);
            let weak_evt = weak.clone();
            let connections_evt = connections.clone();
            let tunnels_evt = tunnels.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = weak_evt.upgrade() {
                    refresh_tunnel_panel(&win, &connections_evt, &tunnels_evt);
                }
            });
        }
    });
}

pub(super) fn wire_tunnel_callbacks(
    window: &AppWindow,
    connections: ConnectionStore,
    tunnels: TunnelStore,
    runtime: Arc<Runtime>,
) {
    {
        let weak = window.as_weak();
        let connections = connections.clone();
        let tunnels = tunnels.clone();
        window.on_tunnel_add_rule(move || {
            let Some(w) = weak.upgrade() else { return };
            let active = w.get_active_tab_id().to_string();
            let Some(session) = connections.lock().unwrap().session(&active) else {
                set_terminal_row(&w, &active, |row| {
                    row.status = t("没有可用的隧道会话", "No session available for tunnels").into();
                });
                return;
            };
            let result = tunnels.lock().unwrap().add_rule(&session.id);
            if let Err(err) = result {
                set_terminal_row(&w, &active, |row| {
                    row.status =
                        format!("{}: {err:#}", t("新增隧道失败", "Add tunnel failed")).into();
                });
            }
            refresh_tunnel_panel(&w, &connections, &tunnels);
        });
    }

    {
        let weak = window.as_weak();
        let connections = connections.clone();
        let tunnels = tunnels.clone();
        let runtime = runtime.clone();
        window.on_tunnel_update_rule(
            move |id, name, local_host, local_port, remote_host, remote_port| {
                let Some(w) = weak.upgrade() else { return };
                let active = w.get_active_tab_id().to_string();
                let result = tunnels.lock().unwrap().update_rule(
                    &id.to_string(),
                    name.to_string(),
                    local_host.to_string(),
                    local_port.to_string(),
                    remote_host.to_string(),
                    remote_port.to_string(),
                );
                match result {
                    Ok(Some(rule)) => {
                        if rule.enabled {
                            if let Some(session) = connections.lock().unwrap().session(&active) {
                                tunnels
                                    .lock()
                                    .unwrap()
                                    .start_rule(runtime.handle(), session, rule);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        set_terminal_row(&w, &active, |row| {
                            row.status =
                                format!("{}: {err:#}", t("保存隧道失败", "Save tunnel failed"))
                                    .into();
                        });
                    }
                }
                refresh_tunnel_panel(&w, &connections, &tunnels);
            },
        );
    }

    {
        let weak = window.as_weak();
        let connections = connections.clone();
        let tunnels = tunnels.clone();
        let runtime = runtime.clone();
        window.on_tunnel_toggle_rule(move |id, enabled| {
            let Some(w) = weak.upgrade() else { return };
            let active = w.get_active_tab_id().to_string();
            let result = tunnels
                .lock()
                .unwrap()
                .set_enabled(&id.to_string(), enabled);
            match result {
                Ok(Some(rule)) => {
                    if enabled {
                        if let Some(session) = connections.lock().unwrap().session(&active) {
                            tunnels
                                .lock()
                                .unwrap()
                                .start_rule(runtime.handle(), session, rule);
                        }
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    set_terminal_row(&w, &active, |row| {
                        row.status =
                            format!("{}: {err:#}", t("更新隧道失败", "Update tunnel failed"))
                                .into();
                    });
                }
            }
            refresh_tunnel_panel(&w, &connections, &tunnels);
        });
    }

    {
        let weak = window.as_weak();
        let connections = connections.clone();
        let tunnels = tunnels.clone();
        window.on_tunnel_delete_rule(move |id| {
            let Some(w) = weak.upgrade() else { return };
            let active = w.get_active_tab_id().to_string();
            if let Err(err) = tunnels.lock().unwrap().delete_rule(&id.to_string()) {
                set_terminal_row(&w, &active, |row| {
                    row.status =
                        format!("{}: {err:#}", t("删除隧道失败", "Delete tunnel failed")).into();
                });
            }
            refresh_tunnel_panel(&w, &connections, &tunnels);
        });
    }
}

pub(super) fn refresh_tunnel_panel(
    win: &AppWindow,
    connections: &ConnectionStore,
    tunnels: &TunnelStore,
) {
    let active = win.get_active_tab_id().to_string();
    let views = connections
        .lock()
        .unwrap()
        .session(&active)
        .map(|session| tunnels.lock().unwrap().views_for_session(&session.id))
        .unwrap_or_default();
    let rows: Vec<TunnelRuleInfo> = views.into_iter().map(tunnel_view_to_info).collect();
    win.set_tunnel_rules(ModelRc::from(Rc::new(VecModel::from(rows))));
}

pub(super) fn tunnel_view_to_info(view: TunnelView) -> TunnelRuleInfo {
    TunnelRuleInfo {
        id: view.id.into(),
        name: view.name.into(),
        enabled: view.enabled,
        local_host: view.local_host.into(),
        local_port: view.local_port.into(),
        remote_host: view.remote_host.into(),
        remote_port: view.remote_port.into(),
        status: view.status_text.into(),
        status_kind: view.status_kind,
    }
}
