use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    app::{settings_menu::SettingsWindow, SharedState},
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
                start.show(ui);
            }
            Pane::Viewer1D => {
                // TODO
            }
            Pane::Viewer2D => {
                // TODO
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

    graph: Option<PathIndex>,
    node_positions: Option<NodePositions>,
}

impl App {
    pub fn init() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("waragraph-tokio")
            .build()?;

        let tokio_rt = Arc::new(runtime);

        Ok(App {
            tokio_rt,
            shared: None,

            viewer_1d: None,

            viewer_2d: None,
            node_positions: None,

            gfa_path: None,
            tsv_path: None,

            resource_state: None,
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        todo!();
    }

    fn start_loading_resources(
        &mut self,
        mut state: ResourceLoadState,
    ) -> Result<()> {
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
                // state.graph = ...
                todo!();
            };

            if let Some(tsv_path) = state.tsv_path.as_ref() {
                // load & set `node_positions`
                // state.node_positions = ...
                todo!();
            };

            state
        });

        let fut = async move {
            let result = handle.await?;
            Ok(result)
        };

        // create the resource_state as a future that awaits the blocking thread
        let resource_state = ImmediateValuePromise::new(fut);

        self.resource_state = Some(resource_state);

        Ok(())
    }
}
