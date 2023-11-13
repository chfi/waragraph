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
use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};
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

use ultraviolet::Vec2;

#[derive(Debug, Clone)]
#[wasm_bindgen]
pub struct CanvasPathTrace {
    // path2d: JsValue,
    points: Vec<Vec2>,
}

#[wasm_bindgen]
impl CanvasPathTrace {
    // pub fn get_path2d(&self) -> &web_sys::Path2d {
    // pub fn get_path2d(&self) -> &JsValue {
    //     &self.path2d
    // }

    pub fn with_points(&self, f: &js_sys::Function) {
        let this = JsValue::null();
        for point in &self.points {
            // let val = JsValue::from_str(name);
            let _ = f.call2(&this, &point.x.into(), &point.y.into());
        }
    }

    pub fn points_array(&self) -> js_sys::Float32Array {
        let ptr = self.points.as_ptr();

        let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
        js_sys::Float32Array::new_with_byte_offset_and_length(
            &memory.buffer(),
            ptr as u32,
            self.points.len() as u32,
        )
    }
}

#[wasm_bindgen]
impl SegmentPositions {
    pub fn sample_canvas_space_path(
        &self,
        view: &viewer_2d::view::View2D,
        canvas_width: f32,
        canvas_height: f32,
        path_slice: &[u32],
        tolerance: f32,
    ) -> Result<CanvasPathTrace, JsValue> {
        use ultraviolet::Vec3;

        if path_slice.is_empty() {
            return Err("Empty path".into());
        }

        let matrix =
            view.to_viewport_matrix(Vec2::new(canvas_width, canvas_height));

        // TODO could use SIMD here maybe

        let mut points: Vec<Vec2> = Vec::new();

        let path_vertices = path_slice.iter().flat_map(|&step_handle| {
            let seg = step_handle >> 1;
            let i = (seg * 2) as usize;

            let p0 = Vec2::new(self.xs[i], self.ys[i]);
            let p1 = Vec2::new(self.xs[i + 1], self.ys[i + 1]);

            [p0, p1]
            // let tan = p1 - p0;

            // let q0 = matrix * Vec3::new(p0.x, p0.y, 1.0);
            // let q1 = matrix * Vec3::new(p1.x, p1.y, 1.0);
        });

        let tol_sq = tolerance * tolerance;

        for p in path_vertices {
            let q = matrix * Vec3::new(p.x, p.y, 1.0);

            if let Some(last_q) = points.last().copied() {
                let delta = q.xy() - last_q;
                let dist_sq = delta.mag_sq();

                if delta.mag_sq() >= tol_sq {
                    points.push(q.xy());
                }
            } else {
                points.push(q.xy());
            }
        }

        Ok(CanvasPathTrace { points })
    }

    pub fn path_to_canvas_space_alt(
        &self,
        view: &viewer_2d::view::View2D,
        canvas_width: f32,
        canvas_height: f32,
        path_slice: &[u32],
    ) -> JsValue {
        use ultraviolet::{Vec2, Vec3};

        let matrix =
            view.to_viewport_matrix(Vec2::new(canvas_width, canvas_height));

        if path_slice.is_empty() {
            return JsValue::NULL;
        }

        let map_point = |step_handle: u32| -> Vec3 {
            let seg = step_handle >> 1;
            let i = (seg * 2) as usize;

            let p0 = Vec2::new(self.xs[i], self.ys[i]);
            let p1 = Vec2::new(self.xs[i + 1], self.ys[i + 1]);

            let p = p0 + (p1 - p0) * 0.5;

            let p_v3 = Vec3::new(p.x, p.y, 1.0);
            let q = matrix * p_v3;
            q
        };

        let start = map_point(path_slice[0]);
        let end = map_point(*path_slice.last().unwrap());

        let obj = segment_pos_obj(start.x, start.y, end.x, end.y);

        obj

        // let obj = js_sys::Object::new();

        // for (k, v) in [("start", s_i as f64), ("end", e_i as f64)] {
        //     js_sys::Reflect::set(obj.as_ref(), &k.into(), &v.into());
        // }
    }

