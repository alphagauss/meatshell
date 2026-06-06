use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use super::state::{AppState, BottomPanelTab};

use super::AppWindow;

pub(super) fn sync_app_state_to_window(win: &AppWindow, state: &AppState) {
    win.set_sidebar_visible(state.sidebar_visible);
    win.set_bottom_panel_visible(state.bottom_panel_visible);
    win.set_bottom_panel_tab(state.bottom_panel_tab.as_str().into());
}

pub(super) fn wire_layout_callbacks(window: &AppWindow, app_state: Rc<RefCell<AppState>>) {
    {
        let weak = window.as_weak();
        let app_state = app_state.clone();
        window.on_toggle_sidebar(move || {
            let Some(w) = weak.upgrade() else { return };
            let mut state = app_state.borrow_mut();
            state.toggle_sidebar();
            sync_app_state_to_window(&w, &state);
        });
    }

    {
        let weak = window.as_weak();
        let app_state = app_state.clone();
        window.on_toggle_bottom_panel(move || {
            let Some(w) = weak.upgrade() else { return };
            let mut state = app_state.borrow_mut();
            state.toggle_bottom_panel();
            sync_app_state_to_window(&w, &state);
        });
    }

    {
        let weak = window.as_weak();
        let app_state = app_state.clone();
        window.on_select_bottom_panel_tab(move |tab: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let Some(tab) = BottomPanelTab::from_str(tab.as_str()) else {
                return;
            };
            let mut state = app_state.borrow_mut();
            state.select_bottom_panel_tab(tab);
            sync_app_state_to_window(&w, &state);
        });
    }
}
