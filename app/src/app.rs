use crossbeam::atomic::AtomicCell;
use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::{
    runtime::Runtime,
    sync::{mpsc, RwLock},
};
use waragraph_core::graph::{Bp, PathId};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::WindowId,
};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    annotations::{AnnotationSet, AnnotationStore},
    color::{ColorSchemeId, ColorStore},
    context::{widget::ContextInspector, ContextState},
    viewer_1d::Viewer1D,
    viewer_2d::Viewer2D,
};

mod window;

pub mod settings_menu;

pub mod workspace;

pub mod resource;

pub use window::AppWindowState;

use self::{
    resource::{AnyArcMap, GraphDataCache},
    settings_menu::SettingsWindow,
    window::{AppWindows, AsleepWindow, WindowDelta},
    workspace::Workspace,
};

#[derive(Clone)]
pub struct SharedState {
    pub graph: Arc<waragraph_core::graph::PathIndex>,

    // pub shared: Arc<RwLock<AnyArcMap>>,
    pub graph_data_cache: Arc<GraphDataCache>,

    pub annotations: Arc<RwLock<AnnotationStore>>,

    pub colors: Arc<RwLock<ColorStore>>,

    pub workspace: Arc<RwLock<Workspace>>,
    // gfa_path: Arc<PathBuf>,
    // tsv_path: Option<Arc<RwLock<PathBuf>>>,
    pub data_color_schemes: Arc<RwLock<HashMap<String, ColorSchemeId>>>,

    pub app_msg_send: tokio::sync::mpsc::Sender<AppMsg>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppType {
    Viewer1D,
    Viewer2D,
    // MainMenu,
    Custom(String),
}

pub struct App {
    pub tokio_rt: Arc<Runtime>,
    pub shared: SharedState,

    context_state: ContextState,

    context_inspector: ContextInspector,

    app_windows: AppWindows,
    // pub windows: HashMap<WindowId, AppType>,
    // pub apps: HashMap<AppType, AppWindowState>,

    // sleeping: HashMap<AppType, AsleepWindow>,
    settings: SettingsWindow,
    settings_window_tgt: Option<WindowId>,

    app_msg_recv: tokio::sync::mpsc::Receiver<AppMsg>,
}

impl App {
    pub fn init(state: &raving_wgpu::State, args: Args) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("waragraph-tokio")
            .build()?;

        let tokio_rt = Arc::new(runtime);

        let path_index = waragraph_core::graph::PathIndex::from_gfa(&args.gfa)?;
        let path_index = Arc::new(path_index);

        let (app_msg_send, app_msg_recv) = mpsc::channel::<AppMsg>(256);

        let mut settings = SettingsWindow::new(
            tokio_rt.handle().clone(),
            app_msg_send.clone(),
        );

        let app_windows = AppWindows::default();

        settings.register_widget(
            "Window",
            "Windows",
            app_windows.widget_state.clone(),
        );

        let shared = {
            let workspace = Arc::new(RwLock::new(Workspace {
                gfa_path: args.gfa,
                tsv_path: args.tsv,
            }));

            {
                let ws = workspace.clone();
                settings.register_widget("General", "Graph & Layout", ws);
            }

            let graph_data_cache = Arc::new(GraphDataCache::init(&path_index));

            let colors = Arc::new(RwLock::new(ColorStore::init(state)));

            let mut data_color_schemes = HashMap::default();

            {
                let mut colors = colors.blocking_write();

                let mut add_entry = |data: &str, color: &str| {
                    let scheme = colors.get_color_scheme_id(color).unwrap();

                    colors.create_color_scheme_texture(state, color);

                    data_color_schemes.insert(data.into(), scheme);
                };

                add_entry("depth", "spectral");
                add_entry("strand", "black_red");
            }

            let mut annotations = AnnotationStore::default();

            for annot_path in args.annotations.iter() {
                if let Some(ext) = annot_path.extension() {
                    let result = if ext == "bed" {
                        AnnotationSet::from_bed(
                            &path_index,
                            None,
                            |name| name.to_string(),
                            annot_path,
                        )
                    } else if ext == "gff" {
                        let attr = args
                            .gff_attr
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("Name");

                        // TODO the name and record functions should be configurable
                        AnnotationSet::from_gff(
                            &path_index,
                            None,
                            |name| name.to_string(),
                            // |name| format!("S288C.{name}"),
                            // |name| format!("SGDref#1#{name}"),
                            |record| {
                                let attrs = record.attributes();
                                let label = attrs.iter().find_map(|entry| {
                                    (entry.key() == attr)
                                        .then_some(entry.value())
                                })?;

                                Some(label.to_string())
                            },
                            annot_path,
                        )
                    } else {
                        log::error!("Unknown annotation file extension `{ext:?}`, ignoring");
                        continue;
                    };

                    match result {
                        Ok(set) => {
                            log::warn!(
                                "loaded annotation set with {} annotations",
                                set.annotations.len()
                            );

                            annotations.insert_set(set);
                        }
                        Err(e) => {
                            log::error!(
                                "Error loading annotation file {:?}: {e:?}",
                                annot_path.as_os_str()
                            );
                        }
                    }
                }
            }

            let annotations: Arc<RwLock<AnnotationStore>> =
                Arc::new(RwLock::new(annotations));

            SharedState {
                graph: path_index,

                // shared: Arc::new(RwLock::new(AnyArcMap::default())),
                graph_data_cache,
                annotations,

                colors,

                data_color_schemes: Arc::new(data_color_schemes.into()),

                workspace,

                app_msg_send,
            }
        };

