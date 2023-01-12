mod window;

use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::runtime::Runtime;
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, WindowId},
};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::viewer_1d::Viewer1D;

pub struct SharedState {
    gfa_path: Arc<PathBuf>,
    tsv_path: Option<Arc<PathBuf>>,

    graph: Arc<waragraph_core::graph::PathIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppType {
    Viewer1D,
    Viewer2D,
}

pub struct AppWindowState {
    // window: Option<WindowId>,
    title: String,
    window: WindowState,
    app: Box<dyn AppWindow>,
    egui: EguiCtx,
}

impl AppWindowState {
    fn init(
        event_loop: &EventLoop<()>,
        state: &raving_wgpu::State,
        title: &str,
        constructor: impl FnOnce(&WindowState) -> Result<Box<dyn AppWindow>>,
        // app: Box<dyn AppWindow>,
    ) -> Result<Self> {
        let window =
            WindowBuilder::new().with_title(title).build(event_loop)?;

        let win_state = state.prepare_window(window)?;

        let egui_ctx = EguiCtx::init(
            &state,
            win_state.surface_format,
            &event_loop,
            None,
            // Some(wgpu::Color::BLACK),
        );

        let app = constructor(&win_state)?;

        Ok(Self {
            title: title.to_string(),
            window: win_state,
            app,
            egui: egui_ctx,
        })
    }

    fn resize(&mut self, state: &raving_wgpu::State) {
        self.window.resize(&state.device);
    }

    fn on_event<'a>(&mut self, event: &WindowEvent<'a>) -> bool {
        let resp = self.egui.on_event(event);
        resp.consumed
    }

    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        dt: f32,
    ) {
        self.app
            .update(tokio_handle, state, &self.window, &mut self.egui, dt);
    }

    fn render(&mut self, state: &raving_wgpu::State) -> anyhow::Result<()> {
        let app = &mut self.app;
        let egui_ctx = &mut self.egui;
        let window = &mut self.window;

        if let Ok(output) = window.surface.get_current_texture() {
            let output_view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = state.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some(&self.title),
                },
            );

            let output_view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let result = app.render(state, window, &output_view, &mut encoder);
            egui_ctx.render(state, window, &output_view, &mut encoder);

            state.queue.submit(Some(encoder.finish()));
            output.present();
        } else {
            window.resize(&state.device);
        }

        Ok(())
    }
}

pub struct NewApp {
    pub tokio_rt: Arc<Runtime>,
    pub shared: SharedState,

    pub windows: HashMap<WindowId, AppType>,
    pub apps: HashMap<AppType, AppWindowState>,
}

impl NewApp {
    pub fn init(args: Args) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("waragraph-tokio")
            .build()?;

        let tokio_rt = Arc::new(runtime);

        let path_index = waragraph_core::graph::PathIndex::from_gfa(&args.gfa)?;
        let path_index = Arc::new(path_index);

        let shared = {
            let gfa_path = Arc::new(args.gfa);
            let tsv_path = args.tsv.map(|p| Arc::new(p));

            SharedState {
                gfa_path,
                tsv_path,
                graph: path_index,
            }
        };

