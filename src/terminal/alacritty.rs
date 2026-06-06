use std::cell::{Ref, RefCell};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};

use super::engine::{TerminalEngine, TerminalEngineMode};
use super::types::{BuiltScreen, RenderSpan};

const DEFAULT_FG: (u8, u8, u8) = (0xd4, 0xd4, 0xd4);
const DEFAULT_BG: (u8, u8, u8) = (0x0e, 0x0f, 0x13);
const DIM_FG: (u8, u8, u8) = (0x88, 0x88, 0x88);

struct AlacrittyDimensions {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for AlacrittyDimensions {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

pub struct AlacrittyTerminalEngine {
    term: Term<VoidListener>,
    parser: Processor,
    displayed_text: RefCell<Vec<String>>,
}

impl AlacrittyTerminalEngine {
    pub fn new(rows: u16, cols: u16) -> Self {
        let dimensions = AlacrittyDimensions {
            columns: cols.max(1) as usize,
            screen_lines: rows.max(1) as usize,
        };
        Self {
            term: Term::new(Config::default(), &dimensions, VoidListener),
            parser: Processor::new(),
            displayed_text: RefCell::new(Vec::new()),
        }
    }

    pub fn displayed_text(&self) -> Ref<'_, Vec<String>> {
        self.displayed_text.borrow()
    }
}

impl TerminalEngine for AlacrittyTerminalEngine {
    fn mode(&self) -> TerminalEngineMode {
        TerminalEngineMode::Alacritty
    }

