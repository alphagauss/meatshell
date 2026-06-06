use super::alacritty::AlacrittyTerminalEngine;
use super::engine::{TerminalEngine, TerminalEngineMode};
use super::types::{BuiltScreen, HistSpan, Line, RenderSpan};

/// Per-terminal state: vt100 parser drives all rendering for both normal
/// (bash) and alt-screen (vim/nano/htop) modes.
///
/// Using vt100 for normal mode too is necessary because readline rewrites the
/// current input line using `\r` + full-line redraw + `\x1b[K` (erase to EOL)
/// whenever the cursor moves. A naive append-only buffer would duplicate the
/// text; vt100 tracks cursor position and overwrites in place correctly.
pub struct LegacyTerminalEngine {
    pub parser: vt100::Parser,
    pub alacritty: Option<AlacrittyTerminalEngine>,
    /// Active find query for this tab ("" = no search).
    pub find_query: String,
    /// Drag selection (start_row, start_col, end_row, end_col) in grid cells.
    pub sel: Option<(u16, u16, u16, u16)>,
    /// Session scrollback: lines that have scrolled off the top (oldest first).
    pub history: Vec<Line>,
    /// Previous frame's grid lines, for scroll-off detection.
    pub prev: Vec<Line>,
    /// Scrollback view offset in lines (0 = live bottom).
    pub view_offset: usize,
    /// Plain text of the rows currently displayed (drives find + selection).
    pub displayed_text: Vec<String>,
    /// CSI-scanner state for rewriting HVP (`ESC [ … f`) into CUP (`ESC [ … H`).
    /// vt100 0.15 only implements the `H` final byte, not the equivalent `f`
    /// that btop/htop use for cursor positioning — without this rewrite their
    /// absolute-positioned full-screen output collapses into a scrolling mess.
    /// Kept here so a sequence split across read chunks is still translated.
    pub csi_state: CsiState,
}

/// Minimal CSI-final-byte rewriter state (persists across read chunks).
#[derive(Clone, Copy, PartialEq)]
pub enum CsiState {
    /// Normal text.
    Normal,
    /// Saw ESC (0x1b), waiting to see if it starts a CSI (`[`).
    Esc,
    /// Inside a CSI sequence (after `ESC [`), scanning params/intermediates.
    Csi,
}

/// Per-session scrollback cap (recycled on clear / tab close).
pub const MAX_HISTORY: usize = 100_000;

/// Build one screen row into `(plain_text, coloured_runs)`.  `plain` carries one
/// char per cell (space for blanks) so a char index equals the grid column.
pub fn build_row(screen: &vt100::Screen, r: u16, cols: u16) -> Line {
    let mut plain = String::with_capacity(cols as usize);
    let mut runs: Vec<HistSpan> = Vec::new();
    let mut c = 0u16;
    while c < cols {
        let (s, fg, bg, bold) = cell_attrs(screen, r, c);
        // Group consecutive cells that share fg + bg + bold into one run.  Unlike
        // before we keep blank cells *inside* a run (so a coloured bar made of
        // spaces still gets a background fill) and break only on attribute change.
        let start_col = c;
        let mut text = s.clone();
        plain.push_str(&s);
        c += 1;
        while c < cols {
            let (cs, cfg, cbg, cbold) = cell_attrs(screen, r, c);
            if cfg != fg || cbg != bg || cbold != bold {
                break;
            }
            plain.push_str(&cs);
            text.push_str(&cs);
            c += 1;
        }
        let cells = (c - start_col) as i32;
        let is_blank = text.chars().all(|ch| ch == ' ');
        let bg_default = matches!(bg, vt100::Color::Default);
        // Skip runs that contribute nothing visible: blank text *and* default bg.
        if is_blank && bg_default {
            continue;
        }
        runs.push(HistSpan {
            text,
            fg: vt_color_to_slint(fg, bold),
            bg: vt_bg_to_slint(bg),
            bold,
            col: start_col as i32,
            cells,
        });
    }
    (plain, runs)
}