        Ok(Self {
            tokio_rt,
            shared,

            windows: HashMap::default(),
            apps: HashMap::default(),
        })
    }

    pub fn init_viewer_1d(
        &mut self,
        event_loop: &EventLoop<()>,
        state: &raving_wgpu::State,
    ) -> Result<()> {
        let title = "Waragraph 1D";

        let app = AppWindowState::init(event_loop, state, title, |window| {
            let dims: [u32; 2] = window.window.inner_size().into();

            let app = Viewer1D::init(
                event_loop,
                dims,
                state,
                &window,
                self.shared.graph.clone(),
            )?;

            Ok(Box::new(app))
        })?;

        let winid = app.window.window.id();

        self.apps.insert(AppType::Viewer1D, app);
        self.windows.insert(winid, AppType::Viewer1D);

        Ok(())
    }

    pub fn run(
        mut self,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
    ) -> Result<()> {
        let mut is_ready = false;
        let mut prev_frame_t = std::time::Instant::now();

        event_loop.run(move |event, _, control_flow| match &event {
            Event::Resumed => {
                if !is_ready {
                    is_ready = true;
                }
            }
            Event::WindowEvent { window_id, event } => {
                println!("window_id: {window_id:?}");
                let app_type = self.windows.get(&window_id);
                if app_type.is_none() {
                    return;
                }
                let app_type = app_type.unwrap();
                let app = self.apps.get_mut(app_type).unwrap();

                let size = app.window.window.inner_size();

                let mut consumed = app.on_event(event);

                if !consumed {
                    match &event {
                        WindowEvent::KeyboardInput { input, .. } => {
                            use VirtualKeyCode as Key;

                            let pressed =
                                matches!(input.state, ElementState::Pressed);

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
                            if is_ready {
                                app.resize(&state);
                                app.app
                                    .on_resize(
                                        &state,
                                        app.window.size.into(),
                                        (*phys_size).into(),
                                    )
                                    .unwrap();
                            }
                        }
                        WindowEvent::ScaleFactorChanged {
                            new_inner_size,
                            ..
                        } => {
                            if is_ready {
                                app.resize(&state);
                            }
                        }
                        _ => {}
                    }
                }
            }

            Event::RedrawRequested(window_id) => {
                let app_type = self.windows.get(&window_id);
                if app_type.is_none() {
                    return;
                }
                let app_type = app_type.unwrap();

                let app = self.apps.get_mut(app_type).unwrap();
                app.render(&state).unwrap();
            }
            Event::MainEventsCleared => {
                let dt = prev_frame_t.elapsed().as_secs_f32();
                prev_frame_t = std::time::Instant::now();

                for (_app_type, app) in self.apps.iter_mut() {
                    app.update(self.tokio_rt.handle(), &state, dt);
                    app.window.window.request_redraw();
                }
            }

            _ => {}
        })
    }
}

pub trait AppWindow {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        dt: f32,
    );

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool;

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()>;

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &WindowState,
        // window_dims: PhysicalSize<u32>,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()>;
}

/*
pub trait AppWindow {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        dt: f32,
    );

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool;

    fn resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()>;

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &mut raving_wgpu::WindowState,
    ) -> anyhow::Result<()>;
}
*/

/*
impl App {
    pub fn init(window_handler: WindowHandler) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("waragraph-tokio")
            .build()?;

        let tokio_rt = Arc::new(runtime);

        Ok(Self {
            window_handler,
            tokio_rt,
        })
    }

    pub async fn run(
        self,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
        window: raving_wgpu::WindowState,
    ) -> Result<()> {
        let Self {
            window_handler,
            tokio_rt,
        } = self;

        window_handler
            .run(tokio_rt, event_loop, state, window)
            .await
    }
}
*/

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub tsv: Option<PathBuf>,
    pub annotations: Option<PathBuf>,
    // init_range: Option<std::ops::Range<u64>>,
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    let annotations = pargs.opt_value_from_os_str("--bed", parse_path)?;
    // let init_range = pargs.opt_value_from_fn("--range", parse_range)?;

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.opt_free_from_os_str(parse_path)?,

        annotations,
        // init_range,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}

// fn parse_range(s: &str) -> Result<std::ops::Range<u64>> {
//     const ERROR_MSG: &'static str = "Range must be in the format `start-end`,\
// where `start` and `end` are nonnegative integers and `start` < `end`";

//     let fields = s.trim().split('-').take(2).collect::<Vec<_>>();

//     if fields.len() != 2 {
//         anyhow::bail!(ERROR_MSG);
//     }

//     let start = fields[0].parse::<u64>()?;
//     let end = fields[1].parse::<u64>()?;
//     if start >= end {
//         anyhow::bail!(ERROR_MSG);
//     }

//     Ok(start..end)
// }
