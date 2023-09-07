pub mod app;
pub mod color;
pub mod context;
pub mod util;

pub mod viewer_1d;
pub mod viewer_2d;

use std::{collections::HashMap, sync::Arc};

use app::resource::GraphDataCache;
use color::{ColorSchemeId, ColorStore};
use parking_lot::RwLock;

use egui_winit::winit;
use raving_wgpu::gui::EguiCtx;
use waragraph_core::graph::PathIndex;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlCanvasElement;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    window::{Fullscreen, WindowBuilder},
};

use wasm_bindgen::prelude::*;

use crate::viewer_1d::CoordSys;

use crate::viewer_2d::layout::NodePositions;

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

#[wasm_bindgen]
pub fn list_path_names(ctx: &Context) -> Box<[JsValue]> {
    let graph = &ctx.app.shared.as_ref().unwrap().graph;

    for n in graph.path_names.right_values() {
        //
        web_sys::console::log_1(&n.into());
    }

    Box::new([])

    // let names: Vec<JsValue> = graph
    //     .path_names
    //     .right_values()
    //     .map(JsValue::from)
    //     .collect::<Vec<_>>();

    // names.into_boxed_slice()
}

#[wasm_bindgen]
pub struct Context {
    // shared: SharedState,
    pub(crate) app: app::App,
    event_loop: EventLoop<()>,
    gpu_state: raving_wgpu::State,
    window: raving_wgpu::WindowState,
    egui_ctx: EguiCtx,
}

#[wasm_bindgen]
impl Context {
    // pub fn global_coord_sys(&self) -> CoordSys {
    //     CoordSys::global_from_graph(&self.app.shared.as_ref().unwrap().graph)
    // }

    // pub fn init_path_viewer(
    //     &self,
    //     path_name: &str,
    // ) -> Option<viewer_1d::PathViewer> {
    //     let graph = self.app.shared.as_ref()?.graph.clone();
    //     let path = *graph.path_names.get_by_right(path_name)?;

    //     Some(viewer_1d::PathViewer::new(graph, path, 512))
    // }

    pub fn canvas_element(&self) -> HtmlCanvasElement {
        use winit::platform::web::WindowExtWebSys;
        self.window.window.canvas()
    }

    pub fn run(self) {
        let result = self.app.run(
            self.event_loop,
            self.gpu_state,
            self.window,
            self.egui_ctx,
        );

        if let Err(e) = result {
            // TODO
            panic!("{e}");
        }
    }
}

#[wasm_bindgen]
pub async fn initialize_with_data(
    gfa_text_src: js_sys::Promise,
    tsv_text_src: js_sys::Promise,
    canvas: JsValue,
) -> Result<Context, JsValue> {
    use web_sys::console;
    console::log_1(&"running initialize_with_data".into());

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let canvas = canvas.dyn_into::<HtmlCanvasElement>().ok();

    let gfa_src = JsFuture::from(gfa_text_src).await?;
    let tsv_src = JsFuture::from(tsv_text_src).await?;

    let gfa = gfa_src.as_string().unwrap();
    let tsv = tsv_src.as_string().unwrap();

    let result =
        initialize_with_data_impl(gfa.as_str(), tsv.as_str(), canvas).await;

    match result {
        Ok(ctx) => Ok(ctx),
        Err(e) => {
            Err(JsValue::from_str(&format!("initialization error: {e:?}")))
        }
    }
}

#[wasm_bindgen]
pub struct PathIndexWrap(pub(crate) PathIndex);

#[wasm_bindgen]
impl PathIndexWrap {
    pub fn node_count(&self) -> usize {
        self.0.node_count
    }

    pub fn path_count(&self) -> usize {
        self.0.path_names.len()
    }
}

#[wasm_bindgen]
pub async fn load_gfa_path_index(
    gfa_resp: js_sys::Promise,
) -> Result<PathIndexWrap, JsValue> {
    use std::io::Cursor;

    let gfa_resp = JsFuture::from(gfa_resp).await?;

    let gfa = JsFuture::from(gfa_resp.dyn_into::<web_sys::Response>()?.text()?)
        .await?;

    let gfa = gfa.as_string().unwrap();

    let graph = PathIndex::from_gfa_impl(Cursor::new(gfa.as_str())).unwrap();

    Ok(PathIndexWrap(graph))
}

