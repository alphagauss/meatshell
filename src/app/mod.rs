//! Top-level UI state machine.
//!
//! Responsibilities:
//!   * Load the config store and expose sessions to Slint.
//!   * Drive the 1-Hz system sampler.
//!   * Manage the tab list + per-tab connection runtime map.
//!   * Route Slint callbacks to the right domain module.

mod events;
mod layout;
mod models;
mod platform;
mod sessions;
mod sftp_panel;
mod sidebar;
mod state;
mod tabs;
mod terminal_input;
mod terminal_render;
mod transfer;
mod tunnels;
mod types;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use tokio::runtime::Runtime;

use self::state::AppState;
use crate::config::ConfigStore;
use crate::connection::ConnectionManager;
use crate::i18n::t;
use crate::system::{SystemSampler, SystemSnapshot};
use crate::terminal::engine::TerminalEngineMode;
use crate::tunnel::{TunnelEvent, TunnelManager};

use self::layout::{sync_app_state_to_window, wire_layout_callbacks};
use self::platform::center_window;
use self::sessions::{sync_sessions_to_model, wire_session_callbacks};
use self::sftp_panel::{handle_file_drop, wire_sftp_callbacks};
use self::sidebar::{push_ring, refresh_sidebar};
use self::tabs::{wire_connection_toolbar_callbacks, wire_tab_callbacks};
use self::terminal_input::wire_key_input;
use self::transfer::wire_transfer_toolbar_callbacks;
use self::tunnels::{refresh_tunnel_panel, spawn_tunnel_event_pump, wire_tunnel_callbacks};
use self::types::{
    ConnectionStore, LocalSnap, NetHist, SftpHandles, SftpManualNav, TabStatus, TabStatuses,
    TermBuffers, TransferWindows, TunnelStore, NET_HISTORY_LEN,
};

// Slint generates types into this scope.
slint::include_modules!();

