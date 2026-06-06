use std::rc::Rc;

use slint::{Model, ModelRc, VecModel};

use crate::terminal::types::RenderSpan;

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
