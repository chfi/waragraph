use std::collections::HashMap;
use std::sync::Arc;

use raving_wgpu::{gui::EguiCtx, WindowState};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use super::AppWindow;

pub struct WindowHandler {
    active_window: usize,
    app_windows: HashMap<usize, Box<dyn AppWindow>>,
}

impl WindowHandler {
    pub fn init(
        apps: impl IntoIterator<Item = Box<dyn AppWindow>>,
    ) -> Option<Self> {
        let mut app_windows = HashMap::new();

        for (ix, app) in apps.into_iter().enumerate() {
            app_windows.insert(ix, app);
        }

        if app_windows.is_empty() {
            return None;
        }

        let active_window = 0;

        Some(Self {
            active_window,
            app_windows,
        })
    }

    pub fn add_window(&mut self, app: Box<dyn AppWindow>) {
        let ix = self.app_windows.len();
        self.app_windows.insert(ix, app);
    }

    pub fn add_windows_iter(
        &mut self,
        apps: impl IntoIterator<Item = Box<dyn AppWindow>>,
    ) {
        for app in apps {
            let ix = self.app_windows.len();
            self.app_windows.insert(ix, app);
        }
    }

    pub fn next_window(&mut self) {
        self.active_window = (self.active_window + 1) % self.app_windows.len()
    }

    pub fn prev_window(&mut self) {
        self.active_window = self
            .active_window
            .checked_sub(1)
            .unwrap_or(self.app_windows.len() - 1)
    }

    pub async fn run(
        mut self,
        tokio_rt: Arc<tokio::runtime::Runtime>,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
        mut window: raving_wgpu::WindowState,
    ) -> anyhow::Result<()> {
        let mut is_ready = false;
        let mut prev_frame_t = std::time::Instant::now();

        event_loop.run(move |event, _, control_flow| {
            let app = self.app_windows.get_mut(&self.active_window).unwrap();

            match &event {
                Event::Resumed => {
                    if !is_ready {
                        is_ready = true;
                    }
                }
                Event::WindowEvent { window_id, event } => {
                    let mut consumed = false;

                    let size = window.window.inner_size();
                    consumed = app.on_event(size.into(), event);

                    if !consumed {
                        match &event {
                            WindowEvent::KeyboardInput { input, .. } => {
                                use VirtualKeyCode as Key;

                                let pressed = matches!(
                                    input.state,
                                    ElementState::Pressed
                                );

                                if let Some(code) = input.virtual_keycode {
                                    if let Key::Escape = code {
                                        *control_flow = ControlFlow::Exit;
                                    } else if let Key::F1 = code {
                                        if pressed {
                                            self.next_window();
                                        }
                                    }
                                }
                            }
                            WindowEvent::CloseRequested => {
                                *control_flow = ControlFlow::Exit
                            }
                            WindowEvent::Resized(phys_size) => {
                                let old_size = window.size;

                                // for some reason i get a validation error if i actually attempt
                                // to execute the first resize
                                // NB: i think there's another event I should wait on before
                                // trying to do any rendering and/or resizing
                                if is_ready {
                                    window.resize(&state.device);
                                }

                                let new_size = window.window.inner_size();

                                app.resize(
                                    &state,
                                    old_size.into(),
                                    new_size.into(),
                                )
                                .unwrap();
                            }
                            WindowEvent::ScaleFactorChanged {
                                new_inner_size,
                                ..
                            } => {
                                if is_ready {
                                    window.resize(&state.device);
                                }
                            }
                            _ => {}
                        }
                    }
                }

                Event::RedrawRequested(window_id)
                    if *window_id == window.window.id() =>
                {
                    app.render(&state, &mut window).unwrap();
                }
                Event::MainEventsCleared => {
                    let dt = prev_frame_t.elapsed().as_secs_f32();
                    prev_frame_t = std::time::Instant::now();

                    app.update(tokio_rt.handle(), &state, &window, dt);

                    window.window.request_redraw();
                }

                _ => {}
            }
        })
    }
}