    pub fn path_to_canvas_space(
        &self,
        view: &viewer_2d::view::View2D,
        canvas_width: f32,
        canvas_height: f32,
        path_slice: &[u32],
        tolerance: f32,
    ) -> Result<web_sys::Path2d, JsValue> {
        use ultraviolet::{Vec2, Vec3};

        let matrix =
            view.to_viewport_matrix(Vec2::new(canvas_width, canvas_height));

        let path2d = web_sys::Path2d::new()?;
        let mut added = 0;

        let mut last_added: Option<Vec2> = None;

        log::warn!("slice length: {}", path_slice.len());

        if path_slice.is_empty() {
            return Err("Empty path".into());
        }

        // let steps = [path_slice[0], *path_slice.last().unwrap()];

        let mut last_q = None;

        for &step_handle in path_slice {
            // for &step_handle in &steps {
            let seg = step_handle >> 1;
            let i = (seg * 2) as usize;

            let p0 = Vec2::new(self.xs[i], self.ys[i]);
            let p1 = Vec2::new(self.xs[i + 1], self.ys[i + 1]);

            let p = p0 + (p1 - p0) * 0.5;

            let p_v3 = Vec3::new(p.x, p.y, 1.0);
            let q = matrix * p_v3;

            last_q = Some(q);

            if let Some(last) = last_added {
                if (last - q.xy()).mag() < tolerance {
                    continue;
                }
            }

            if added == 0 {
                path2d.move_to(q.x as f64, q.y as f64);
            } else {
                path2d.line_to(q.x as f64, q.y as f64);
            }
            last_added = Some(q.xy());

            added += 1;
        }

        if added == 1 {
            if let Some(q) = last_q {
                path2d.line_to(q.x as f64, q.y as f64);
            }
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
        let i = seg_id as usize;

        log::warn!("seg_id: {seg_id}");
        log::warn!("self.xs.len() {}", self.xs.len());

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
        let t0 = instant::now();
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
        let t1 = instant::now();
        log::warn!("layout TSV parsing took {:.2} ms", t1 - t0);

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
pub struct PathIndexWrapped(pub(crate) PathIndex);

#[wasm_bindgen]
impl PathIndexWrapped {
    pub fn paths_on_segment(&self, segment: u32) -> () {
        let bitmap = self.0.segment_path_matrix.paths_on_segment(segment);

        for (ix, &bits) in bitmap.iter() {
            println!("{ix} - {:b}", bits);
        }

        // bitmap
        // for set in bitmap {
        // }
        // let matrix = self.0.segment_path_matrix.matrix();
        // let rows = matrix.rows();

        // let rhs: CsVec = sprs::CsVecI::empty();
        // let result = vec![0; rows];

        // let mut out = vec![0; rows];
    }
}

#[wasm_bindgen]
impl ArrowGFAWrapped {
    pub fn generate_path_index(&self) -> PathIndexWrapped {
        let path_index = PathIndex::from_arrow_gfa(&self.0);
        PathIndexWrapped(path_index)
    }

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

    pub fn path_index(&self, path_name: &str) -> Result<u32, JsValue> {
        self.0.path_name_index(path_name).ok_or_else(|| {
            JsValue::from_str(&format!("Path `{path_name}` not found"))
        })
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
        let path_index = self.path_index(path_name)?;

        let steps = &self.0.path_steps(path_index);
        let slice = steps.values().as_slice();

        let ptr = slice.as_ptr();

        let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
        Ok(js_sys::Uint32Array::new_with_byte_offset_and_length(
            &memory.buffer(),
            ptr as u32,
            slice.len() as u32,
        ))
    }

    pub fn segment_sequences_array(&self) -> js_sys::Uint8Array {
        let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());

        let seq = self.0.segment_sequences.values();
        let ptr = seq.as_ptr();

        js_sys::Uint8Array::new_with_byte_offset_and_length(
            &memory.buffer(),
            ptr as u32,
            seq.len() as u32,
        )
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
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Debug);
}
