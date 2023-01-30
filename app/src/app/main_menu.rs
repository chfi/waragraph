use std::collections::BTreeMap;

use crossbeam::atomic::AtomicCell;
use winit::window::WindowId;

pub fn main_menu_id() -> egui::Id {
    egui::Id::new("_MainMenu_")
}

pub struct MainMenuCtx {
    panels: BTreeMap<String, ()>,
}

// pub struct MainMenuUi<'a> {
pub struct MainMenuUi {
    window_list: Vec<(WindowId, String)>,
    //
}

#[derive(Debug, Clone)]
pub struct MainMenuResponse {
    window_delta: Option<WindowDelta>,
}

pub struct MainMenuState {
    active_panel: Option<String>,
    //
}

#[derive(Debug, Clone)]
pub enum WindowDelta {
    Open(super::AppType),
    Close(super::AppType),
}

pub(crate) fn test_window_toggler(
    ui: &mut egui::Ui,
    cell: &AtomicCell<Option<WindowDelta>>,
    label: &str,
    app_ty: super::AppType,
) {
    if ui.button(&format!("Open {label}")).clicked() {
        cell.store(Some(crate::app::main_menu::WindowDelta::Open(
            app_ty.clone(),
        )));
    }

    if ui.button(&format!("Close {label}")).clicked() {
        cell.store(Some(crate::app::main_menu::WindowDelta::Close(app_ty)));
    }
}
