use std::collections::HashMap;
use std::sync::Arc;

use raving_wgpu::{gui::EguiCtx, WindowState};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::WindowBuilder,
};

use super::AppWindow;

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
        dt: f32,
    ) {
        self.app
            .update(tokio_handle, state, &self.window, &mut self.egui, dt);
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
