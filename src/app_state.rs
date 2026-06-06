#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BottomPanelTab {
    Files,
    #[allow(dead_code)]
    Tunnels,
}

impl BottomPanelTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Tunnels => "tunnels",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub sidebar_visible: bool,
    pub bottom_panel_visible: bool,
    pub bottom_panel_tab: BottomPanelTab,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sidebar_visible: true,
            bottom_panel_visible: true,
            bottom_panel_tab: BottomPanelTab::Files,
        }
    }
}
