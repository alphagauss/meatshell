use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::config::{AuthMethod, ConfigStore, Secret, Session};
use crate::i18n::t;
use crate::sftp::spawn_sftp;
use crate::ssh::SessionEvent;
use crate::terminal::engine::{TerminalEngine, TerminalEngineMode};

use super::events::{spawn_sftp_event_pump, spawn_shell_event_pump};
use super::types::{
    ConnectionStore, LocalSnap, NetHist, SftpHandles, SftpManualNav, TabStatuses, TermBuffer,
    TermBuffers, TunnelStore,
};
use super::{
    AppWindow, SessionDraft, SessionInfo, SftpEntry, SftpTreeNode, TabInfo, TermMatch, TermSpan,
    TerminalState,
};

pub(super) fn sync_sessions_to_model(store: &ConfigStore, model: &VecModel<SessionInfo>) {
    let rows: Vec<SessionInfo> = store
        .sessions()
        .iter()
        .map(|s| SessionInfo {
            id: s.id.clone().into(),
            name: s.name.clone().into(),
            host: s.host.clone().into(),
            port: s.port as i32,
            user: s.user.clone().into(),
            auth: s.auth.as_str().into(),
            last_used: s
                .last_used
                .clone()
                .unwrap_or_else(|| "never".to_string())
                .into(),
        })
        .collect();
    model.set_vec(rows);
}

