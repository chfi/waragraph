use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    app::{settings_menu::SettingsWindow, SharedState},
    viewer_1d::Viewer1D,
    viewer_2d::Viewer2D,
};

use anyhow::{Context, Result};

pub enum Pane {
    Start,
    Viewer1D,
    Viewer2D,
    Settings,
}

pub struct AppBehavior<'a> {
    shared_state: &'a SharedState,

    viewer_1d: Option<&'a mut Viewer1D>,
    viewer_2d: Option<&'a mut Viewer2D>,
    settings: &'a mut SettingsWindow,
}

impl<'a> AppBehavior<'a> {
    //
}

impl<'a> egui_tiles::Behavior<Pane> for AppBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Start => "Start".into(),
            Pane::Viewer1D => "1D View".into(),
            Pane::Viewer2D => "2D View".into(),
            Pane::Settings => "Settings".into(),
        }
    }

    fn pane_ui(
        &mut self,
        _ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::Start => {
                todo!()
            }
            Pane::Viewer1D => {
                todo!()
            }
            Pane::Viewer2D => {
                todo!()
            }
            Pane::Settings => {
                todo!()
            }
        }

        todo!()
    }
}

pub struct App {
    pub tokio_rt: Arc<tokio::runtime::Runtime>,
    pub shared: Option<SharedState>,

    viewer_1d: Option<Viewer1D>,
    viewer_2d: Option<Viewer2D>,

    gfa_path: Option<Arc<PathBuf>>,
    tsv_path: Option<Arc<PathBuf>>,
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
            gfa_path: None,
            tsv_path: None,
        })
    }
}