impl LegacyTerminalEngine {
    pub fn new(rows: u16, cols: u16, scrollback: usize, mode: TerminalEngineMode) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, scrollback),
            alacritty: match mode {
                TerminalEngineMode::Legacy => None,
                TerminalEngineMode::AlacrittyExperimental => {
                    Some(AlacrittyTerminalEngine::new(rows, cols))
                }
            },
            find_query: String::new(),
            sel: None,
            history: Vec::new(),
            prev: Vec::new(),
            view_offset: 0,
            displayed_text: Vec::new(),
            csi_state: CsiState::Normal,
        }
    }

    pub fn mouse_reporting(&self) -> bool {
        self.alacritty
            .as_ref()
            .map(|engine| engine.mouse_reporting())
            .unwrap_or(false)
    }

    /// Feed bytes to vt100 and capture scrolled-off lines into history.
    ///
    /// We detect scroll by diffing the screen before/after a `process`, which
    /// can only recover up to one screen of shift per call.  A single large
    /// burst can scroll many screens at once, so we split the input at newline
    /// boundaries into batches of at most ~half a screen of lines and capture
    /// after each — that way no batch ever scrolls more than the diff can see,
    /// and nothing is lost.  (Splitting only on `\n` is safe: VT escape
    /// sequences never contain a newline.)
    fn ingest(&mut self, raw: &[u8]) {
        // Rewrite HVP (`ESC [ … f`) → CUP (`ESC [ … H`) so vt100 (which only
        // implements `H`) honours btop/htop's absolute cursor positioning.
        let bytes = self.rewrite_hvp(raw);
        let bytes = &bytes[..];
        let rows = self.parser.screen().size().0 as usize;
        let batch_lines = (rows / 2).max(1);
        let mut start = 0usize;
        let mut nl = 0usize;
        for i in 0..bytes.len() {
            if bytes[i] == b'\n' {
                nl += 1;
                if nl >= batch_lines {
                    self.ingest_chunk(&bytes[start..=i]);
                    start = i + 1;
                    nl = 0;
                }
            }
        }
        if start < bytes.len() {
            self.ingest_chunk(&bytes[start..]);
        }
    }

    /// Translate every CSI sequence terminated by `f` (HVP) into the identical
    /// sequence terminated by `H` (CUP).  The scanner state persists across
    /// calls, so a sequence split across read chunks is still handled.  Only the
    /// final byte of a CSI sequence is ever touched; text bytes pass through.
    fn rewrite_hvp(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            match self.csi_state {
                CsiState::Normal => {
                    if b == 0x1b {
                        self.csi_state = CsiState::Esc;
                    }
                    out.push(b);
                }
                CsiState::Esc => {
                    if b == b'[' {
                        self.csi_state = CsiState::Csi;
                    } else {
                        // Not a CSI (could be another ESC, OSC, etc.).  Re-arm on
                        // a fresh ESC, otherwise fall back to normal text.
                        self.csi_state = if b == 0x1b {
                            CsiState::Esc
                        } else {
                            CsiState::Normal
                        };
                    }
                    out.push(b);
                }
                CsiState::Csi => {
                    // Final bytes are 0x40..=0x7e; params/intermediates are
                    // 0x20..=0x3f.  Rewrite an `f` final into `H`.
                    if (0x40..=0x7e).contains(&b) {
                        out.push(if b == b'f' { b'H' } else { b });
                        self.csi_state = CsiState::Normal;
                    } else {
                        out.push(b);
                    }
                }
            }
        }
        out
    }

    /// Process one bounded batch and capture any lines that scrolled off the top
    /// (skipped for alt-screen programs like vim/nano).
    fn ingest_chunk(&mut self, bytes: &[u8]) {
        // Detect full-screen-clear sequences *before* processing so we can
        // suppress history for programs that redraw without alt-screen (e.g.
        // btop configured with `alt-screen = false`).
        // We look for \033[H (cursor-home) and \033[2J / \033[J (erase display)
        // as indicators that the program is doing a full-screen refresh.
        let has_cursor_home = bytes.windows(3).any(|w| w == b"\x1b[H");
        let has_erase_display =
            bytes.windows(4).any(|w| w == b"\x1b[2J") || bytes.windows(3).any(|w| w == b"\x1b[J");
        let is_fullscreen_refresh = has_cursor_home && has_erase_display;

        self.parser.process(bytes);
        let (is_alt, rows, cols) = {
            let s = self.parser.screen();
            let (r, c) = s.size();
            (s.alternate_screen(), r, c)
        };
        if is_alt {
            // Snap to live view whenever we're on the alt screen — this
            // prevents old history (accumulated before alt-screen was entered)
            // from mixing with the full-screen program's output after a scroll.
            self.view_offset = 0;
            self.prev.clear();
            return;
        }
        if is_fullscreen_refresh {
            // Non-alt-screen full-screen refresh (btop, htop with alt disabled…).
            // Don't capture lines into history; they'd mix with the next frame.
            self.view_offset = 0;
            self.prev.clear();
            return;
        }
        let curr: Vec<Line> = {
            let s = self.parser.screen();
            (0..rows).map(|r| build_row(s, r, cols)).collect()
        };
        if !self.prev.is_empty() {
            let k = detect_scroll(&self.prev, &curr);
            for line in self.prev.iter().take(k) {
                self.history.push(line.clone());
            }
            if self.history.len() > MAX_HISTORY {
                let drop = self.history.len() - MAX_HISTORY;
                self.history.drain(0..drop);
            }
        }
        self.prev = curr;
    }

    /// Render the terminal grid for the current scrollback `view_offset`
    /// (0 = live).  Caches the displayed plain text for find/selection.
    fn render(&mut self) -> BuiltScreen<RenderSpan> {
        let (is_alt, rows, cols, cur_row, cur_col) = {
            let s = self.parser.screen();
            let (r, c) = s.size();
            let (cr, cc) = s.cursor_position();
            (s.alternate_screen(), r, c, cr, cc)
        };

        // --- Live view (also alt-screen): render the current grid -----------
        if is_alt || self.view_offset == 0 {
            let mut spans = Vec::new();
            let mut displayed = Vec::with_capacity(rows as usize);
            let mut last_content = 0i32;
            let s = self.parser.screen();
            for r in 0..rows {
                let (plain, runs) = build_row(s, r, cols);
                if !runs.is_empty() {
                    last_content = r as i32;
                }
                for hs in runs {
                    spans.push(RenderSpan {
                        text: hs.text,
                        fg: hs.fg,
                        bg: hs.bg,
                        bold: hs.bold,
                        row: r as i32,
                        col: hs.col,
                        cells: hs.cells,
                    });
                }
                displayed.push(plain.trim_end().to_string());
            }
            self.displayed_text = displayed;
            let rows_used = if is_alt {
                rows as i32
            } else {
                last_content + 1
            };
            return BuiltScreen {
                spans,
                cursor_row: cur_row as i32,
                cursor_col: cur_col as i32,
                rows_used,
                is_alt,
                mouse_reporting: false,
            };
        }

        // --- Scrolled view: window into history ++ live content -------------
        let live: Vec<Line> = {
            let s = self.parser.screen();
            (0..rows).map(|r| build_row(s, r, cols)).collect()
        };
        let live_used = live
            .iter()
            .rposition(|(_, r)| !r.is_empty())
            .map(|i| i + 1)
            .unwrap_or(0);
        let hist_len = self.history.len();
        let combined_len = hist_len + live_used;
        let win = rows as usize;
        let start = combined_len.saturating_sub(win + self.view_offset);
        let end = (start + win).min(combined_len);

        let mut spans = Vec::new();
        let mut displayed = Vec::with_capacity(win);
        for (d, idx) in (start..end).enumerate() {
            let line: &Line = if idx < hist_len {
                &self.history[idx]
            } else {
                &live[idx - hist_len]
            };
            for hs in &line.1 {
                spans.push(RenderSpan {
                    text: hs.text.clone(),
                    fg: hs.fg,
                    bg: hs.bg,
                    bold: hs.bold,
                    row: d as i32,
                    col: hs.col,
                    cells: hs.cells,
                });
            }
            displayed.push(line.0.trim_end().to_string());
        }
        while displayed.len() < win {
            displayed.push(String::new());
        }
        self.displayed_text = displayed;
        BuiltScreen {
            spans,
            cursor_row: -1, // hide the live cursor while viewing history
            cursor_col: 0,
            rows_used: win as i32,
            is_alt: false,
            mouse_reporting: false,
        }
    }
}