pub(super) fn wire_session_callbacks(
    window: &AppWindow,
    store: Rc<RefCell<ConfigStore>>,
    sessions_model: Rc<VecModel<SessionInfo>>,
    tabs_model: Rc<VecModel<TabInfo>>,
    terminals_model: Rc<VecModel<TerminalState>>,
    connections: ConnectionStore,
    bufs: TermBuffers,
    runtime: Arc<tokio::runtime::Runtime>,
    last_term_size: Arc<Mutex<(u32, u32)>>,
    sftp_handles: SftpHandles,
    sftp_manual_nav: SftpManualNav,
    tab_statuses: TabStatuses,
    local_snap: LocalSnap,
    local_net_hist: NetHist,
    tunnels: TunnelStore,
) {
    // New session -> open dialog with blank draft.
    let weak = window.as_weak();
    window.on_new_session_clicked(move || {
        if let Some(w) = weak.upgrade() {
            let empty = Session::new_empty();
            w.set_dialog_id(empty.id.into());
            w.set_dialog_name("".into());
            w.set_dialog_host("".into());
            w.set_dialog_port("22".into());
            w.set_dialog_user("root".into());
            w.set_dialog_auth("password".into());
            w.set_dialog_password("".into());
            w.set_dialog_key_path("".into());
            w.set_dialog_proxy("".into());
            w.set_dialog_editing(false);
            w.set_dialog_open(true);
        }
    });

    // Import hosts from ~/.ssh/config -> add them as sessions (skipping dups).
    {
        let weak = window.as_weak();
        let store = store.clone();
        let sessions_model = sessions_model.clone();
        window.on_import_ssh_config(move || {
            let hosts = crate::ssh_config::parse_default();
            let mut added = 0usize;
            if hosts.is_empty() {
                if let Some(w) = weak.upgrade() {
                    w.set_ssh_import_hint(
                        t("未找到 ~/.ssh/config", "no ~/.ssh/config found").into(),
                    );
                }
                return;
            }
            {
                let mut s = store.borrow_mut();
                for h in hosts {
                    let dup = s
                        .sessions()
                        .iter()
                        .any(|x| x.name == h.alias || (x.host == h.hostname && x.user == h.user));
                    if dup {
                        continue;
                    }
                    let auth = if h.identity_file.is_empty() {
                        AuthMethod::Password
                    } else {
                        AuthMethod::Key
                    };
                    s.upsert(Session {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: h.alias,
                        host: h.hostname,
                        port: h.port,
                        user: if h.user.is_empty() {
                            "root".into()
                        } else {
                            h.user
                        },
                        auth,
                        password: Secret::default(),
                        private_key_path: h.identity_file,
                        proxy: String::new(),
                        group: String::new(),
                        last_used: None,
                    });
                    added += 1;
                }
                if added > 0 {
                    let _ = s.save();
                }
            }
            sync_sessions_to_model(&store.borrow(), &sessions_model);
            if let Some(w) = weak.upgrade() {
                let hint = if added > 0 {
                    format!("{} {}", t("已导入", "imported"), added)
                } else {
                    t("没有新主机可导入", "no new hosts to import").to_string()
                };
                w.set_ssh_import_hint(hint.into());
            }
        });
    }

    // Edit -> open dialog prefilled.
    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_edit_session(move |id: SharedString| {
            let id = id.to_string();
            let store = store.borrow();
            let Some(session) = store.get(&id) else {
                return;
            };
            if let Some(w) = weak.upgrade() {
                w.set_dialog_id(session.id.clone().into());
                w.set_dialog_name(session.name.clone().into());
                w.set_dialog_host(session.host.clone().into());
                w.set_dialog_port(session.port.to_string().into());
                w.set_dialog_user(session.user.clone().into());
                w.set_dialog_auth(session.auth.as_str().into());
                w.set_dialog_password("".into());
                w.set_dialog_key_path(session.private_key_path.clone().into());
                w.set_dialog_proxy(session.proxy.clone().into());
                w.set_dialog_editing(true);
                w.set_dialog_open(true);
            }
        });
    }

    // Remove session.
    {
        let weak = window.as_weak();
        let store = store.clone();
        let sessions_model = sessions_model.clone();
        window.on_remove_session(move |id: SharedString| {
            {
                let mut s = store.borrow_mut();
                s.remove(&id.to_string());
                if let Err(err) = s.save() {
                    tracing::warn!("failed to save config: {err:#}");
                }
            }
            sync_sessions_to_model(&store.borrow(), &sessions_model);
            if let Some(w) = weak.upgrade() {
                let _ = w.get_sessions();
            }
        });
    }

    // Dialog submit -> persist + (optionally) connect.
    {
        let weak = window.as_weak();
        let store = store.clone();
        let sessions_model = sessions_model.clone();
        window.on_session_dialog_submit(move |draft: SessionDraft| {
            let id = draft.id.to_string();
            let password = if draft.password.is_empty() {
                store
                    .borrow()
                    .get(&id)
                    .map(|s| s.password.clone())
                    .unwrap_or_default()
            } else {
                Secret::new(draft.password.to_string())
            };
            let group = store
                .borrow()
                .get(&id)
                .map(|s| s.group.clone())
                .unwrap_or_default();
            let new_session = Session {
                id,
                name: if draft.name.is_empty() {
                    format!("{}@{}", draft.user, draft.host)
                } else {
                    draft.name.to_string()
                },
                host: draft.host.to_string(),
                port: if draft.port <= 0 {
                    22
                } else {
                    draft.port as u16
                },
                user: draft.user.to_string(),
                auth: AuthMethod::from_str(&draft.auth.to_string()),
                password,
                private_key_path: draft.private_key_path.to_string().replace('\\', "/"),
                proxy: draft.proxy.to_string(),
                group,
                last_used: None,
            };
            {
                let mut s = store.borrow_mut();
                s.upsert(new_session);
                if let Err(err) = s.save() {
                    tracing::warn!("failed to save config: {err:#}");
                }
            }
            sync_sessions_to_model(&store.borrow(), &sessions_model);
            if let Some(w) = weak.upgrade() {
                w.set_dialog_open(false);
            }
        });
    }

    // Cancel dialog.
    {
        let weak = window.as_weak();
        window.on_session_dialog_cancel(move || {
            if let Some(w) = weak.upgrade() {
                w.set_dialog_open(false);
            }
        });
    }

    // Private-key file picker.
    {
        let weak = window.as_weak();
        window.on_session_dialog_pick_key(move || {
            let mut dialog =
                rfd::FileDialog::new().set_title(t("选择私钥文件", "Choose private key file"));
            if let Some(home) = directories::UserDirs::new().map(|u| u.home_dir().join(".ssh")) {
                if home.is_dir() {
                    dialog = dialog.set_directory(home);
                }
            }
            if let Some(file) = dialog.pick_file() {
                let path = file.to_string_lossy().replace('\\', "/");
                if let Some(w) = weak.upgrade() {
                    w.set_dialog_key_path(path.into());
                }
            }
        });
    }

    // Connect session -> open a new terminal tab.
    {
        let weak = window.as_weak();
        let store = store.clone();
        let tabs_model = tabs_model.clone();
        let terminals_model = terminals_model.clone();
        let connections = connections.clone();
        let bufs = bufs.clone();
        let runtime = runtime.clone();
        let last_term_size = last_term_size.clone();
        let sftp_handles = sftp_handles.clone();
        let sftp_manual_nav = sftp_manual_nav.clone();
        let tab_statuses = tab_statuses.clone();
        let local_snap = local_snap.clone();
        let local_net_hist = local_net_hist.clone();
        let tunnels = tunnels.clone();
        window.on_connect_session(move |id: SharedString| {
            let id = id.to_string();
            let session = match store.borrow().get(&id).cloned() {
                Some(s) => s,
                None => return,
            };
            let terminal_engine_mode = store.borrow().terminal_engine_mode();
            let tab_id = format!("term-{}", uuid::Uuid::new_v4());
            let tab_title = session.name.clone();

            tab_statuses.lock().unwrap().insert(
                tab_id.clone(),
                super::TabStatus {
                    host: format!("{}@{}", session.user, session.host),
                    state: 0,
                    ..Default::default()
                },
            );

            tabs_model.push(TabInfo {
                id: tab_id.clone().into(),
                title: tab_title.into(),
                kind: "terminal".into(),
                connected: false,
            });
            terminals_model.push(TerminalState {
                id: tab_id.clone().into(),
                status: t("连接中...", "Connecting...").into(),
                spans: ModelRc::from(std::rc::Rc::new(VecModel::<TermSpan>::default())),
                cursor_row: 0,
                cursor_col: 0,
                rows_used: 0,
                is_alt_screen: false,
                mouse_reporting: false,
                find_matches: ModelRc::from(std::rc::Rc::new(VecModel::<TermMatch>::default())),
                selection: ModelRc::from(std::rc::Rc::new(VecModel::<TermMatch>::default())),
                sftp_path: "/".into(),
                sftp_entries: ModelRc::from(std::rc::Rc::new(VecModel::<SftpEntry>::default())),
                sftp_status: t("SFTP 连接中...", "SFTP connecting...").into(),
                sftp_loading: true,
                sftp_tree_nodes: ModelRc::from(std::rc::Rc::new(
                    VecModel::<SftpTreeNode>::default(),
                )),
            });
            let (buf, effective_mode) =
                TermBuffer::new_with_fallback(24, 80, 5000, terminal_engine_mode);
            tracing::info!(
                "new terminal tab {} using {} engine",
                tab_id,
                TerminalEngine::mode(&buf).as_str()
            );
            bufs.lock().unwrap().insert(tab_id.clone(), buf);
            sftp_manual_nav
                .lock()
                .unwrap()
                .insert(tab_id.clone(), false);
            if let Some(w) = weak.upgrade() {
                w.set_active_tab_id(tab_id.clone().into());
                if terminal_engine_mode == TerminalEngineMode::Alacritty
                    && effective_mode == TerminalEngineMode::Legacy
                {
                    let message = t(
                        "Alacritty 初始化失败，当前会话已回退到 Legacy",
                        "Alacritty initialization failed; this session fell back to Legacy",
                    );
                    w.set_settings_hint(message.into());
                    w.set_ssh_import_hint(message.into());
                }
            }

            let (initial_cols, initial_rows) = *last_term_size.lock().unwrap();
            let launch = connections.lock().unwrap().connect(
                runtime.handle(),
                tab_id.clone(),
                session.clone(),
                initial_cols,
                initial_rows,
            );

            let sftp_evt_tx = {
                let (sftp_tx, sftp_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
                let sftp_handle = spawn_sftp(runtime.handle(), session, sftp_tx);
                sftp_handles
                    .lock()
                    .unwrap()
                    .insert(tab_id.clone(), sftp_handle);
                sftp_rx
            };

            spawn_shell_event_pump(
                weak.clone(),
                tab_id.clone(),
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
                tab_id.clone(),
                sftp_evt_tx,
                bufs.clone(),
                tab_statuses.clone(),
                local_snap.clone(),
                local_net_hist.clone(),
            );
        });
    }
}
