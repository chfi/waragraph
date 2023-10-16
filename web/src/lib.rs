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
use waragraph_core::{arrow_graph::ArrowGFA, graph::PathIndex};
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
pub struct RavingCtx {
    pub(crate) gpu_state: raving_wgpu::State,
    pub(crate) surface_format: wgpu::TextureFormat,
}

#[wasm_bindgen]
impl RavingCtx {
    pub async fn initialize_(
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<RavingCtx, JsValue> {
        let gpu_state =
            raving_wgpu::State::new_for_canvas(canvas.clone()).await;

        let Ok(gpu_state) = gpu_state else {
            return Err(JsValue::from_str("not that it matters"));
        };

        // create a canvas to create a surface so we can get the texture format
        let surface: wgpu::Surface = gpu_state
            .instance
            .create_surface_from_canvas(canvas)
            .map_err(|err: wgpu::CreateSurfaceError| -> JsValue {
                format!("error creating surface from offscreen canvas: {err:?}")
                    .into()
            })?;

        let caps = surface.get_capabilities(&gpu_state.adapter);
        let surface_format = caps.formats[0];

        Ok(Self {
            gpu_state,
            surface_format,
        })
    }
    pub async fn initialize() -> Result<RavingCtx, JsValue> {
        let gpu_state = raving_wgpu::State::new_web().await;

        let Ok(gpu_state) = gpu_state else {
            log::debug!("?");
            return Err(JsValue::from_str("not that it matters"));
        };

        // create a canvas to create a surface so we can get the texture format
        log::debug!("!");
        let canvas = web_sys::OffscreenCanvas::new(300, 150)?;
        log::debug!("!");
        let surface: wgpu::Surface = gpu_state
            .instance
            .create_surface_from_offscreen_canvas(canvas)
            .map_err(|err: wgpu::CreateSurfaceError| -> JsValue {
                format!("error creating surface from offscreen canvas: {err:?}")
                    .into()
            })?;

        log::debug!("!");
        let caps = surface.get_capabilities(&gpu_state.adapter);
        let surface_format = caps.formats[0];
        log::debug!("{surface_format:?}");

        Ok(Self {
            gpu_state,
            surface_format,
        })
    }
}

#[wasm_bindgen]
pub struct SegmentPositions {
    xs: Vec<f32>,
    ys: Vec<f32>,
}

#[wasm_bindgen]
impl SegmentPositions {
    pub fn from_tsv(
        // tsv_text: js_sys::Promise,
        tsv_text: JsValue,
    ) -> Result<SegmentPositions, JsValue> {
        use std::io::prelude::*;
        use std::io::Cursor;

        let tsv_text = tsv_text
            .as_string()
            .ok_or_else(|| format!("TSV could not be read as text"))?;

        let cursor = Cursor::new(tsv_text.as_bytes());

        let mut xs = Vec::new();
        let mut ys = Vec::new();

        log::debug!("parsing?????");
        for (i, line) in cursor.lines().enumerate() {
            if i == 0 {
                continue;
            }

            let Ok(line) = line else { continue };
            let line = line.trim();

            let mut fields = line.split_ascii_whitespace();

            let _id = fields.next();

            let x = fields.next().unwrap().parse::<f32>().unwrap();
            let y = fields.next().unwrap().parse::<f32>().unwrap();

            xs.push(x);
            ys.push(y);
        }

        Ok(SegmentPositions { xs, ys })
    }
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
pub struct ArrowGFAWrapped(pub(crate) ArrowGFA);

#[wasm_bindgen]
impl ArrowGFAWrapped {
    pub fn segment_count(&self) -> usize {
        self.0.segment_count()
    }

    pub fn path_count(&self) -> usize {
        self.0.path_count()
    }

    pub fn path_name(&self, path_index: u32) -> Result<String, JsValue> {
        let name = self.0.path_name(path_index).ok_or_else(|| {
            JsValue::from_str(&format!("Path index `{path_index}` not found"))
        })?;

        Ok(name.to_string())
    }

    pub fn with_path_names(&self, f: &js_sys::Function) {
        let this = JsValue::null();
        for path_name in self.0.path_names.iter() {
            if let Some(name) = path_name {
                let val = JsValue::from_str(name);
                let _ = f.call1(&this, &val);
            }
        }
    }

