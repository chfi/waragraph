use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};

use raving_wgpu::gui::EguiCtx;
use tokio::{
    runtime::Runtime,
    sync::{mpsc, RwLock},
};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::WindowId,
};

use crate::{
    annotations::{AnnotationSet, AnnotationStore},
    app::{
        resource::GraphDataCache, settings_menu::SettingsWindow,
        workspace::Workspace, SharedState,
    },
    color::ColorStore,
    viewer_1d::Viewer1D,
    viewer_2d::{layout::NodePositions, Viewer2D},
};

use anyhow::{Context, Result};
use lazy_async_promise::ImmediateValuePromise;
use waragraph_core::graph::PathIndex;

mod start;

pub enum Pane {
    Start(start::StartPage),
    Viewer1D,
    Viewer2D,
    // Settings,
}

impl Pane {
    fn data_id(&self) -> egui::Id {
        match self {
            Pane::Start(_) => egui::Id::new("RootStart"),
            Pane::Viewer1D => egui::Id::new("RootViewer1D"),
            Pane::Viewer2D => egui::Id::new("RootViewer2D"),
        }
    }
}

pub struct AppBehavior<'a> {
    shared_state: Option<&'a SharedState>,

    viewer_1d: Option<&'a mut Viewer1D>,
    viewer_2d: Option<&'a mut Viewer2D>,
    // settings: &'a mut SettingsWindow,

    // bit ugly but fine for now
    // probably want to move into a struct when there's more cases
    init_resources: Option<ResourceLoadState>,
}

impl<'a> AppBehavior<'a> {
    //
}

impl<'a> egui_tiles::Behavior<Pane> for AppBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Start(_) => "Start".into(),
            Pane::Viewer1D => "1D View".into(),
            Pane::Viewer2D => "2D View".into(),
            // Pane::Settings => "Settings".into(),
        }
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::Start(start) => {
                if let Some(load_state) = start.show(ui) {
                    self.init_resources = Some(load_state);
                }
            }
            Pane::Viewer1D => {
                // TODO
                ui.label("1D placeholder");
            }
            Pane::Viewer2D => {
                // TODO
                ui.label("2D placeholder");
            } //
              // Pane::Settings => {
              //     todo!()
              // }
        }

        Default::default()
    }
}

pub struct App {
    pub tokio_rt: Arc<tokio::runtime::Runtime>,
    pub shared: Option<SharedState>,

    tree: egui_tiles::Tree<Pane>,

    viewer_1d: Option<Viewer1D>,

    viewer_2d: Option<Viewer2D>,
    pub node_positions: Option<Arc<NodePositions>>,

    gfa_path: Option<Arc<PathBuf>>,
    tsv_path: Option<Arc<PathBuf>>,

    resource_state: Option<ImmediateValuePromise<ResourceLoadState>>,
}

struct ResourceLoadState {
    gfa_path: Option<PathBuf>,
    tsv_path: Option<PathBuf>,

    graph: Option<Arc<PathIndex>>,
    node_positions: Option<Arc<NodePositions>>,
}

impl App {
    pub fn init() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("waragraph-tokio")
            .build()?;

        let mut tiles = egui_tiles::Tiles::default();
        let tabs =
            vec![tiles.insert_pane(Pane::Start(start::StartPage::default()))];
        let root = tiles.insert_tab_tile(tabs);

        let tree = egui_tiles::Tree::new(root, tiles);

        let tokio_rt = Arc::new(runtime);

