use super::types::{BuiltScreen, RenderSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalEngineMode {
    Legacy,
    Alacritty,
}

impl TerminalEngineMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Alacritty => "alacritty",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "alacritty" | "alacritty-experimental" | "experimental" => Self::Alacritty,
            _ => Self::Legacy,
        }
    }
}

pub trait TerminalEngine {
    fn mode(&self) -> TerminalEngineMode;
    fn ingest(&mut self, bytes: &[u8]);
    fn render(&self) -> BuiltScreen<RenderSpan>;
    fn resize(&mut self, rows: usize, cols: usize);

    fn mouse_reporting(&self) -> bool {
        false
    }

    fn application_cursor(&self) -> bool {
        false
    }

    fn bracketed_paste(&self) -> bool {
        false
    }
}
