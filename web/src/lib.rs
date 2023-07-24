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
use raving_wgpu::gui::EguiCtx;
use waragraph_core::graph::PathIndex;
use web_sys::HtmlCanvasElement;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    window::{Fullscreen, WindowBuilder},
};

use wasm_bindgen::prelude::*;

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
pub struct Context {
    app: app::App,
    event_loop: EventLoop<()>,
    gpu_state: raving_wgpu::State,
    window: raving_wgpu::WindowState,
    egui_ctx: EguiCtx,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl Context {
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
        }
    }
}

#[wasm_bindgen]
pub async fn initialize(canvas: JsValue) -> Result<Context, JsValue> {
    use web_sys::console;
    console::log_1(&"running initialize_impl".into());

    console::log_1(&canvas);
    let canvas = canvas.dyn_into::<HtmlCanvasElement>().ok();
    console::log_1(&canvas.is_some().into());

    let result = initialize_impl(canvas).await;

    match result {
        Ok(ctx) => Ok(ctx),
        Err(e) => {
            Err(JsValue::from_str(&format!("initialization error: {e:?}")))
        }
    }
}

async fn initialize_impl(
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

    // console::log_1(&"wgpu".into());
    // let _ = {
    //     // let backends = wgpu::util::backend_bits_from_env()
    //     //     .unwrap_or(wgpu::Backends::all());
    //     let backends = wgpu::Backends::BROWSER_WEBGPU;
    //     // let backends = wgpu::Backends::all();

    //     console::log_1(&format!("backends: {backends:?}").into());
    //     let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    //         backends,
    //         dx12_shader_compiler: Default::default(),
    //     });
    //     console::log_1(&format!("instance: {instance:?}").into());
    //     // console::log_1(&instance.into());

    //     let adapter = wgpu::util::initialize_adapter_from_env_or_default(
    //         &instance, backends, None,
    //     )
    //     .await
    //     .ok_or(anyhow::anyhow!("Could not find compatible adapter"))?;

    //     let allowed_limits = adapter.limits();

    //     let (device, queue) = adapter
    //         .request_device(
    //             &wgpu::DeviceDescriptor {
    //                 features: wgpu::Features::empty(),
    //                 limits: if cfg!(target_arch = "wasm32") {
    //                     wgpu::Limits::downlevel_webgl2_defaults()
    //                 } else {
    //                     wgpu::Limits {
    //                         max_push_constant_size: allowed_limits
    //                             .max_push_constant_size,
    //                         ..wgpu::Limits::default()
    //                     }
    //                 },
    //                 label: None,
    //             },
    //             None,
    //         )
    //         .await?;
    // };

    // #[cfg(target_arch = "wasm32")]
    // let builder = {
    // use winit::platform::web::{WindowBuilderExtWebSys, WindowExtWebSys};
    // builder
    // };
    console::log_1(&"window".into());

    // need to pass the data in!
    // the methods I have all read from the file system

    let gfa_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test/data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa"
    ));

    let tsv_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test/data/A-3105.layout.tsv"
    ));

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
