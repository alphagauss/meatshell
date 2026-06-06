/// A cursor-annotated terminal snapshot ready for a UI adapter.
pub struct BuiltScreen<Span> {
    pub spans: Vec<Span>,
    pub cursor_row: i32,
    pub cursor_col: i32,
    pub rows_used: i32,
    pub is_alt: bool,
}

/// One coloured run positioned on the terminal grid.
#[derive(Clone)]
pub struct RenderSpan {
    pub text: String,
    pub fg: slint::Color,
    pub bg: slint::Color,
    pub bold: bool,
    pub row: i32,
    pub col: i32,
    pub cells: i32,
}

/// One coloured run within a line; its grid row is assigned at render time.
#[derive(Clone)]
pub struct HistSpan {
    pub text: String,
    pub fg: slint::Color,
    pub bg: slint::Color,
    pub bold: bool,
    pub col: i32,
    pub cells: i32,
}

/// A rendered line: plain text (one char per cell, for find/selection) + runs.
pub type Line = (String, Vec<HistSpan>);