impl TerminalEngine for LegacyTerminalEngine {
    type Screen = BuiltScreen<RenderSpan>;

    fn ingest(&mut self, bytes: &[u8]) {
        if let Some(engine) = self.alacritty.as_mut() {
            TerminalEngine::ingest(engine, bytes);
        } else {
            LegacyTerminalEngine::ingest(self, bytes);
        }
    }

    fn render(&mut self) -> Self::Screen {
        if let Some(engine) = self.alacritty.as_mut() {
            let screen = TerminalEngine::render(engine);
            self.displayed_text = engine.displayed_text().to_vec();
            screen
        } else {
            LegacyTerminalEngine::render(self)
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.set_size(rows, cols);
        if let Some(engine) = self.alacritty.as_mut() {
            TerminalEngine::resize(engine, rows, cols);
        }
    }
}

/// Standard 16-colour ANSI palette (VS Code "Dark+" values — reads well on the
/// dark terminal background).
const ANSI16: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // 0 black
    (0xcd, 0x31, 0x31), // 1 red
    (0x0d, 0xbc, 0x79), // 2 green
    (0xe5, 0xe5, 0x10), // 3 yellow
    (0x24, 0x72, 0xc8), // 4 blue
    (0xbc, 0x3f, 0xbc), // 5 magenta
    (0x11, 0xa8, 0xcd), // 6 cyan
    (0xe5, 0xe5, 0xe5), // 7 white
    (0x66, 0x66, 0x66), // 8 bright black
    (0xf1, 0x4c, 0x4c), // 9 bright red
    (0x23, 0xd1, 0x8b), // 10 bright green
    (0xf5, 0xf5, 0x43), // 11 bright yellow
    (0x3b, 0x8e, 0xea), // 12 bright blue
    (0xd6, 0x70, 0xd6), // 13 bright magenta
    (0x29, 0xb8, 0xdb), // 14 bright cyan
    (0xff, 0xff, 0xff), // 15 bright white
];