#[wasm_bindgen]
pub async fn initialize_with_data_fetch(
    gfa_resp: js_sys::Promise,
    tsv_resp: js_sys::Promise,
    canvas: JsValue,
) -> Result<Context, JsValue> {
    use web_sys::console;
    console::log_1(&"running initialize_with_data_fetch".into());

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let canvas = canvas.dyn_into::<HtmlCanvasElement>().ok();

    let gfa_resp = JsFuture::from(gfa_resp).await?;
    let tsv_resp = JsFuture::from(tsv_resp).await?;

    let gfa = JsFuture::from(gfa_resp.dyn_into::<web_sys::Response>()?.text()?)
        .await?;

    let tsv = JsFuture::from(tsv_resp.dyn_into::<web_sys::Response>()?.text()?)
        .await?;

    let gfa = gfa.as_string().unwrap();
    let tsv = tsv.as_string().unwrap();

    let result =
        initialize_with_data_impl(gfa.as_str(), tsv.as_str(), canvas).await;

    match result {
        Ok(ctx) => Ok(ctx),
        Err(e) => {
            Err(JsValue::from_str(&format!("initialization error: {e:?}")))
        }
    }
}

#[wasm_bindgen]
pub async fn initialize(canvas: JsValue) -> Result<Context, JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    use web_sys::console;
    console::log_1(&"running initialize_impl".into());

    console::log_1(&canvas);
    let canvas = canvas.dyn_into::<HtmlCanvasElement>().ok();
    console::log_1(&canvas.is_some().into());

    let gfa_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test/data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa"
    ));

    let tsv_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test/data/A-3105.layout.tsv"
    ));

    let result = initialize_with_data_impl(gfa_src, tsv_src, canvas).await;

    match result {
        Ok(ctx) => Ok(ctx),
        Err(e) => {
            Err(JsValue::from_str(&format!("initialization error: {e:?}")))
        }
    }
}

async fn initialize_with_data_impl(
    gfa_src: &str,
    tsv_src: &str,
    canvas: Option<HtmlCanvasElement>,
) -> anyhow::Result<Context> {
    use web_sys::console;
    use winit::platform::web::WindowBuilderExtWebSys;
    console::log_1(&"event loop".into());
    let event_loop = EventLoop::new();

    console::log_1(&"window builder".into());
    let builder = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_canvas(canvas);

    let window = builder.build(&event_loop).unwrap();

    console::log_1(&"raving".into());
    let (gpu_state, window) =
        raving_wgpu::State::new_with_window(window).await?;

    console::log_1(&"after raving".into());

    // #[cfg(target_arch = "wasm32")]
    // let builder = {
    // use winit::platform::web::{WindowBuilderExtWebSys, WindowExtWebSys};
    // builder
    // };
    console::log_1(&"window".into());

    // need to pass the data in!
    // the methods I have all read from the file system

    console::log_1(&"app".into());
    let mut app = app::App::init()?;

    use std::io::Cursor;

    console::log_1(&"node pos".into());
    let node_positions =
        NodePositions::from_layout_tsv_impl(Cursor::new(tsv_src))?;

    console::log_1(&"graph".into());
    let graph = PathIndex::from_gfa_impl(Cursor::new(gfa_src))?;
    // let window = gpu_state.prepare_window(window)?;

    // let (event_loop, gpu_state, window) = raving_wgpu::initialize().await?;

    console::log_1(&"egui".into());
    let mut egui_ctx = EguiCtx::init(
        &gpu_state,
        window.surface_format,
        &event_loop,
        Some(wgpu::Color::WHITE),
    );

    console::log_1(&"shared state".into());
    app.initialize_shared_state(&gpu_state, Arc::new(graph));

    app.node_positions = Some(Arc::new(node_positions));

    // app.initialize_2d_viewer(&gpu_state, &window, &mut egui_ctx);

    let ctx = Context {
        app,
        event_loop,
        gpu_state,
        window,
        egui_ctx,
    };

    Ok(ctx)
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