        Ok(App {
            tokio_rt,
            shared: None,

            tree,

            viewer_1d: None,

            viewer_2d: None,
            node_positions: None,

            gfa_path: None,
            tsv_path: None,

            resource_state: None,
        })
    }

    pub fn update(
        &mut self,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
    ) {
        // if resources are ready, initialize the SharedState

        if let Some(res_state) = self.resource_state.as_mut() {
            use lazy_async_promise::ImmediateValueState as State;
            match res_state.poll_state() {
                State::Success(res_state) => {
                    self.node_positions = res_state.node_positions.clone();
                    // TODO initialize sharedstate

                    if let Some(graph) = res_state.graph.clone() {
                        let gfa_path = res_state
                            .gfa_path
                            .as_ref()
                            .map(|p| p.to_path_buf())
                            .unwrap();

                        let tsv_path = res_state
                            .tsv_path
                            .as_ref()
                            .map(|p| p.to_path_buf());
                        self.initialize_shared_state(
                            state, gfa_path, tsv_path, graph,
                        );
                    }
                }
                State::Updating | State::Empty => {
                    // do nothing
                }
                State::Error(err) => {
                    // report error and reset
                    log::error!("{:#?}", err.0);
                    self.resource_state = None;
                }
            }
        }

        let mut rebuild_tree = false;

        // if SharedState and node positions are ready, but the
        // viewers haven't been initialized, create them and add panes

        if let Some(shared) = self.shared.as_ref() {
            let dims: [u32; 2] = window.window.inner_size().into();

            if self.viewer_1d.is_none() {
                let viewer =
                    Viewer1D::init(dims, state, window, shared).unwrap();
                self.viewer_1d = Some(viewer);

                rebuild_tree = true;
            }

            if self.viewer_2d.is_none() {
                if let Some(pos) = self.node_positions.clone() {
                    // initialize 2d viewer
                    let viewer =
                        Viewer2D::init(state, window, pos, shared).unwrap();
                    self.viewer_2d = Some(viewer);

                    rebuild_tree = true;
                }
            }
        }

        if rebuild_tree {
            let mut tiles = egui_tiles::Tiles::default();

            let has_1d = self.viewer_1d.is_some();
            let has_2d = self.viewer_2d.is_some();

            let mut tabs = vec![];
            // let tabs = vec![
            //     tiles.insert_pane(Pane::Start(start::StartPage::default()))
            // ];

            if !(has_1d && has_2d) {
                tabs.push(
                    tiles.insert_pane(Pane::Start(start::StartPage::default())),
                );
            }

            if has_1d {
                tabs.push(tiles.insert_pane(Pane::Viewer1D));
            }

            if has_2d {
                tabs.push(tiles.insert_pane(Pane::Viewer2D));
            }

            let root = tiles.insert_tab_tile(tabs);

            let tree = egui_tiles::Tree::new(root, tiles);

            self.tree = tree;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let mut behavior = AppBehavior {
            shared_state: self.shared.as_ref(),
            viewer_1d: self.viewer_1d.as_mut(),
            viewer_2d: self.viewer_2d.as_mut(),
            init_resources: None,
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            self.tree.ui(&mut behavior, ui);
        });

        if let Some(state) = behavior.init_resources {
            self.start_loading_resources(state).unwrap();
        }
    }

    fn start_loading_resources(
        &mut self,
        mut state: ResourceLoadState,
    ) -> Result<()> {
        if self.resource_state.is_some() {
            return Ok(());
        }

        // just in case, don't try to load if we've already begun
        if self.gfa_path.is_some() {
            state.gfa_path = None;
        }
        if self.tsv_path.is_some() {
            state.tsv_path = None;
        }

        if state.gfa_path.is_none() && state.tsv_path.is_none() {
            return Ok(());
        }

        // spawn a blocking thread that loads the GFA and/or TSV

        // while a GFA is necessary in both 1D and 2D, I'm designing
        // this to handle the case where a GFA is first loaded, and
        // then extended further with a TSV, using the same function
        // and type

        let handle = self.tokio_rt.spawn_blocking(move || {
            if let Some(gfa_path) = state.gfa_path.as_ref() {
                // load & set `graph`
                let result =
                    waragraph_core::graph::PathIndex::from_gfa(gfa_path);

                match result {
                    Ok(path_index) => state.graph = Some(Arc::new(path_index)),
                    Err(err) => log::error!("Error parsing GFA: {err:#?}"),
                }
            };

            if let Some(tsv_path) = state.tsv_path.as_ref() {
                // load & set `node_positions`
                let result = NodePositions::from_layout_tsv(tsv_path);

                match result {
                    Ok(pos) => state.node_positions = Some(Arc::new(pos)),
                    Err(err) => log::error!("Error parsing layout: {err:#?}"),
                }
            };

            state
        });

        let fut = async move {
            let result = handle.await?;
            Ok(result)
        };

        let _guard = self.tokio_rt.enter();

        // create the resource_state as a future that awaits the blocking thread
        let resource_state = ImmediateValuePromise::new(fut);

        self.resource_state = Some(resource_state);

        Ok(())
    }
}