        let context_state = ContextState::default();

        let context_inspector = ContextInspector::with_default_widgets(&shared);

        settings.register_widget(
            "Context",
            "Context Inspector",
            context_inspector.settings_widget().clone(),
        );

        Ok(Self {
            tokio_rt,
            shared,

            context_state,
            context_inspector,

            app_windows,
            // windows: HashMap::default(),
            // apps: HashMap::default(),

            // sleeping: HashMap::default(),
            settings,
            settings_window_tgt: None,

            app_msg_recv,
        })
    }

    pub fn init_custom_window(
        &mut self,
        event_loop: &EventLoop<()>,
        state: &raving_wgpu::State,
        id: &str,
        title: Option<&str>,
        constructor: impl FnOnce(&WindowState) -> anyhow::Result<Box<dyn AppWindow>>,
    ) -> Result<()> {
        let id = id.to_string();
        let title = title.map(|s| s.to_string()).unwrap_or(id.clone());
        let app_id = AppType::Custom(id);

        let app = AppWindowState::init(event_loop, state, &title, constructor)?;

        let winid = app.window.window.id();

        self.app_windows.apps.insert(app_id.clone(), app);
        self.app_windows.windows.insert(winid, app_id);

        Ok(())
    }

    pub fn init_viewer_1d(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
    ) -> Result<()> {
        let title = "Waragraph 1D";

        let app = AppWindowState::init(event_loop, state, title, |window| {
            let dims: [u32; 2] = window.window.inner_size().into();

            let mut app = Viewer1D::init(
                dims,
                state,
                &window,
                self.shared.graph.clone(),
                &self.shared,
                &mut self.settings,
            )?;

            Ok(Box::new(app))
        })?;

        let winid = app.window.window.id();

        self.app_windows.apps.insert(AppType::Viewer1D, app);
        self.app_windows.windows.insert(winid, AppType::Viewer1D);

        Ok(())
    }

