pub mod app;
pub mod color;
pub mod context;
pub mod util;
pub mod viewer_2d;

use std::{collections::HashMap, sync::Arc};

use app::resource::GraphDataCache;
use color::{ColorSchemeId, ColorStore};
use parking_lot::RwLock;

use egui_winit::winit;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    window::{Fullscreen, WindowBuilder},
};

#[derive(Clone)]
pub struct SharedState {
    pub graph: Arc<waragraph_core::graph::PathIndex>,

    // pub shared: Arc<RwLock<AnyArcMap>>,
    pub graph_data_cache: Arc<GraphDataCache>,

    // pub annotations: Arc<RwLock<AnnotationStore>>,
    pub colors: Arc<RwLock<ColorStore>>,

    // pub workspace: Arc<RwLock<Workspace>>,
    // gfa_path: Arc<PathBuf>,
    // tsv_path: Option<Arc<RwLock<PathBuf>>>,
    pub data_color_schemes: Arc<RwLock<HashMap<String, ColorSchemeId>>>,
    // pub app_msg_send: tokio::sync::mpsc::Sender<AppMsg>,
}

pub fn run() {
    let event_loop = EventLoop::new();
    let builder = WindowBuilder::new().with_title("A fantastic window!");
    #[cfg(target_arch = "wasm32")]
    let builder = {
        use winit::platform::web::WindowBuilderExtWebSys;
        builder.with_append(true)
    };
    let window = builder.build(&event_loop).unwrap();

    // need to pass the data in!
    // the methods I have all read from the file system

    let app = app::App::init();
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