    // returning a Vec<JsValue> seems broken right now, idk why
    // pub fn path_names(&self) -> Vec<JsValue> {
    //     let mut vector: Vec<JsValue> = vec![];
    //     for (i, name) in self.0.path_names.iter().enumerate() {
    //         if let Some(name) = name {
    //             vector.push(JsValue::from_str(name));
    //         }
    //     }
    //     vector
    // }
}

#[wasm_bindgen]
pub async fn load_gfa_arrow(
    gfa_resp: js_sys::Promise,
) -> Result<ArrowGFAWrapped, JsValue> {
    use std::io::Cursor;

    web_sys::console::log_1(&"JsFuture from gfa response".into());
    let gfa_resp = JsFuture::from(gfa_resp).await?;

    web_sys::console::log_1(&"JsFuture response text".into());
    let gfa = JsFuture::from(gfa_resp.dyn_into::<web_sys::Response>()?.text()?)
        .await?;

    web_sys::console::log_1(&"gfa as string".into());
    let gfa = gfa.as_string().unwrap();

    web_sys::console::log_1(&"calling arrow_graph_from_gfa".into());
    let graph = waragraph_core::arrow_graph::arrow_graph_from_gfa(Cursor::new(
        gfa.as_str(),
    ))
    .unwrap();

    Ok(ArrowGFAWrapped(graph))
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
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Debug);
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

mod gpustuff {
    use egui_winit::winit::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::Window,
    };
    use std::borrow::Cow;

    /*
    initialize instance/surface/adapter
    pipeline
    */

    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub async fn run_once(canvas: web_sys::HtmlCanvasElement) {
        super::set_panic_hook();

        let width = canvas.width();
        let height = canvas.height();
        // let size = window.inner_size();

        let rvng = super::RavingCtx::initialize_(canvas.clone()).await.unwrap();

        // let instance = wgpu::Instance::default();

        // let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let surface = rvng
            .gpu_state
            .instance
            .create_surface_from_canvas(canvas)
            .unwrap();
        /*
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");
        */

        // Load the shaders from disk
        let shader = rvng.gpu_state.device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../shader.wgsl"
                ))),
            },
        );

        let pipeline_layout = rvng.gpu_state.device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            },
        );

        let swapchain_capabilities =
            surface.get_capabilities(&rvng.gpu_state.adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let render_pipeline = rvng.gpu_state.device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(swapchain_format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            },
        );

        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: width,
            height: height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&rvng.gpu_state.device, &config);

        let frame = surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = rvng.gpu_state.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: None },
        );
        {
            let mut rpass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                store: true,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                });
            rpass.set_pipeline(&render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        rvng.gpu_state.queue.submit(Some(encoder.finish()));
        frame.present();

        /*
        event_loop.run(move |event, _, control_flow| {
            // Have the closure take ownership of the resources.
            // `event_loop.run` never returns, therefore we must do this to ensure
            // the resources are properly cleaned up.
            let _ = (&instance, &adapter, &shader, &pipeline_layout);

            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    // Reconfigure the surface with the new size
                    config.width = size.width;
                    config.height = size.height;
                    surface.configure(&device, &config);
                    // On macos the window needs to be redrawn manually after resizing
                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    let frame = surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: None },
                    );
                    {
                        let mut rpass = encoder.begin_render_pass(
                            &wgpu::RenderPassDescriptor {
                                label: None,
                                color_attachments: &[Some(
                                    wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(
                                                wgpu::Color::GREEN,
                                            ),
                                            store: true,
                                        },
                                    },
                                )],
                                depth_stencil_attachment: None,
                            },
                        );
                        rpass.set_pipeline(&render_pipeline);
                        rpass.draw(0..3, 0..1);
                    }

                    queue.submit(Some(encoder.finish()));
                    frame.present();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => {}
            }
        });
        */
    }

    async fn run_event_loop(event_loop: EventLoop<()>, window: Window) {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();

        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        // Load the shaders from disk
        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../shader.wgsl"
                ))),
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(swapchain_format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        event_loop.run(move |event, _, control_flow| {
            // Have the closure take ownership of the resources.
            // `event_loop.run` never returns, therefore we must do this to ensure
            // the resources are properly cleaned up.
            let _ = (&instance, &adapter, &shader, &pipeline_layout);

            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    // Reconfigure the surface with the new size
                    config.width = size.width;
                    config.height = size.height;
                    surface.configure(&device, &config);
                    // On macos the window needs to be redrawn manually after resizing
                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    let frame = surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: None },
                    );
                    {
                        let mut rpass = encoder.begin_render_pass(
                            &wgpu::RenderPassDescriptor {
                                label: None,
                                color_attachments: &[Some(
                                    wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(
                                                wgpu::Color::GREEN,
                                            ),
                                            store: true,
                                        },
                                    },
                                )],
                                depth_stencil_attachment: None,
                            },
                        );
                        rpass.set_pipeline(&render_pipeline);
                        rpass.draw(0..3, 0..1);
                    }

                    queue.submit(Some(encoder.finish()));
                    frame.present();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => {}
            }
        });
    }

    #[wasm_bindgen]
    pub fn run_main() {
        let event_loop = EventLoop::new();
        let window =
            egui_winit::winit::window::Window::new(&event_loop).unwrap();
        #[cfg(target_arch = "wasm32")]
        {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init().expect("could not initialize logger");
            use egui_winit::winit::platform::web::WindowExtWebSys;
            // On wasm, append the canvas to the document body
            web_sys::window()
                .and_then(|win| win.document())
                .and_then(|doc| doc.body())
                .and_then(|body| {
                    body.append_child(&web_sys::Element::from(window.canvas()))
                        .ok()
                })
                .expect("couldn't append canvas to document body");
            wasm_bindgen_futures::spawn_local(run_event_loop(
                event_loop, window,
            ));
        }
    }
}