    pub fn init_viewer_2d(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
    ) -> Result<()> {
        let tsv = if let Some(tsv) =
            self.shared.workspace.blocking_read().tsv_path().cloned()
        {
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
                tsv,
                &self.shared,
                &mut self.settings,
            )?;

            Ok(Box::new(app))
        })?;

        let winid = app.window.window.id();

        self.app_windows.apps.insert(AppType::Viewer2D, app);
        self.app_windows.windows.insert(winid, AppType::Viewer2D);

        Ok(())
    }

    pub fn run(
        mut self,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
    ) -> Result<()> {
        let mut is_ready = false;
        let mut prev_frame_t = std::time::Instant::now();

        self.app_windows.update_widget_state();

        {
            // upload color buffers -- should obviously be handled better,
            // rather than just once at the start!
            let mut colors = self.shared.colors.blocking_write();
            colors.upload_color_schemes_to_gpu(&state)?;
        }

        event_loop.run(
            move |event, event_loop_tgt, control_flow| match &event {
                Event::Resumed => {
                    if !is_ready {
                        is_ready = true;
                    }
                }
                Event::WindowEvent { window_id, event } => {
                    let app_type = self.app_windows.windows.get(&window_id);
                    if app_type.is_none() {
                        return;
                    }
                    let app_type = app_type.unwrap();
                    let app = self.app_windows.apps.get_mut(app_type).unwrap();

                    let size = app.window.window.inner_size();

                    let mut consumed = app.on_event(event);

                    if !consumed {
                        match &event {
                            WindowEvent::KeyboardInput { input, .. } => {
                                use VirtualKeyCode as Key;

                                let pressed = matches!(
                                    input.state,
                                    ElementState::Pressed
                                );

                                if let Some(Key::Escape) = input.virtual_keycode
                                {
                                    if pressed {
                                        if let Err(e) =
                                            self.shared.app_msg_send.try_send(
                                                AppMsg::ToggleSettingsWindow {
                                                    src: *window_id,
                                                },
                                            )
                                        {
                                            log::error!("{e:?}");
                                        }
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
                    let app_type = self.app_windows.windows.get(&window_id);
                    if app_type.is_none() {
                        return;
                    }
                    let app_type = app_type.unwrap();

                    let app = self.app_windows.apps.get_mut(app_type).unwrap();
                    app.render(&state).unwrap();
                }
                Event::MainEventsCleared => {
                    let dt = prev_frame_t.elapsed().as_secs_f32();
                    prev_frame_t = std::time::Instant::now();

                    self.context_state.start_frame();

                    while let Ok(msg) = self.app_msg_recv.try_recv() {
                        if let Err(e) =
                            self.process_msg(event_loop_tgt, &state, msg)
                        {
                            log::error!("Error processing AppMsg: {e:?}");
                        }
                    }

                    // TODO: don't really like just having this here,
                    // but good enough for now
                    self.app_windows.update_widget_state();

                    let context_inspector_tgts =
                        self.context_inspector.active_targets();

                    for (app_type, app) in self.app_windows.apps.iter_mut() {
                        app.update(
                            self.tokio_rt.handle(),
                            &state,
                            &mut self.context_state,
                            dt,
                        );

                        if Some(app.window.window.id())
                            == self.settings_window_tgt
                        {
                            self.settings.show(app.egui.ctx());
                        }

                        if context_inspector_tgts.contains(app_type) {
                            egui::Window::new("Context Inspector")
                                .default_pos([100.0, 100.0])
                                .show(app.egui.ctx(), |ui| {
                                    self.context_inspector
                                        .show(&self.context_state, ui);
                                });
                        }

                        app.window.window.request_redraw();
                    }
                }

                _ => {}
            },
        )
    }
}

impl App {
    fn process_msg(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        msg: AppMsg,
    ) -> Result<()> {
        match msg {
            AppMsg::InitViewer1D => {
                if !self.app_windows.apps.contains_key(&AppType::Viewer1D) {
                    // todo
                }
            }
            AppMsg::InitViewer2D => {
                if !self.app_windows.apps.contains_key(&AppType::Viewer2D) {
                    if let Err(e) = self.init_viewer_2d(event_loop, state) {
                        log::error!("Error initializing 2D viewer");
                    }
                }
            }
            AppMsg::OpenSettingsWindow { src } => {
                if self.settings_window_tgt.is_none() {
                    self.settings_window_tgt = Some(src);
                }
            }
            AppMsg::ToggleSettingsWindow { src } => {
                if let Some(tgt) = self.settings_window_tgt.take() {
                    if src != tgt {
                        self.settings_window_tgt = Some(src);
                    }
                } else {
                    self.settings_window_tgt = Some(src);
                }
            }
            AppMsg::WindowDelta(delta) => {
                self.app_windows
                    .handle_window_delta(event_loop, state, delta)?;
            }
        }

        Ok(())
    }
}

pub trait AppWindow {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        context_state: &mut ContextState,
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

    pub annotations: Vec<PathBuf>,
    pub gff_attr: Option<String>,
    // pub annotations: Option<PathBuf>,
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    // let init_range = pargs.opt_value_from_fn("--range", parse_range)?;

    let mut annotations = Vec::new();

    let bed = pargs.opt_value_from_os_str("--bed", parse_path)?;
    if let Some(bed) = bed {
        annotations.push(bed);
    }

    let gff = pargs.opt_value_from_os_str("--gff", parse_path)?;
    if let Some(gff) = gff {
        annotations.push(gff);
    }

    let gff_attr = pargs.opt_value_from_str("--gff-attr")?;

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.opt_free_from_os_str(parse_path)?,

        annotations,
        gff_attr,
        // init_range,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}

#[derive(Debug, Clone)]
pub enum AppMsg {
    InitViewer1D,
    InitViewer2D,
    OpenSettingsWindow { src: WindowId },
    ToggleSettingsWindow { src: WindowId },
    WindowDelta(WindowDelta),
}
