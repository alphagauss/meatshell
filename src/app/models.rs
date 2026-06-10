use std::rc::Rc;

use slint::{Model, ModelRc, VecModel};

use crate::config::Session;
use crate::i18n::t;
use crate::terminal::types::RenderSpan;

use super::types::{ConnectionStore, SftpHandles};
use super::{AppWindow, TermSpan, TerminalState};

/// Convert a Rust render span list into the Slint model used by the terminal UI.
pub(super) fn term_spans_model(spans: Vec<RenderSpan>) -> ModelRc<TermSpan> {
    let rows: Vec<TermSpan> = spans
        .into_iter()
        .map(|span| TermSpan {
            text: span.text.into(),
            fg: span.fg,
            bg: span.bg,
            bold: span.bold,
            row: span.row,
            col: span.col,
            cells: span.cells,
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

/// Mutate the `TerminalState` whose id matches `tab_id` in the live model.
/// Must run on the Slint event loop thread.
pub(super) fn set_terminal_row(
    win: &AppWindow,
    tab_id: &str,
    mutator: impl Fn(&mut TerminalState),
) {
    let terminals = win.get_terminals();
    let Some(model) = terminals.as_any().downcast_ref::<VecModel<TerminalState>>() else {
        return;
    };
    for i in 0..model.row_count() {
        if let Some(mut row) = model.row_data(i) {
            if row.id.as_str() == tab_id {
                mutator(&mut row);
                model.set_row_data(i, row);
                break;
            }
        }
    }
}

pub(super) fn show_connect_session_hint(win: &AppWindow, tab_id: &str) {
    let message = t("请先连接一个会话", "Connect a session first");
    if tab_id == "welcome" || win.get_active_tab_id().as_str() == "welcome" {
        win.set_ssh_import_hint(message.into());
    } else {
        set_terminal_row(win, tab_id, |row| {
            row.status = message.into();
        });
    }
}

pub(super) fn active_session_or_hint(
    win: &AppWindow,
    connections: &ConnectionStore,
) -> Option<(String, Session)> {
    let active = win.get_active_tab_id().to_string();
    let session = if active == "welcome" {
        None
    } else {
        connections.lock().unwrap().session(&active)
    };
    match session {
        Some(session) => Some((active, session)),
        None => {
            show_connect_session_hint(win, &active);
            None
        }
    }
}

pub(super) fn sftp_handle_or_hint(
    win: &AppWindow,
    sftp_handles: &SftpHandles,
    tab_id: &str,
) -> bool {
    if sftp_handles.lock().unwrap().contains_key(tab_id) {
        true
    } else {
        show_connect_session_hint(win, tab_id);
        false
    }
}
