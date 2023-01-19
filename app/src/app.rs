use crossbeam::atomic::AtomicCell;
use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::{runtime::Runtime, sync::RwLock};
use waragraph_core::graph::{Bp, PathId};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowId,
};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    viewer_1d::Viewer1D,
    viewer_2d::{PathRenderer, Viewer2D},
};

mod window;

pub mod resource;

pub use window::AppWindowState;

use self::resource::{AnyArcMap, GraphDataCache};

pub struct SharedState {
    pub graph: Arc<waragraph_core::graph::PathIndex>,

    pub shared: Arc<RwLock<AnyArcMap>>,
    pub graph_data_cache: Arc<GraphDataCache>,

    pub gfa_path: Arc<PathBuf>,
    pub tsv_path: Option<Arc<PathBuf>>,

    viewer_1d_interactions: Arc<AtomicCell<VizInteractions>>,
    viewer_2d_interactions: Arc<AtomicCell<VizInteractions>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppType {
    Viewer1D,
    Viewer2D,
}

pub struct App {
    pub tokio_rt: Arc<Runtime>,
    pub shared: SharedState,

    pub windows: HashMap<WindowId, AppType>,
    pub apps: HashMap<AppType, AppWindowState>,
}

impl App {
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

            let graph_data_cache = Arc::new(GraphDataCache::init(&path_index));

            SharedState {
                graph: path_index,

                shared: Arc::new(RwLock::new(AnyArcMap::default())),
                graph_data_cache,

                gfa_path,
                tsv_path,

                viewer_1d_interactions: Arc::new(AtomicCell::new(
                    Default::default(),
                )),
                viewer_2d_interactions: Arc::new(AtomicCell::new(
                    Default::default(),
                )),
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

            let mut app = Viewer1D::init(
                event_loop,
                dims,
                state,
                &window,
                self.shared.graph.clone(),
                &self.shared,
            )?;

            app.self_viz_interact = self.shared.viewer_1d_interactions.clone();
            app.connected_viz_interact =
                Some(self.shared.viewer_2d_interactions.clone());

            Ok(Box::new(app))
        })?;

        let winid = app.window.window.id();

        self.apps.insert(AppType::Viewer1D, app);
        self.windows.insert(winid, AppType::Viewer1D);

        Ok(())
    }

    pub fn init_viewer_2d(
        &mut self,
        event_loop: &EventLoop<()>,
        state: &raving_wgpu::State,
    ) -> Result<()> {
        let tsv = if let Some(tsv) = self.shared.tsv_path.as_ref() {
            tsv
        } else {
            anyhow::bail!("Can't initialize 2D viewer without layout TSV");
        };

        let title = "Waragraph 2D";

        let app = AppWindowState::init(event_loop, state, title, |window| {
            let mut app = Viewer2D::init(
                state,
                &window,
                self.shared.graph.clone(),
                tsv.as_ref(),
            )?;

            app.self_viz_interact = self.shared.viewer_2d_interactions.clone();
            app.connected_viz_interact =
                Some(self.shared.viewer_1d_interactions.clone());

            Ok(Box::new(app))
        })?;

        let winid = app.window.window.id();

        self.apps.insert(AppType::Viewer2D, app);
        self.windows.insert(winid, AppType::Viewer2D);

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
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub tsv: Option<PathBuf>,
    pub annotations: Option<PathBuf>,
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct VizInteractions {
    pub clicked: bool,
    pub interact_path: Option<PathId>,
    pub interact_node: Option<waragraph_core::graph::Node>,
    pub interact_pan_pos: Option<Bp>,
    pub interact_path_pos: Option<(PathId, Bp)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VizInteractMsg {
    Click,
    Path(PathId),
    Node(waragraph_core::graph::Node),
    PangenomePos(Bp),
    PathPos { path: PathId, pos: Bp },
}

impl VizInteractions {
    pub fn apply(&mut self, msg: VizInteractMsg) {
        match msg {
            VizInteractMsg::Click => self.clicked = true,
            VizInteractMsg::Path(path) => self.interact_path = Some(path),
            VizInteractMsg::Node(node) => self.interact_node = Some(node),
            VizInteractMsg::PangenomePos(pos) => {
                self.interact_pan_pos = Some(pos)
            }
            VizInteractMsg::PathPos { path, pos } => {
                self.interact_path_pos = Some((path, pos))
            }
        }
    }
}
