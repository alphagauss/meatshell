use std::cell::{Ref, RefCell};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column as AColumn, Line as ALine};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};

use super::engine::{TerminalEngine, TerminalEngineMode};
use super::types::{BuiltScreen, RenderSpan};

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
        let rows = self.term.screen_lines();
        let cols = self.term.columns();
        let cursor = self.term.grid().cursor.point;
        let mut spans = Vec::new();
        let mut displayed = Vec::with_capacity(rows);
        let mut last_content = 0i32;
        let grid = self.term.grid();

        for row_idx in 0..rows {
            let line = &grid[ALine(row_idx as i32)];
            let mut plain = String::with_capacity(cols);
            let mut col = 0usize;
            while col < cols {
                let cell = &line[AColumn(col)];
                let attrs = cell_attrs(cell);
                let start_col = col;
                let mut text = attrs.text.clone();
                plain.push_str(&attrs.text);
                col += 1;
                while col < cols {
                    let next = cell_attrs(&line[AColumn(col)]);
                    if next.fg != attrs.fg
                        || next.bg != attrs.bg
                        || next.bold != attrs.bold
                        || next.hidden != attrs.hidden
                    {
                        break;
                    }
                    plain.push_str(&next.text);
                    text.push_str(&next.text);
                    col += 1;
                }
                let is_blank = text.chars().all(|ch| ch == ' ');
                if !(is_blank && attrs.bg_is_default) {
                    last_content = row_idx as i32;
                    spans.push(RenderSpan {
                        text,
                        fg: attrs.fg,
                        bg: attrs.bg,
                        bold: attrs.bold,
                        row: row_idx as i32,
                        col: start_col as i32,
                        cells: (col - start_col) as i32,
                    });
                }
            }
            displayed.push(plain.trim_end().to_string());
        }

        self.displayed_text.replace(displayed);
        BuiltScreen {
            spans,
            cursor_row: cursor.line.0,
            cursor_col: cursor.column.0 as i32,
            rows_used: if self.term.mode().contains(TermMode::ALT_SCREEN) {
                rows as i32
            } else {
                last_content + 1
            },
            is_alt: self.term.mode().contains(TermMode::ALT_SCREEN),
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

struct CellAttrs {
    text: String,
    fg: slint::Color,
    bg: slint::Color,
    bold: bool,
    hidden: bool,
    bg_is_default: bool,
}

fn cell_attrs(cell: &Cell) -> CellAttrs {
    let inverse = cell.flags.contains(Flags::INVERSE);
    let bold = cell.flags.contains(Flags::BOLD);
    let hidden = cell.flags.contains(Flags::HIDDEN);
    let mut fg = cell.fg;
    let mut bg = cell.bg;
    if inverse {
        std::mem::swap(&mut fg, &mut bg);
    }
    let mut text = if hidden || cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
        " ".to_string()
    } else {
        cell.c.to_string()
    };
    if let Some(extra) = cell.zerowidth() {
        text.extend(extra.iter());
    }
    CellAttrs {
        text,
        fg: color_to_slint(fg, false, bold),
        bg: color_to_slint(bg, true, false),
        bold,
        hidden,
        bg_is_default: matches!(bg, Color::Named(NamedColor::Background)),
    }
}

fn color_to_slint(color: Color, background: bool, bold: bool) -> slint::Color {
    match color {
        Color::Spec(Rgb { r, g, b }) => slint::Color::from_rgb_u8(r, g, b),
        Color::Indexed(index) => {
            let (r, g, b) = idx_to_rgb(index, bold);
            slint::Color::from_rgb_u8(r, g, b)
        }
        Color::Named(NamedColor::Foreground) | Color::Named(NamedColor::BrightForeground) => {
            slint::Color::from_rgb_u8(0xd4, 0xd4, 0xd4)
        }
        Color::Named(NamedColor::DimForeground) => slint::Color::from_rgb_u8(0x88, 0x88, 0x88),
        Color::Named(NamedColor::Background) if background => {
            slint::Color::from_argb_u8(0, 0, 0, 0)
        }
        Color::Named(NamedColor::Background) => slint::Color::from_rgb_u8(0xd4, 0xd4, 0xd4),
        Color::Named(named) => {
            let (r, g, b) = named_to_rgb(named, bold);
            slint::Color::from_rgb_u8(r, g, b)
        }
    }
}

fn named_to_rgb(named: NamedColor, bold: bool) -> (u8, u8, u8) {
    let index = match named {
        NamedColor::Black | NamedColor::DimBlack => 0,
        NamedColor::Red | NamedColor::DimRed => 1,
        NamedColor::Green | NamedColor::DimGreen => 2,
        NamedColor::Yellow | NamedColor::DimYellow => 3,
        NamedColor::Blue | NamedColor::DimBlue => 4,
        NamedColor::Magenta | NamedColor::DimMagenta => 5,
        NamedColor::Cyan | NamedColor::DimCyan => 6,
        NamedColor::White | NamedColor::DimWhite => 7,
        NamedColor::BrightBlack => 8,
        NamedColor::BrightRed => 9,
        NamedColor::BrightGreen => 10,
        NamedColor::BrightYellow => 11,
        NamedColor::BrightBlue => 12,
        NamedColor::BrightMagenta => 13,
        NamedColor::BrightCyan => 14,
        NamedColor::BrightWhite => 15,
        _ => 7,
    };
    idx_to_rgb(index, bold)
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
