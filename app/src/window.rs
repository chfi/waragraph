use std::collections::HashMap;

use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use crate::AppWindow;

pub struct WindowHandler {
    active_window: usize,
    app_windows: HashMap<usize, Box<dyn AppWindow>>,
}

impl WindowHandler {
    pub fn init(
        viewer_1d: Option<Box<dyn AppWindow>>,
        viewer_2d: Option<Box<dyn AppWindow>>,
    ) -> Option<Self> {
        let mut app_windows = HashMap::new();

        if let Some(app) = viewer_1d {
            app_windows.insert(0, app);
        }

        if let Some(app) = viewer_2d {
            app_windows.insert(1, app);
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
        event_loop: EventLoop<()>,
        window: winit::window::Window,
        mut state: raving_wgpu::State,
    ) -> anyhow::Result<()> {
        let mut first_resize = true;
        let mut prev_frame_t = std::time::Instant::now();

        event_loop.run(move |event, _, control_flow| {
            let app = self.app_windows.get_mut(&self.active_window).unwrap();

            match &event {
                Event::WindowEvent { window_id, event } => {
                    let mut consumed = false;

                    let size = window.inner_size();
                    consumed = app.on_event([size.width, size.height], event);

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
                                let old_size = state.size;

                                // for some reason i get a validation error if i actually attempt
                                // to execute the first resize
                                // NB: i think there's another event I should wait on before
                                // trying to do any rendering and/or resizing
                                if first_resize {
                                    first_resize = false;
                                } else {
                                    state.resize(*phys_size);
                                }

                                let new_size = window.inner_size();

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
                                state.resize(**new_inner_size);
                            }
                            _ => {}
                        }
                    }
                }

                Event::RedrawRequested(window_id)
                    if *window_id == window.id() =>
                {
                    app.render(&mut state).unwrap();
                }
                Event::MainEventsCleared => {
                    let dt = prev_frame_t.elapsed().as_secs_f32();
                    prev_frame_t = std::time::Instant::now();

                    app.update(&state, &window, dt);

                    window.request_redraw();
                }

                _ => {}
            }
        })
    }

    /*
        pub async fn run(
            &mut self,
        event_loop: EventLoop<()>,
        window: winit::window::Window,
        mut state: raving_wgpu::State,
        active_window: usize,
    ) -> Result<()> {

        let mut first_resize = true;
        let mut prev_frame_t = std::time::Instant::now();

        event_loop.run(move |event, _, control_flow| {
            match &event {
                Event::WindowEvent { window_id, event } => {
                    let mut consumed = false;

                    let size = window.inner_size();
                    consumed = app.on_event([size.width, size.height], event);

                    if !consumed {
                        match &event {
                            WindowEvent::KeyboardInput { input, .. } => {
                                use VirtualKeyCode as Key;
                                if let Some(code) = input.virtual_keycode {
                                    if let Key::Escape = code {
                                        *control_flow = ControlFlow::Exit;
                                    }
                                }
                            }
                            WindowEvent::CloseRequested => {
                                *control_flow = ControlFlow::Exit
                            }
                            WindowEvent::Resized(phys_size) => {
                                let old_size = state.size;

                                // for some reason i get a validation error if i actually attempt
                                // to execute the first resize
                                // NB: i think there's another event I should wait on before
                                // trying to do any rendering and/or resizing
                                if first_resize {
                                    first_resize = false;
                                } else {
                                    state.resize(*phys_size);
                                }

                                let new_size = window.inner_size();

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
                                state.resize(**new_inner_size);
                            }
                            _ => {}
                        }
                    }
                }

                Event::RedrawRequested(window_id) if *window_id == window.id() => {
                    app.render(&mut state).unwrap();
                }
                Event::MainEventsCleared => {
                    let dt = prev_frame_t.elapsed().as_secs_f32();
                    prev_frame_t = std::time::Instant::now();

                    app.update(&state, &window, dt);

                    window.request_redraw();
                }

                _ => {}
            }
        }
                       */
}