    fn ingest(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn render(&self) -> BuiltScreen<RenderSpan> {
        let renderable = self.term.renderable_content();
        let colors = renderable.colors;
        let cursor = renderable.cursor;
        let mode = renderable.mode;
        let rows = self.term.screen_lines();
        let cols = self.term.columns();
        let mut cells_by_row = (0..rows)
            .map(|_| Vec::with_capacity(cols))
            .collect::<Vec<Vec<CellAttrs>>>();

        for indexed in renderable.display_iter {
            let row = indexed.point.line.0;
            if row < 0 {
                continue;
            }
            let row = row as usize;
            if row >= rows {
                continue;
            }
            cells_by_row[row].push(cell_to_attrs(indexed.cell, colors));
        }

        let mut spans = Vec::new();
        let mut displayed = Vec::with_capacity(rows);
        let mut last_content = -1i32;

        for (row_idx, row_cells) in cells_by_row.iter().enumerate() {
            let mut plain = String::with_capacity(cols);
            let mut pending = None;
            let mut has_non_default_background = false;

            for (col_idx, attrs) in row_cells.iter().enumerate() {
                plain.push_str(&attrs.plain_text);
                has_non_default_background |= !attrs.bg_is_default;
                push_span(
                    &mut spans,
                    &mut pending,
                    row_idx as i32,
                    col_idx as i32,
                    attrs,
                );
            }
            finish_span(&mut spans, pending);

            if !plain.trim_end_matches(' ').is_empty() || has_non_default_background {
                last_content = row_idx as i32;
            }
            displayed.push(plain.trim_end_matches(' ').to_string());
        }

        self.displayed_text.replace(displayed);

        let (cursor_row, cursor_col) = if cursor.shape == CursorShape::Hidden {
            (-1, 0)
        } else {
            (cursor.point.line.0, cursor.point.column.0 as i32)
        };

        BuiltScreen {
            spans,
            cursor_row,
            cursor_col,
            rows_used: if mode.contains(TermMode::ALT_SCREEN) {
                rows as i32
            } else {
                last_content + 1
            },
            is_alt: mode.contains(TermMode::ALT_SCREEN),
            mouse_reporting: self.mouse_reporting(),
        }
    }

    fn resize(&mut self, rows: usize, cols: usize) {
        self.term.resize(AlacrittyDimensions {
            columns: cols.max(1),
            screen_lines: rows.max(1),
        });
    }

    fn mouse_reporting(&self) -> bool {
        let mode = self.term.mode();
        mode.intersects(TermMode::MOUSE_MODE) && mode.contains(TermMode::SGR_MOUSE)
    }

    fn application_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }
}

#[derive(Clone, Debug)]
struct CellAttrs {
    plain_text: String,
    render_text: String,
    fg: slint::Color,
    bg: slint::Color,
    bold: bool,
    bg_is_default: bool,
}

#[derive(Debug)]
struct PendingSpan {
    text: String,
    fg: slint::Color,
    bg: slint::Color,
    bold: bool,
    row: i32,
    col: i32,
    cells: i32,
    bg_is_default: bool,
}

fn cell_to_attrs(cell: &Cell, palette: &Colors) -> CellAttrs {
    let inverse = cell.flags.contains(Flags::INVERSE);
    let bold = cell.flags.contains(Flags::BOLD);
    let hidden = cell.flags.contains(Flags::HIDDEN);
    let wide_placeholder = should_skip_wide_placeholder(cell.flags);
    let mut fg = cell.fg;
    let mut bg = cell.bg;
    if inverse {
        std::mem::swap(&mut fg, &mut bg);
    }

    let plain_text = if hidden || wide_placeholder {
        " ".to_string()
    } else {
        cell.c.to_string()
    };

    let render_text = if hidden || wide_placeholder {
        String::new()
    } else {
        let mut text = cell.c.to_string();
        if let Some(extra) = cell.zerowidth() {
            text.extend(extra.iter());
        }
        text
    };

    CellAttrs {
        plain_text,
        render_text,
        fg: convert_color(fg, palette, false, bold),
        bg: convert_color(bg, palette, true, false),
        bold,
        bg_is_default: matches!(bg, Color::Named(NamedColor::Background)),
    }
}

fn should_skip_wide_placeholder(flags: Flags) -> bool {
    flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
}

fn push_span(
    spans: &mut Vec<RenderSpan>,
    pending: &mut Option<PendingSpan>,
    row: i32,
    col: i32,
    attrs: &CellAttrs,
) {
    if let Some(current) = pending.as_mut() {
        if current.row == row
            && current.col + current.cells == col
            && current.fg == attrs.fg
            && current.bg == attrs.bg
            && current.bold == attrs.bold
            && current.bg_is_default == attrs.bg_is_default
        {
            current.cells += 1;
            current.text.push_str(&attrs.render_text);
            return;
        }
    }

    finish_span(spans, pending.take());
    *pending = Some(PendingSpan {
        text: attrs.render_text.clone(),
        fg: attrs.fg,
        bg: attrs.bg,
        bold: attrs.bold,
        row,
        col,
        cells: 1,
        bg_is_default: attrs.bg_is_default,
    });
}

fn finish_span(spans: &mut Vec<RenderSpan>, pending: Option<PendingSpan>) {
    let Some(span) = pending else {
        return;
    };
    if span.bg_is_default && span.text.chars().all(|ch| ch == ' ') {
        return;
    }
    spans.push(RenderSpan {
        text: span.text,
        fg: span.fg,
        bg: span.bg,
        bold: span.bold,
        row: span.row,
        col: span.col,
        cells: span.cells,
    });
}

fn convert_color(color: Color, palette: &Colors, background: bool, bold: bool) -> slint::Color {
    match color {
        Color::Spec(rgb) => rgb_color_to_slint(rgb),
        Color::Indexed(index) => palette[index as usize]
            .map(rgb_color_to_slint)
            .unwrap_or_else(|| tuple_rgb_to_slint(idx_to_rgb(index, bold))),
        Color::Named(named) => convert_named_color(named, palette, background, bold),
    }
}

fn convert_named_color(
    named: NamedColor,
    palette: &Colors,
    background: bool,
    bold: bool,
) -> slint::Color {
    if background && named == NamedColor::Background {
        return slint::Color::from_argb_u8(0, 0, 0, 0);
    }

    if let Some(rgb) = palette[named] {
        return rgb_color_to_slint(rgb);
    }

    let rgb = match named {
        NamedColor::Foreground | NamedColor::BrightForeground => DEFAULT_FG,
        NamedColor::DimForeground => DIM_FG,
        NamedColor::Background => DEFAULT_BG,
        NamedColor::Cursor => DEFAULT_FG,
        NamedColor::Black | NamedColor::DimBlack => idx_to_rgb(0, bold),
        NamedColor::Red | NamedColor::DimRed => idx_to_rgb(1, bold),
        NamedColor::Green | NamedColor::DimGreen => idx_to_rgb(2, bold),
        NamedColor::Yellow | NamedColor::DimYellow => idx_to_rgb(3, bold),
        NamedColor::Blue | NamedColor::DimBlue => idx_to_rgb(4, bold),
        NamedColor::Magenta | NamedColor::DimMagenta => idx_to_rgb(5, bold),
        NamedColor::Cyan | NamedColor::DimCyan => idx_to_rgb(6, bold),
        NamedColor::White | NamedColor::DimWhite => idx_to_rgb(7, bold),
        NamedColor::BrightBlack => idx_to_rgb(8, false),
        NamedColor::BrightRed => idx_to_rgb(9, false),
        NamedColor::BrightGreen => idx_to_rgb(10, false),
        NamedColor::BrightYellow => idx_to_rgb(11, false),
        NamedColor::BrightBlue => idx_to_rgb(12, false),
        NamedColor::BrightMagenta => idx_to_rgb(13, false),
        NamedColor::BrightCyan => idx_to_rgb(14, false),
        NamedColor::BrightWhite => idx_to_rgb(15, false),
    };
    tuple_rgb_to_slint(rgb)
}

fn tuple_rgb_to_slint((r, g, b): (u8, u8, u8)) -> slint::Color {
    slint::Color::from_rgb_u8(r, g, b)
}

fn rgb_color_to_slint(rgb: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(rgb.r, rgb.g, rgb.b)
}

const ANSI16: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00),
    (0xcd, 0x31, 0x31),
    (0x0d, 0xbc, 0x79),
    (0xe5, 0xe5, 0x10),
    (0x24, 0x72, 0xc8),
    (0xbc, 0x3f, 0xbc),
    (0x11, 0xa8, 0xcd),
    (0xe5, 0xe5, 0xe5),
    (0x66, 0x66, 0x66),
    (0xf1, 0x4c, 0x4c),
    (0x23, 0xd1, 0x8b),
    (0xf5, 0xf5, 0x43),
    (0x3b, 0x8e, 0xea),
    (0xd6, 0x70, 0xd6),
    (0x29, 0xb8, 0xdb),
    (0xff, 0xff, 0xff),
];

