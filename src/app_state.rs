#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BottomPanelTab {
    Files,
    Tunnels,
}

impl BottomPanelTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Tunnels => "tunnels",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "files" => Some(Self::Files),
            "tunnels" => Some(Self::Tunnels),
            _ => None,
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

impl AppState {
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_bottom_panel(&mut self) {
        self.bottom_panel_visible = !self.bottom_panel_visible;
    }

    pub fn select_bottom_panel_tab(&mut self, tab: BottomPanelTab) {
        self.bottom_panel_tab = tab;
        self.bottom_panel_visible = true;
    }
}
