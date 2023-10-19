// pub mod app;
pub mod color;
pub mod context;
pub mod util;

pub mod viewer_1d;
pub mod viewer_2d;

use std::{collections::HashMap, sync::Arc};

// use app::resource::GraphDataCache;
use color::{ColorSchemeId, ColorStore};
use parking_lot::RwLock;

use egui_winit::winit;
use raving_wgpu::gui::EguiCtx;
use waragraph_core::{arrow_graph::ArrowGFA, graph::PathIndex};
use wasm_bindgen_futures::JsFuture;

use wasm_bindgen::prelude::*;

use crate::viewer_1d::CoordSys;

use crate::viewer_2d::layout::NodePositions;

#[wasm_bindgen(module = "/js/util.js")]
extern "C" {
    pub(crate) fn segment_pos_obj(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) -> JsValue;

    pub(crate) fn uint32_array_helper(
        memory: JsValue,
        data_ptr: *const u32,
        data_len: u32,
    ) -> js_sys::Uint32Array;
}

#[derive(Clone)]
pub struct SharedState {
    pub graph: Arc<waragraph_core::graph::PathIndex>,

    // pub shared: Arc<RwLock<AnyArcMap>>,
    // pub graph_data_cache: Arc<GraphDataCache>,

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

    min_bounds: ultraviolet::Vec2,
    max_bounds: ultraviolet::Vec2,
}

#[wasm_bindgen]
impl SegmentPositions {
    pub fn path_to_canvas_space(
        &self,
        view: &viewer_2d::view::View2D,
        canvas_width: f32,
        canvas_height: f32,
        path_slice: &[u32],
    ) -> Result<web_sys::Path2d, JsValue> {
        use ultraviolet::{Vec2, Vec3};

        let matrix =
            view.to_viewport_matrix(Vec2::new(canvas_width, canvas_height));

        let path2d = web_sys::Path2d::new()?;
        let mut added = 0;

        for &step_handle in path_slice {
            let seg = step_handle >> 1;
            let i = (seg * 2) as usize;

            let p0 = Vec2::new(self.xs[i], self.ys[i]);
            let p1 = Vec2::new(self.xs[i + 1], self.ys[i + 1]);

            let p = p0 + (p1 - p0) * 0.5;

            let p_v3 = Vec3::new(p.x, p.y, 1.0);
            let q = matrix * p_v3;

            if added == 0 {
                path2d.move_to(q.x as f64, q.y as f64);
            } else {
                path2d.line_to(q.x as f64, q.y as f64);
            }
            added += 1;
        }

        Ok(path2d)
    }

    pub fn bounds_as_view_obj(&self) -> JsValue {
        let size = self.max_bounds - self.min_bounds;
        let center = self.min_bounds + size * 0.5;

        crate::viewer_2d::view::create_view_obj(
            center.x, center.y, size.x, size.y,
        )
    }

    // pub fn segment_pos(&self, seg_id: u32) -> JsValue {
    pub fn segment_pos(&self, seg_id: u32) -> JsValue {
        let i = (seg_id * 2) as usize;

        if i >= self.xs.len() {
            return JsValue::NULL;
        }

        let x0 = self.xs[i];
        let y0 = self.ys[i];
        let x1 = self.xs[i + 1];
        let y1 = self.ys[i + 1];

        segment_pos_obj(x0, y0, x1, y1)
    }

    pub fn from_tsv(
        // tsv_text: js_sys::Promise,
        tsv_text: JsValue,
    ) -> Result<SegmentPositions, JsValue> {
        use std::io::prelude::*;
        use std::io::Cursor;
        use ultraviolet::Vec2;

        let tsv_text = tsv_text
            .as_string()
            .ok_or_else(|| format!("TSV could not be read as text"))?;

        let cursor = Cursor::new(tsv_text.as_bytes());

        let mut xs = Vec::new();
        let mut ys = Vec::new();

        let mut min_bounds = Vec2::broadcast(std::f32::MAX);
        let mut max_bounds = Vec2::broadcast(std::f32::MIN);

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
            let p = Vec2::new(x, y);
            min_bounds = min_bounds.min_by_component(p);
            max_bounds = max_bounds.max_by_component(p);

            xs.push(x);
            ys.push(y);
        }

        Ok(SegmentPositions {
            xs,
            ys,
            min_bounds,
            max_bounds,
        })
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

    pub fn path_steps(
        &self,
        path_name: &str,
    ) -> Result<js_sys::Uint32Array, JsValue> {
        let path_index =
            self.0.path_name_index(path_name).ok_or_else(|| {
                JsValue::from_str(&format!("Path `{path_name}` not found"))
            })?;

        let steps = &self.0.path_steps[path_index as usize];
        let slice = steps.values().as_slice();

        let ptr = slice.as_ptr();

        let memory = wasm_bindgen::memory();
        let array = uint32_array_helper(memory, ptr, slice.len() as u32);

        Ok(array)
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
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Debug);
}
