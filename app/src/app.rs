mod window;

use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::runtime::Runtime;
use winit::{
    event_loop::EventLoop,
    window::{WindowBuilder, WindowId},
};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;

pub use window::WindowHandler;

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

pub struct AppState {
    // window: Option<WindowId>,
    window: WindowId,
    app: Box<dyn AppWindow>,
    egui: EguiCtx,
}

pub struct NewApp {
    tokio_rt: Arc<Runtime>,
    shared: SharedState,

    windows: HashMap<WindowId, WindowState>,
    apps: HashMap<AppType, AppState>,
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
        let title = "Waragraph 1D".to_string();

        let window =
            WindowBuilder::new().with_title(title).build(event_loop)?;
        let window = state.prepare_window(window)?;

        let dims: [u32; 2] = window.window.inner_size().into();

        let winid = window.window.id();

        let app = {
            let viewer = Viewer1D::init(
                event_loop,
                dims,
                state,
                &window,
                self.shared.graph.clone(),
            )?;
            let app = Box::new(viewer) as Box<dyn AppWindow>;

            let egui_ctx =
                EguiCtx::init(&state, window.surface_format, &event_loop, None);

            AppState {
                window: winid,
                app,
                egui: egui_ctx,
            }
        };

        self.apps.insert(AppType::Viewer1D, app);
        self.windows.insert(winid, window);

        Ok(())
    }

    pub async fn run(
        self,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
    ) -> Result<()> {
        todo!();
    }
}

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
        window: &raving_wgpu::WindowState,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()>;

    fn render(&mut self, state: &mut raving_wgpu::State) -> anyhow::Result<()>;
}

pub struct App {
    window_handler: WindowHandler,
    tokio_rt: Arc<Runtime>,
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