fn idx_to_rgb(index: u8, bold: bool) -> (u8, u8, u8) {
    let index = if bold && index < 8 { index + 8 } else { index };
    match index {
        0..=15 => ANSI16[index as usize],
        16..=231 => {
            let n = index - 16;
            let to = |v: u8| -> u8 {
                if v == 0 {
                    0
                } else {
                    55 + 40 * v
                }
            };
            (to(n / 36), to((n % 36) / 6), to(n % 6))
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(
        plain_text: &str,
        render_text: &str,
        fg: slint::Color,
        bg: slint::Color,
        bold: bool,
        bg_is_default: bool,
    ) -> CellAttrs {
        CellAttrs {
            plain_text: plain_text.to_string(),
            render_text: render_text.to_string(),
            fg,
            bg,
            bold,
            bg_is_default,
        }
    }

    #[test]
    fn indexed_bold_uses_bright_palette() {
        let color = convert_color(Color::Indexed(1), &Colors::default(), false, true);
        assert_eq!(color, slint::Color::from_rgb_u8(0xf1, 0x4c, 0x4c));
    }

    #[test]
    fn skips_both_wide_placeholder_flag_variants() {
        assert!(should_skip_wide_placeholder(Flags::WIDE_CHAR_SPACER));
        assert!(should_skip_wide_placeholder(
            Flags::LEADING_WIDE_CHAR_SPACER
        ));
        assert!(!should_skip_wide_placeholder(Flags::WIDE_CHAR));
    }

    #[test]
    fn wide_placeholder_extends_span_without_duplicate_text() {
        let fg = slint::Color::from_rgb_u8(1, 2, 3);
        let bg = slint::Color::from_argb_u8(0, 0, 0, 0);
        let mut spans = Vec::new();
        let mut pending = None;

        push_span(
            &mut spans,
            &mut pending,
            0,
            0,
            &attrs("中", "中", fg, bg, false, true),
        );
        push_span(
            &mut spans,
            &mut pending,
            0,
            1,
            &attrs(" ", "", fg, bg, false, true),
        );
        finish_span(&mut spans, pending.take());

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "中");
        assert_eq!(spans[0].cells, 2);
    }

    #[test]
    fn blank_default_background_span_is_dropped() {
        let fg = slint::Color::from_rgb_u8(1, 2, 3);
        let bg = slint::Color::from_argb_u8(0, 0, 0, 0);
        let mut spans = Vec::new();
        let mut pending = None;

        push_span(
            &mut spans,
            &mut pending,
            0,
            0,
            &attrs(" ", " ", fg, bg, false, true),
        );
        push_span(
            &mut spans,
            &mut pending,
            0,
            1,
            &attrs(" ", " ", fg, bg, false, true),
        );
        finish_span(&mut spans, pending.take());

        assert!(spans.is_empty());
    }
}