impl App {
    pub fn run(
        mut self,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
        mut window: raving_wgpu::WindowState,
    ) -> Result<()> {
        let mut is_ready = false;
        let mut prev_frame_t = std::time::Instant::now();

        let mut egui_ctx = EguiCtx::init(
            &state,
            window.surface_format,
            &event_loop,
            Some(wgpu::Color::BLACK),
        );

        event_loop.run(
            move |event, event_loop_tgt, control_flow| match &event {
                Event::Resumed => {
                    if !is_ready {
                        is_ready = true;
                    }
                }
                Event::WindowEvent { window_id, event } => {
                    let consumed = egui_ctx.on_event(event).consumed;
                    // let mut consumed = app.on_event(event);

                    if !consumed {
                        match &event {
                            WindowEvent::CloseRequested => {
                                *control_flow = ControlFlow::Exit
                            }
                            WindowEvent::Resized(phys_size) => {
                                if is_ready {
                                    window.resize(&state.device);
                                    /*
                                    app.resize(&state);
                                    app.app
                                        .on_resize(
                                            &state,
                                            app.window.size.into(),
                                            (*phys_size).into(),
                                        )
                                        .unwrap();
                                    */
                                }
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

                Event::RedrawRequested(window_id) => {
                    if let Ok(output) = window.surface.get_current_texture() {
                        let mut encoder = state.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("TileApp Command Encoder"),
                            },
                        );

                        let output_view = output.texture.create_view(
                            &wgpu::TextureViewDescriptor::default(),
                        );

                        // let result = app.render(
                        //     state,
                        //     window,
                        //     &output_view,
                        //     &mut encoder,
                        // );
                        // if let Err(e) = result {
                        //     log::error!("Rendering error: {e:?}");
                        // }

                        egui_ctx.render(
                            &state,
                            &window,
                            &output_view,
                            &mut encoder,
                        );
                        state.queue.submit(Some(encoder.finish()));
                        output.present();
                    } else {
                        window.resize(&state.device);
                    }

                    /*
                    let app_type = self.app_windows.windows.get(&window_id);
                    if app_type.is_none() {
                        return;
                    }
                    let app_type = app_type.unwrap();

                    let app = self.app_windows.apps.get_mut(app_type).unwrap();
                    app.render(&state).unwrap();
                    */
                }
                Event::MainEventsCleared => {
                    let dt = prev_frame_t.elapsed().as_secs_f32();
                    prev_frame_t = std::time::Instant::now();

                    self.update(&state, &window);

                    egui_ctx.begin_frame(&window.window);

                    self.show(egui_ctx.ctx());

                    egui_ctx.end_frame(&window.window);

                    window.window.request_redraw();
                }

                _ => {}
            },
        );
    }
}

impl App {
    fn initialize_shared_state(
        &mut self,
        state: &raving_wgpu::State,
        gfa_path: PathBuf,
        tsv_path: Option<PathBuf>,
        // res_state: &ResourceLoadState,
        graph: Arc<PathIndex>,
    ) {
        let workspace = Arc::new(RwLock::new(Workspace { gfa_path, tsv_path }));

        let graph_data_cache = Arc::new(GraphDataCache::init(&graph));

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

        let annotations: Arc<RwLock<AnnotationStore>> =
            Arc::new(RwLock::new(annotations));

        // i'll remove this before i actually use it
        let (app_msg_send, app_msg_recv) = mpsc::channel(256);

        let shared = SharedState {
            graph,

            // shared: Arc::new(RwLock::new(AnyArcMap::default())),
            graph_data_cache,
            annotations,

            colors,

            data_color_schemes: Arc::new(data_color_schemes.into()),

            workspace,

            app_msg_send,
        };

        self.shared = Some(shared);
    }
}
