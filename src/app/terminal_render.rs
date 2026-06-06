use crate::terminal::engine::TerminalEngine;

use super::models;
use super::{AppWindow, TermBuffers, TermMatch};

/// Find every (case-insensitive) occurrence of `query` across the currently
/// displayed rows and return highlight rectangles (char index == grid column).
pub(super) fn compute_find_matches(rows: &[String], query: &str) -> Vec<TermMatch> {
    let mut out: Vec<TermMatch> = Vec::new();
    if query.is_empty() {
        return out;
    }
    let q: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    if q.is_empty() {
        return out;
    }
    for (r, line) in rows.iter().enumerate() {
        let lower: Vec<char> = line.chars().map(|c| c.to_ascii_lowercase()).collect();
        let mut i = 0usize;
        while i + q.len() <= lower.len() {
            if lower[i..i + q.len()] == q[..] {
                out.push(TermMatch {
                    row: r as i32,
                    col: i as i32,
                    len: q.len() as i32,
                });
                i += q.len();
            } else {
                i += 1;
            }
        }
    }
    out
}

/// Order a selection so start ≤ end (by row, then column).
pub(super) fn norm_sel(sr: u16, sc: u16, er: u16, ec: u16) -> (u16, u16, u16, u16) {
    if (sr, sc) <= (er, ec) {
        (sr, sc, er, ec)
    } else {
        (er, ec, sr, sc)
    }
}

/// Highlight rectangles for a linear (line-wrapping) selection.
pub(super) fn selection_rects(sr: u16, sc: u16, er: u16, ec: u16, cols: u16) -> Vec<TermMatch> {
    let (sr, sc, er, ec) = norm_sel(sr, sc, er, ec);
    let mut out = Vec::new();
    if sr == er {
        let lo = sc.min(ec);
        let hi = sc.max(ec);
        out.push(TermMatch {
            row: sr as i32,
            col: lo as i32,
            len: (hi - lo + 1) as i32,
        });
    } else {
        out.push(TermMatch {
            row: sr as i32,
            col: sc as i32,
            len: (cols - sc) as i32,
        });
        for r in (sr + 1)..er {
            out.push(TermMatch {
                row: r as i32,
                col: 0,
                len: cols as i32,
            });
        }
        out.push(TermMatch {
            row: er as i32,
            col: 0,
            len: (ec + 1) as i32,
        });
    }
    out
}

/// Extract the selected text from the displayed rows (trailing spaces trimmed).
pub(super) fn extract_selection(rows: &[String], sr: u16, sc: u16, er: u16, ec: u16) -> String {
    let (sr, sc, er, ec) = norm_sel(sr, sc, er, ec);
    let mut out = String::new();
    for r in sr..=er {
        let chars: Vec<char> = rows
            .get(r as usize)
            .map(|l| l.chars().collect())
            .unwrap_or_default();
        let (lo, hi) = if sr == er {
            (sc.min(ec), sc.max(ec))
        } else if r == sr {
            (sc, u16::MAX)
        } else if r == er {
            (0, ec)
        } else {
            (0, u16::MAX)
        };
        let lo = (lo as usize).min(chars.len());
        let hi = ((hi as usize).saturating_add(1)).min(chars.len()); // exclusive
        let seg: String = if lo < hi {
            chars[lo..hi].iter().collect()
        } else {
            String::new()
        };
        out.push_str(seg.trim_end());
        if r != er {
            out.push('\n');
        }
    }
    out
}

/// Recompute spans + cursor + find/selection highlights for one tab from its
/// current vt100 screen (respecting scrollback) and push them to the model.
/// Used by scroll + selection callbacks (Output has its own equivalent inline).
pub(super) fn rebuild_tab_display(win: &AppWindow, bufs: &TermBuffers, tab_id: &str) {
    let data = {
        let mut map = bufs.lock().unwrap();
        let Some(buf) = map.get_mut(tab_id) else {
            return;
        };
        let cols = buf.parser.screen().size().1;
        let b = TerminalEngine::render(buf); // also refreshes buf.displayed_text
        let matches = compute_find_matches(&buf.displayed_text, &buf.find_query);
        let sel = match buf.sel {
            Some((sr, sc, er, ec)) => selection_rects(sr, sc, er, ec, cols),
            None => Vec::new(),
        };
        (b, matches, sel)
    };
    let (b, matches, sel) = data;
    let spans = models::term_spans_model(b.spans);
    let fm = slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(matches)));
    let sm = slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(sel)));
    let (cr, cc, ru, alt, mouse) = (
        b.cursor_row,
        b.cursor_col,
        b.rows_used,
        b.is_alt,
        b.mouse_reporting,
    );
    models::set_terminal_row(win, tab_id, move |row| {
        row.spans = spans.clone();
        row.cursor_row = cr;
        row.cursor_col = cc;
        row.rows_used = ru;
        row.is_alt_screen = alt;
        row.mouse_reporting = mouse;
        row.find_matches = fm.clone();
        row.selection = sm.clone();
    });
}
