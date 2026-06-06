pub trait TerminalEngine {
    type Screen;

    fn ingest(&mut self, bytes: &[u8]);
    fn render(&mut self) -> Self::Screen;
    fn resize(&mut self, rows: u16, cols: u16);
}