/// Effective (contents, fg, bg, bold) for one grid cell, applying reverse-video.
/// `contents` is always one display string (" " for a blank cell).
fn cell_attrs(
    screen: &vt100::Screen,
    r: u16,
    c: u16,
) -> (String, vt100::Color, vt100::Color, bool) {
    match screen.cell(r, c) {
        Some(cell) => {
            let (mut fg, mut bg) = (cell.fgcolor(), cell.bgcolor());
            if cell.inverse() {
                std::mem::swap(&mut fg, &mut bg);
            }
            let s = cell.contents();
            let s = if s.is_empty() { " ".to_string() } else { s };
            (s, fg, bg, cell.bold())
        }
        None => (
            " ".to_string(),
            vt100::Color::Default,
            vt100::Color::Default,
            false,
        ),
    }
}

/// Detect how many lines scrolled off the top between two screen snapshots by
/// finding the vertical shift `k` that best aligns `prev` onto `curr` (longest
/// top-anchored run of equal plain-text lines).  `k` lines left the top.
fn detect_scroll(prev: &[Line], curr: &[Line]) -> usize {
    let mut best_k = 0usize;
    let mut best_len = 0usize;
    for k in 0..prev.len() {
        let mut p = 0usize;
        while k + p < prev.len() && p < curr.len() && prev[k + p].0 == curr[p].0 {
            p += 1;
        }
        if p > best_len {
            best_len = p;
            best_k = k;
        }
    }
    best_k
}

/// Convert a vt100 colour (+ bold) to a Slint colour.  Bold + a base colour
/// (0–7) maps to the bright variant (8–15), matching how terminals render
/// `ls --color` (e.g. bold-green executables, bold-blue directories).
fn vt_color_to_slint(color: vt100::Color, bold: bool) -> slint::Color {
    let (r, g, b) = match color {
        vt100::Color::Default => (0xd4, 0xd4, 0xd4), // Theme.term-fg
        vt100::Color::Idx(i) => idx_to_rgb(i, bold),
        vt100::Color::Rgb(r, g, b) => (r, g, b),
    };
    slint::Color::from_rgb_u8(r, g, b)
}

/// Convert a vt100 *background* colour to Slint.  The default background maps to
/// fully transparent so we don't paint a fill over the terminal's own bg (and
/// can cheaply skip drawing it).  Non-default backgrounds (btop/htop bars,
/// selected rows, meter fills) become opaque colours.
fn vt_bg_to_slint(color: vt100::Color) -> slint::Color {
    match color {
        vt100::Color::Default => slint::Color::from_argb_u8(0, 0, 0, 0), // transparent
        vt100::Color::Idx(i) => {
            let (r, g, b) = idx_to_rgb(i, false);
            slint::Color::from_rgb_u8(r, g, b)
        }
        vt100::Color::Rgb(r, g, b) => slint::Color::from_rgb_u8(r, g, b),
    }
}

/// Map an xterm-256 palette index to RGB (16 ANSI + 6×6×6 cube + grayscale).
fn idx_to_rgb(i: u8, bold: bool) -> (u8, u8, u8) {
    let i = if bold && i < 8 { i + 8 } else { i };
    match i {
        0..=15 => ANSI16[i as usize],
        16..=231 => {
            let n = i - 16;
            let to = |v: u8| -> u8 {
                if v == 0 {
                    0
                } else {
                    55 + v * 40
                }
            };
            (to(n / 36), to((n % 36) / 6), to(n % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    }
}