/*
mod gpustuff {
    #[cfg(not(target_arch = "wasm32"))]
    use wgpu_example::utils::output_image_native;
    #[cfg(target_arch = "wasm32")]
    use wgpu_example::utils::output_image_wasm;

    const TEXTURE_DIMS: (usize, usize) = (512, 512);

    async fn run(_path: Option<String>) {
        // This will later store the raw pixel value data locally. We'll create it now as
        // a convenient size reference.
        let mut texture_data =
            Vec::<u8>::with_capacity(TEXTURE_DIMS.0 * TEXTURE_DIMS.1 * 4);

        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .unwrap();

        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                    include_str!("../shader.wgsl"),
                )),
            });

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: TEXTURE_DIMS.0 as u32,
                height: TEXTURE_DIMS.1 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });
        let output_staging_buffer =
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: texture_data.capacity() as u64,
                usage: wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

        let pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(
                        wgpu::TextureFormat::Rgba8UnormSrgb.into(),
                    )],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        log::info!("Wgpu context set up.");

        //-----------------------------------------------

        let texture_view =
            render_target.create_view(&wgpu::TextureViewDescriptor::default());

        let mut command_encoder = device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut render_pass = command_encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: &texture_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                store: true,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    // occlusion_query_set: None,
                    // timestamp_writes: None,
                },
            );
            render_pass.set_pipeline(&pipeline);
            render_pass.draw(0..3, 0..1);
        }
        // The texture now contains our rendered image
        command_encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    // This needs to be a multiple of 256. Normally we would need to pad
                    // it but we here know it will work out anyways.
                    bytes_per_row: Some((TEXTURE_DIMS.0 * 4) as u32),
                    rows_per_image: Some(TEXTURE_DIMS.1 as u32),
                },
            },
            wgpu::Extent3d {
                width: TEXTURE_DIMS.0 as u32,
                height: TEXTURE_DIMS.1 as u32,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(command_encoder.finish()));
        log::info!("Commands submitted.");

        //-----------------------------------------------

        // Time to get our image.
        let buffer_slice = output_staging_buffer.slice(..);
        let (sender, receiver) =
            futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice
            .map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
        device.poll(wgpu::Maintain::Wait);
        receiver.receive().await.unwrap().unwrap();
        log::info!("Output buffer mapped.");
        {
            let view = buffer_slice.get_mapped_range();
            texture_data.extend_from_slice(&view[..]);
        }
        log::info!("Image data copied to local.");
        output_staging_buffer.unmap();

        #[cfg(not(target_arch = "wasm32"))]
        output_image_native(
            texture_data.to_vec(),
            TEXTURE_DIMS,
            _path.unwrap(),
        );
        #[cfg(target_arch = "wasm32")]
        output_image_wasm(texture_data.to_vec(), TEXTURE_DIMS);
        log::info!("Done.");
    }

    fn run_main() {
        #[cfg(target_arch = "wasm32")]
        {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Info)
                .expect("could not initialize logger");
            wasm_bindgen_futures::spawn_local(run(None));
        }
    }
}
*/
