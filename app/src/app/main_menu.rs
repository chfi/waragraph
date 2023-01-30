use std::collections::BTreeMap;

use crossbeam::atomic::AtomicCell;
use egui::Ui;
use winit::window::WindowId;

use super::{AppType, AppWindow};

pub fn main_menu_id() -> egui::Id {
    egui::Id::new("_MainMenu_")
}

pub struct MainMenuCtx {
    // panels: BTreeMap<String, ()>,
}

impl MainMenuCtx {
    // pub fn new(windows: impl IntoIterator<Item = (AppType, String)>) -> Self {
    //     Self {
    //         all_windows: windows.into_iter().collect(),
    //     }
    // }
}

// pub struct MainMenuUi<'a> {
pub struct MainMenuUi {
    window_list: Vec<(AppType, String, bool)>,
}

#[derive(Default)]
pub struct MainMenuOutput {
    window_deltas: Vec<WindowDelta>,
}

impl MainMenuUi {
    fn new(windows: impl IntoIterator<Item = (AppType, String, bool)>) -> Self {
        Self {
            window_list: windows.into_iter().collect(),
        }
    }

    fn show(self, ui: &mut Ui) -> MainMenuOutput {
        let mut out = MainMenuOutput::default();

        ui.horizontal_wrapped(|ui| {
            for (app_ty, title, awake) in self.window_list.iter() {
                ui.label(title);
                let button_text = if *awake { "Close" } else { "Open" };

                if ui.button(button_text).clicked() {
                    if *awake {
                        out.window_deltas
                            .push(WindowDelta::Close(app_ty.clone()));
                    } else {
                        out.window_deltas
                            .push(WindowDelta::Open(app_ty.clone()));
                    }
                }

                ui.end_row();
            }
        });

        out
    }
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

pub struct MainMenuApp {
    //
}

impl AppWindow for MainMenuApp {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut raving_wgpu::gui::EguiCtx,
        dt: f32,
    ) {
        egui_ctx.begin_frame(&window.window);

        let [w, h]: [u32; 2] = window.window.inner_size().into();

        let size = egui::vec2(w as f32, h as f32);

        egui::Window::new("Main Menu")
            .fixed_size(size)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(egui_ctx.ctx(), |ui| {
                ui.add(egui::Label::new(
                    egui::RichText::new("Main Menu")
                        .font(egui::FontId::proportional(20.0)),
                ));

                ui.add(egui::Separator::default().horizontal());

                //
            });

        egui_ctx.end_frame(&window.window);
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        false
    }

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
