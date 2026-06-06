#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalEngineMode {
    Legacy,
    AlacrittyExperimental,
}

impl TerminalEngineMode {
    pub fn from_env() -> Self {
        match std::env::var("MEATSHELL_TERMINAL_ENGINE") {
            Ok(value) if value.eq_ignore_ascii_case("alacritty") => Self::AlacrittyExperimental,
            _ => Self::Legacy,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::AlacrittyExperimental => "alacritty",
        }
    }
}

pub trait TerminalEngine {
    type Screen;

    fn ingest(&mut self, bytes: &[u8]);
    fn render(&mut self) -> Self::Screen;
    fn resize(&mut self, rows: u16, cols: u16);
}