pub fn run() -> Result<()> {
    // --- Runtime + store -------------------------------------------------
    let runtime = Arc::new(Runtime::new().context("failed to start tokio runtime")?);
    let store = Rc::new(RefCell::new(
        ConfigStore::load().context("failed to load config")?,
    ));

    // Per-tab terminal connection runtimes.
    let connections: ConnectionStore = Arc::new(Mutex::new(ConnectionManager::new()));
    let (tunnel_tx, tunnel_rx) = tokio::sync::mpsc::unbounded_channel::<TunnelEvent>();
    let tunnels: TunnelStore = Arc::new(Mutex::new(
        TunnelManager::load(tunnel_tx).context("failed to load tunnel config")?,
    ));
    tracing::info!(
        "terminal engine mode: {}",
        store.borrow().terminal_engine_mode().as_str()
    );

    // Per-tab SFTP handles — Arc<Mutex> so the event-pump OS thread and the
    // Slint UI thread can both post SftpCommands.
    let sftp_handles: SftpHandles = Arc::new(Mutex::new(HashMap::new()));
    // Once the user navigates manually in the SFTP panel, stop auto-following cd.
    let sftp_manual_nav: SftpManualNav = Arc::new(Mutex::new(HashMap::new()));

    // Per-tab vt100 parsers + history logs (Arc<Mutex> so they can be cloned
    // into the thread that pumps session events into invoke_from_event_loop).
    let bufs: TermBuffers = Arc::new(Mutex::new(HashMap::new()));

    // Last-known terminal pixel dimensions, updated by every terminal-resize
    // callback.  Shared so on_connect_session can pass a sensible initial PTY
    // size to spawn_session before the first resize callback fires.
    // Default: 80 cols × 24 rows (SSH spec minimum).
    let last_term_size: Arc<Mutex<(u32, u32)>> = Arc::new(Mutex::new((80, 24)));

    // --- Build window + models ------------------------------------------
    // Set the Wayland app_id / X11 WM_CLASS *before* the window is created so
    // the Linux desktop shell can match the running window to the installed
    // `meatshell.desktop` entry and show our icon in the dock/taskbar.  (On
    // Windows the icon comes from the embedded .ico, so this is a no-op there.)
    let _ = slint::set_xdg_app_id("meatshell");
    let window = AppWindow::new().context("failed to build Slint window")?;

    // Apply the saved UI language.  The Rust-side flag drives `i18n::t(...)`;
    // `apply_to_slint` selects the bundled `.po` for the static `@tr(...)` text
    // (must run after the first component exists, which it now does).
    let startup_language = crate::i18n::normalize_language(store.borrow().language());
    crate::i18n::set_language(startup_language);
    crate::i18n::apply_to_slint();
    window.set_lang_en(crate::i18n::is_en());

    {
        let is_dark = match store.borrow().theme_pref() {
            "light" => false,
            "dark" => true,
            _ => match dark_light::detect() {
                dark_light::Mode::Light => false,
                dark_light::Mode::Dark => true,
                dark_light::Mode::Default => true,
            },
        };
        window.set_dark_mode(is_dark);
    }
    {
        let s = store.borrow();
        let family = s.font_family().to_string();
        if !family.is_empty() {
            window.set_term_font_family(family.into());
        }
        window.set_term_font_size(s.font_size() as f32);
    }
    window.set_term_fonts(ModelRc::from(Rc::new(VecModel::from(
        system_monospace_fonts(),
    ))));
    window.set_terminal_engine_mode(store.borrow().terminal_engine_mode().as_str().into());

    let app_state = Rc::new(RefCell::new(AppState::default()));
    sync_app_state_to_window(&window, &app_state.borrow());

    let sessions_model: Rc<VecModel<SessionInfo>> = Rc::new(VecModel::default());
    window.set_sessions(ModelRc::from(sessions_model.clone()));
    sync_sessions_to_model(&store.borrow(), &sessions_model);

    let tabs_model: Rc<VecModel<TabInfo>> = Rc::new(VecModel::default());
    tabs_model.push(TabInfo {
        id: "welcome".into(),
        title: t("新标签页", "New tab").into(),
        kind: "welcome".into(),
        connected: false,
    });
    window.set_tabs(ModelRc::from(tabs_model.clone()));
    window.set_active_tab_id("welcome".into());

    let terminals_model: Rc<VecModel<TerminalState>> = Rc::new(VecModel::default());
    window.set_terminals(ModelRc::from(terminals_model.clone()));
    window.set_tunnel_rules(ModelRc::from(
        Rc::new(VecModel::<TunnelRuleInfo>::default()),
    ));

    // Per-tab connection status + remote resources, the latest local sample,
    // and the local machine's network history (bottom sparkline).
    let tab_statuses: TabStatuses = Arc::new(Mutex::new(HashMap::new()));
    let local_snap: LocalSnap = Arc::new(Mutex::new(SystemSnapshot::default()));
    let local_net_hist: NetHist = Arc::new(Mutex::new(vec![0.0; NET_HISTORY_LEN]));
    let transfer_windows: TransferWindows = Rc::new(RefCell::new(None));

    // --- Wire callbacks --------------------------------------------------
    wire_layout_callbacks(&window, app_state.clone());
    wire_session_callbacks(
        &window,
        store.clone(),
        sessions_model.clone(),
        tabs_model.clone(),
        terminals_model.clone(),
        connections.clone(),
        bufs.clone(),
        runtime.clone(),
        last_term_size.clone(),
        sftp_handles.clone(),
        sftp_manual_nav.clone(),
        tab_statuses.clone(),
        local_snap.clone(),
        local_net_hist.clone(),
        tunnels.clone(),
    );
    wire_connection_toolbar_callbacks(
        &window,
        tabs_model.clone(),
        connections.clone(),
        bufs.clone(),
        runtime.clone(),
        last_term_size.clone(),
        sftp_handles.clone(),
        sftp_manual_nav.clone(),
        tab_statuses.clone(),
        local_snap.clone(),
        local_net_hist.clone(),
        tunnels.clone(),
    );
    wire_transfer_toolbar_callbacks(
        &window,
        connections.clone(),
        runtime.clone(),
        store.clone(),
        transfer_windows.clone(),
    );

    // Recompute the sidebar whenever the active tab changes (fired from Slint's
    // `changed active-tab-id`).
    {
        let weak = window.as_weak();
        let statuses = tab_statuses.clone();
        let local = local_snap.clone();
        let net = local_net_hist.clone();
        let connections = connections.clone();
        let tunnels = tunnels.clone();
        window.on_refresh_sidebar(move || {
            if let Some(w) = weak.upgrade() {
                refresh_sidebar(&w, &statuses, &local, &net);
                refresh_tunnel_panel(&w, &connections, &tunnels);
            }
        });
    }

    // Switch UI language at runtime.  Static `@tr(...)` text updates live via
    // select_bundled_translation; we additionally refresh the Rust-driven
    // dynamic strings (sidebar status + the welcome tab title).
    {
        let weak = window.as_weak();
        let store = store.clone();
        let tabs_model = tabs_model.clone();
        window.on_set_language(move |code| {
            crate::i18n::set_language(&code.to_string());
            {
                let mut s = store.borrow_mut();
                s.set_language(crate::i18n::current_code().to_string());
                let _ = s.save();
            }
            // Re-translate the welcome tab's dynamic title.
            for i in 0..tabs_model.row_count() {
                if let Some(mut row) = tabs_model.row_data(i) {
                    if row.id.as_str() == "welcome" {
                        row.title = t("新标签页", "New tab").into();
                        tabs_model.set_row_data(i, row);
                    }
                }
            }
            if let Some(w) = weak.upgrade() {
                w.set_lang_en(crate::i18n::is_en());
                w.invoke_refresh_sidebar();
            }
        });
    }

    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_toggle_theme(move || {
            let Some(w) = weak.upgrade() else { return };
            let next_dark = !w.get_dark_mode();
            w.set_dark_mode(next_dark);
            {
                let mut s = store.borrow_mut();
                s.set_theme_pref(if next_dark { "dark" } else { "light" }.to_string());
                if let Err(err) = s.save() {
                    tracing::warn!("failed to save config: {err:#}");
                }
            }
        });
    }

    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_set_term_font(move |family: SharedString| {
            {
                let mut s = store.borrow_mut();
                s.set_font_family(family.to_string());
                if let Err(err) = s.save() {
                    tracing::warn!("failed to save config: {err:#}");
                }
            }
            if let Some(w) = weak.upgrade() {
                w.set_term_font_family(family);
            }
        });
    }

    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_set_term_font_size(move |size: i32| {
            {
                let mut s = store.borrow_mut();
                s.set_font_size(size as u32);
                if let Err(err) = s.save() {
                    tracing::warn!("failed to save config: {err:#}");
                }
            }
            if let Some(w) = weak.upgrade() {
                w.set_term_font_size(size as f32);
            }
        });
    }

    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_set_terminal_engine_mode(move |mode| {
            let requested_mode = TerminalEngineMode::from_str(mode.as_str());
            let effective_mode = {
                let mut s = store.borrow_mut();
                s.set_terminal_engine_mode(requested_mode);
                if let Err(err) = s.save() {
                    tracing::warn!("failed to save config: {err:#}");
                }
                s.terminal_engine_mode()
            };
            let message = format!(
                "{}: {}",
                t(
                    "终端引擎已更新，新建会话生效",
                    "Terminal engine updated; new sessions only"
                ),
                effective_mode.as_str()
            );
            if let Some(w) = weak.upgrade() {
                w.set_terminal_engine_mode(effective_mode.as_str().into());
                w.set_settings_hint(message.clone().into());
                w.set_ssh_import_hint(message.into());
            }
        });
    }

    // NIC selector: remember the user's choice for the active tab and refresh.
    {
        let weak = window.as_weak();
        let statuses = tab_statuses.clone();
        let local = local_snap.clone();
        let net = local_net_hist.clone();
        window.on_select_net_iface(move |iface: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let active = w.get_active_tab_id().to_string();
            if let Some(st) = statuses.lock().unwrap().get_mut(&active) {
                st.selected_iface = iface.to_string();
                st.net_hist = vec![0.0; NET_HISTORY_LEN]; // reset graph for new NIC
            }
            refresh_sidebar(&w, &statuses, &local, &net);
        });
    }

    // Settings: preset download directory (load + pick + open).
    window.set_download_dir(store.borrow().download_dir().to_string().into());
    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_pick_download_dir(move || {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let dir = folder.to_string_lossy().to_string();
                {
                    let mut s = store.borrow_mut();
                    s.set_download_dir(dir.clone());
                    let _ = s.save();
                }
                if let Some(w) = weak.upgrade() {
                    w.set_download_dir(dir.into());
                }
            }
        });
    }
    {
        let weak = window.as_weak();
        window.on_open_download_dir(move || {
            let Some(w) = weak.upgrade() else { return };
            let dir = w.get_download_dir().to_string();
            if dir.is_empty() {
                return;
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
            }
        });
    }

    // Transfer records (download/upload progress + history) shown in the popup.
    let transfers_model: Rc<VecModel<TransferInfo>> = Rc::new(VecModel::default());
    window.set_transfers(ModelRc::from(transfers_model.clone()));
    {
        let tm = transfers_model.clone();
        window.on_clear_transfers(move || tm.set_vec(Vec::<TransferInfo>::new()));
    }

    // Open-source libraries shown in the About popup.
    {
        let libs: Vec<SharedString> = [
            t("Slint — 图形界面框架 (GUI)", "Slint — GUI framework"),
            t(
                "russh / russh-keys — SSH 协议实现",
                "russh / russh-keys — SSH protocol",
            ),
            t(
                "russh-sftp — SFTP 文件传输",
                "russh-sftp — SFTP file transfer",
            ),
            t("ssh-key — SSH 密钥解析", "ssh-key — SSH key parsing"),
            t("tokio — 异步运行时", "tokio — async runtime"),
            t(
                "vt100 — 终端 (VT100/xterm) 解析",
                "vt100 — terminal (VT100/xterm) parser",
            ),
            t(
                "sysinfo — 本机资源采集",
                "sysinfo — local resource sampling",
            ),
            t(
                "serde / serde_json — 配置序列化",
                "serde / serde_json — config serialization",
            ),
            t("arboard — 系统剪贴板", "arboard — system clipboard"),
            t("rfd — 原生文件对话框", "rfd — native file dialogs"),
            t(
                "directories — 配置目录定位",
                "directories — config dir lookup",
            ),
            t("chrono — 日期时间处理", "chrono — date/time handling"),
            t("uuid — 唯一标识符", "uuid — unique identifiers"),
            t(
                "anyhow / thiserror — 错误处理",
                "anyhow / thiserror — error handling",
            ),
            t(
                "tracing / tracing-subscriber — 日志",
                "tracing / tracing-subscriber — logging",
            ),
            t(
                "futures / async-trait — 异步辅助",
                "futures / async-trait — async helpers",
            ),
            t("rand — 随机数", "rand — randomness"),
            t(
                "winresource — Windows 图标/资源嵌入",
                "winresource — Windows icon/resource embedding",
            ),
        ]
        .iter()
        .map(|s| (*s).into())
        .collect();
        window.set_about_libs(ModelRc::from(Rc::new(VecModel::from(libs))));
    }

    wire_tab_callbacks(
        &window,
        tabs_model.clone(),
        terminals_model.clone(),
        connections.clone(),
        bufs.clone(),
        sftp_handles.clone(),
        sftp_manual_nav.clone(),
        tunnels.clone(),
    );
    wire_sftp_callbacks(&window, sftp_handles.clone(), sftp_manual_nav.clone());
    wire_tunnel_callbacks(
        &window,
        connections.clone(),
        tunnels.clone(),
        runtime.clone(),
    );
    wire_key_input(
        &window,
        connections.clone(),
        bufs.clone(),
        last_term_size.clone(),
    );
    spawn_tunnel_event_pump(
        window.as_weak(),
        tunnel_rx,
        connections.clone(),
        tunnels.clone(),
    );

    // --- System sampler (1 Hz) ------------------------------------------
    let sampler = Rc::new(Mutex::new(SystemSampler::new()));
    let weak = window.as_weak();
    let tick_sampler = sampler.clone();
    let tick_statuses = tab_statuses.clone();
    let tick_local = local_snap.clone();
    let tick_net = local_net_hist.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        SystemSampler::recommended_interval(),
        move || {
            let snap = {
                let mut s = tick_sampler.lock().expect("sampler poisoned");
                s.sample()
            };
            // Append the raw local throughput to the bottom-graph ring buffer
            // (normalisation happens at display time so the graph auto-scales).
            push_ring(&mut tick_net.lock().unwrap(), snap.net_bytes_per_sec as f32);
            // Stash the local sample; the sidebar shows it on the welcome tab
            // and in the bottom network graph.
            *tick_local.lock().unwrap() = snap.clone();

            if let Some(w) = weak.upgrade() {
                // Everything (status, CPU/mem/swap, both graphs) follows the
                // active tab; refresh_sidebar reads the stores we just updated.
                refresh_sidebar(&w, &tick_statuses, &tick_local, &tick_net);
            }
        },
    );
    // Keep the timer alive for the entire event loop by parking it on a
    // leaked Box. Slint timers drop themselves on Drop, and we don't want
    // that here.
    Box::leak(Box::new(timer));

    // OS file drag-and-drop → upload to the active session's SFTP directory,
    // but only when the file is dropped over the file-list area.
    {
        use i_slint_backend_winit::winit::event::WindowEvent as WEvent;
        use i_slint_backend_winit::EventResult;
        let weak = window.as_weak();
        let sh = sftp_handles.clone();
        window.window().on_winit_window_event(move |_w, event| {
            if let WEvent::DroppedFile(path) = event {
                if let Some(win) = weak.upgrade() {
                    handle_file_drop(&win, &sh, path.to_string_lossy().to_string());
                }
            }
            EventResult::Propagate
        });
    }

    // Center the window on the primary monitor once it's shown (size is only
    // known after the first frame, so defer via a single-shot timer).
    {
        let weak = window.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
            if let Some(w) = weak.upgrade() {
                center_window(&w);
            }
        });
    }

    window.run().context("event loop exited with error")?;
    Ok(())
}

/// Enumerate installed monospace font families for the terminal font picker.
fn system_monospace_fonts() -> Vec<slint::SharedString> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .filter(|f| f.monospaced)
        .filter_map(|f| f.families.first().map(|(name, _)| name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().map(slint::SharedString::from).collect()
}
