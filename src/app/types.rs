use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::config::Session;
use crate::connection::ConnectionManager;
use crate::sftp::SftpHandle;
use crate::ssh::RemoteProcess;
use crate::system::SystemSnapshot;
use crate::tunnel::TunnelManager;

pub(super) type TermBuffers = Arc<Mutex<HashMap<String, TermBuffer>>>;
pub(super) type SftpHandles = Arc<Mutex<HashMap<String, SftpHandle>>>;
pub(super) type ConnectionStore = Arc<Mutex<ConnectionManager>>;
pub(super) type TunnelStore = Arc<Mutex<TunnelManager>>;
pub(super) type TermBuffer = crate::terminal::legacy::LegacyTerminalEngine;
/// Per-tab flag: once the user explicitly navigates via the SFTP tree or
/// toolbar, stop auto-syncing to the terminal's `cd` path.
pub(super) type SftpManualNav = Arc<Mutex<HashMap<String, bool>>>;

/// Per-tab connection status + latest remote resource sample, used to drive the
/// sidebar for whichever tab is active.  `Arc<Mutex>` because the SSH event-pump
/// threads update it before bouncing to the UI thread.
#[derive(Clone, Default)]
pub(super) struct TabStatus {
    pub(super) host: String, // "root@192.168.100.2"
    pub(super) state: u8,    // 0 = connecting, 1 = connected, 2 = disconnected
    pub(super) cpu: f32,     // 0.0..1.0
    pub(super) mem_used_kib: u64,
    pub(super) mem_total_kib: u64,
    pub(super) swap_used_kib: u64,
    pub(super) swap_total_kib: u64,
    /// Latest per-interface rates: (name, rx_bps, tx_bps), busiest first.
    pub(super) net: Vec<(String, u64, u64)>,
    /// Which interface drives the top sparkline (empty = auto = busiest).
    pub(super) selected_iface: String,
    /// Ring buffer of the selected interface's total (rx+tx) bytes/sec.
    pub(super) net_hist: Vec<f32>,
    /// Per-filesystem (mount, available_bytes, total_bytes).
    pub(super) disks: Vec<(String, u64, u64)>,
    /// Latest remote processes shown in the sidebar.
    pub(super) processes: Vec<RemoteProcess>,
    /// Current process sort key: "mem" or "cpu".
    pub(super) process_sort_key: String,
}
pub(super) type TabStatuses = Arc<Mutex<HashMap<String, TabStatus>>>;
/// Last local-machine sample (shown on the welcome tab).
pub(super) type LocalSnap = Arc<Mutex<SystemSnapshot>>;

#[allow(dead_code)]
pub(super) struct TransferRemoteTab {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) session: Session,
    pub(super) sftp: Rc<SftpHandle>,
    pub(super) remote_path: String,
    pub(super) connected: bool,
}

#[allow(dead_code)]
pub(super) struct TransferWindowState {
    pub(super) window: super::TransferWindow,
    pub(super) remote_tabs: Rc<RefCell<Vec<TransferRemoteTab>>>,
}
pub(super) type TransferWindows = Rc<RefCell<Option<TransferWindowState>>>;

/// Number of samples kept for the sparkline.
pub(super) const NET_HISTORY_LEN: usize = 60;
pub(super) type NetHist = Arc<Mutex<Vec<f32>>>;
