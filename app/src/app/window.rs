use std::collections::HashMap;
use std::sync::Arc;

use egui_winit::winit;
use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::sync::RwLock;
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::{WindowBuilder, WindowId},
};

use crate::context::ContextState;

use super::{
    settings_menu::{SettingsUiResponse, SettingsWidget},
    AppMsg, AppType, AppWindow,
};

pub struct AppWindowState {
    pub title: String,
    pub(super) window: WindowState,
    pub(super) app: Box<dyn AppWindow>,
    pub(super) egui: EguiCtx,
}

impl AppWindowState {
    pub(super) fn sleep(self) -> AsleepWindow {
        AsleepWindow {
            title: self.title,
            app: self.app,
            egui: self.egui,
        }
    }

    pub(super) fn init(
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        title: &str,
        constructor: impl FnOnce(&WindowState) -> anyhow::Result<Box<dyn AppWindow>>,
    ) -> anyhow::Result<Self> {
        let window =
            WindowBuilder::new().with_title(title).build(event_loop)?;

        let win_state = state.prepare_window(window)?;

        let egui_ctx =
            EguiCtx::init(&state, win_state.surface_format, &event_loop, None);

        let app = constructor(&win_state)?;

        Ok(Self {
            title: title.to_string(),
            window: win_state,
            app,
            egui: egui_ctx,
        })
    }

    pub(super) fn resize(&mut self, state: &raving_wgpu::State) {
        self.window.resize(&state.device);
    }

    pub(super) fn on_event<'a>(&mut self, event: &WindowEvent<'a>) -> bool {
        let resp = self.egui.on_event(event);
        let mut consumed = resp.consumed;
        if !consumed {
            consumed = self
                .app
                .on_event(self.window.window.inner_size().into(), event);
        }
        consumed
    }

    pub(super) fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        context_state: &mut ContextState,
        dt: f32,
    ) {
        self.app.update(
            tokio_handle,
            state,
            &self.window,
            &mut self.egui,
            context_state,
            dt,
        );
    }

    pub(super) fn render(
        &mut self,
        state: &raving_wgpu::State,
    ) -> anyhow::Result<()> {
        let app = &mut self.app;
        let egui_ctx = &mut self.egui;
        let window = &mut self.window;

        if let Ok(output) = window.surface.get_current_texture() {
            let mut encoder = state.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some(&self.title),
                },
            );

            let output_view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let result = app.render(state, window, &output_view, &mut encoder);
            if let Err(e) = result {
                log::error!("Render error in window {}: {e:?}", &self.title);
            }
            egui_ctx.render(state, window, &output_view, &mut encoder);

            state.queue.submit(Some(encoder.finish()));
            output.present();
        } else {
            window.resize(&state.device);
        }

        Ok(())
    }
}

pub struct AsleepWindow {
    pub title: String,
    pub(super) app: Box<dyn AppWindow>,
    pub(super) egui: EguiCtx,
}

impl AsleepWindow {
    pub(super) fn wake(
        self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
    ) -> anyhow::Result<AppWindowState> {
        let window = WindowBuilder::new()
            .with_title(&self.title)
            .build(event_loop)?;

        let win_state = state.prepare_window(window)?;

        Ok(AppWindowState {
            title: self.title,
            window: win_state,
            app: self.app,
            egui: self.egui,
        })
    }
}

#[derive(Default, Clone)]
pub struct AppWindowsWidgetState {
    window_app_map: HashMap<WindowId, AppType>,
    window_wake_state: HashMap<AppType, WindowWakeState>,
}

impl SettingsWidget for AppWindowsWidgetState {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings_ctx: &super::settings_menu::SettingsUiContext,
    ) -> super::settings_menu::SettingsUiResponse {
        let mut windows = self
            .window_wake_state
            .iter()
            .map(|(app_ty, state)| {
                let title = match app_ty {
                    AppType::Viewer1D => "1D Viewer".to_string(),
                    AppType::Viewer2D => "2D Viewer".to_string(),
                    AppType::Custom(name) => name.to_string(),
                };
                (title, app_ty.clone(), state)
            })
            .collect::<Vec<_>>();

        windows.sort_by(|(t1, _, _), (t2, _, _)| t1.cmp(t2));

        let resp = ui.horizontal(|ui| {
            //
            for (label, app_ty, wake_state) in windows {
                let active = wake_state.is_awake();
                let btn = egui::SelectableLabel::new(active, label);

                if ui.add(btn).clicked() {
                    if active {
                        // sleep
                        settings_ctx.send_app_msg_task(AppMsg::WindowDelta(
                            WindowDelta::Close(app_ty),
                        ));
                    } else {
                        // wake
                        settings_ctx.send_app_msg_task(AppMsg::WindowDelta(
                            WindowDelta::Open(app_ty),
                        ));
                    }
                }
            }
        });

        SettingsUiResponse {
            response: resp.response,
        }
    }
}

#[derive(Default)]
pub struct AppWindows {
    pub(super) windows: HashMap<WindowId, AppType>,
    pub(super) apps: HashMap<AppType, AppWindowState>,
    pub(super) sleeping: HashMap<AppType, AsleepWindow>,

    pub(super) widget_state: Arc<RwLock<AppWindowsWidgetState>>,
}

impl AppWindows {
    pub(super) fn update_widget_state(&self) {
        let mut state = self.widget_state.blocking_write();
        self.windows.clone_into(&mut state.window_app_map);

        for (app_ty, app) in self.apps.iter() {
            let wake_state = WindowWakeState::Awake(app.window.window.id());
            state.window_wake_state.insert(app_ty.clone(), wake_state);
        }

        for (app_ty, _app) in self.sleeping.iter() {
            let wake_state = WindowWakeState::Sleeping;
            state.window_wake_state.insert(app_ty.clone(), wake_state);
        }
    }

    pub(super) fn handle_window_delta(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        delta: WindowDelta,
    ) -> anyhow::Result<()> {
        match delta {
            WindowDelta::Open(app_ty) => {
                if self.apps.contains_key(&app_ty) {
                    return Ok(());
                }

                let asleep = self.sleeping.remove(&app_ty).ok_or(
                    anyhow::anyhow!("Can't wake a window that's not asleep"),
                )?;
                let state = asleep.wake(event_loop, state)?;

                self.windows
                    .insert(state.window.window.id(), app_ty.clone());
                self.apps.insert(app_ty, state);

                Ok(())
            }
            WindowDelta::Close(app_ty) => {
                if let Some(win_id) =
                    self.apps.get(&app_ty).map(|s| s.window.window.id())
                {
                    if self.windows.len() == 1 {
                        anyhow::bail!("Can't close the only open window!");
                    }

                    let _app_ty = self.windows.remove(&win_id);
                    let app = self.apps.remove(&app_ty).unwrap();
                    self.sleeping.insert(app_ty, app.sleep());
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum WindowDelta {
    Open(super::AppType),
    Close(super::AppType),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WindowWakeState {
    // Uninitialized,
    Sleeping,
    Awake(WindowId),
}

impl WindowWakeState {
    pub fn is_awake(&self) -> bool {
        matches!(self, Self::Awake(_))
    }
}
