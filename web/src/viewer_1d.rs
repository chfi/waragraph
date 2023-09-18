use std::{collections::BTreeMap, sync::Arc};

use arrow2::{array::PrimitiveArray, offset::OffsetsBuffer};
use waragraph_core::graph::{Bp, Node, PathId, PathIndex};

use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, OffscreenCanvas,
};

use crate::{ArrowGFAWrapped, PathIndexWrap};

pub mod sampler;

#[wasm_bindgen(module = "/js/util.js")]
extern "C" {
    //
    #[wasm_bindgen(catch)]
    fn create_image_data_impl(
        mem: JsValue,
        data_ptr: *const u8,
        data_len: u32,
    ) -> Result<ImageData, JsValue>;

}

#[wasm_bindgen]
pub struct PathViewer {
    cs: CoordSys,
    data: SparseData,

    // graph: Arc<PathIndex>,
    // path: PathId,
    color_map: Box<dyn Fn(f32) -> [u8; 4]>,

    bins: Vec<f32>,
    last_sampled_range: Option<std::ops::RangeInclusive<u64>>,

    canvas: OffscreenCanvas, // matches `bins`
    target_canvas: Option<OffscreenCanvas>,
}

#[wasm_bindgen]
impl PathViewer {
    #[wasm_bindgen(getter)]
    pub fn coord_sys(&self) -> CoordSys {
        self.cs.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn get_bin_data(&self) -> Box<[f32]> {
        self.bins.clone().into_boxed_slice()
    }

    pub fn sample_range(&mut self, bp_start: JsValue, bp_end: JsValue) {
        let bp_start = bp_start.as_f64().unwrap() as u64;
        let bp_end = bp_end.as_f64().unwrap() as u64;

        let range = (bp_start as u64)..=(bp_end as u64);

        let bin_count = self.bins.len().min((bp_end - bp_start) as usize);

        let bins = &mut self.bins[..bin_count];

        self.cs.sample_impl(
            range.clone(),
            self.data.indices.values(),
            self.data.data.values(),
            bins,
        );

        self.last_sampled_range = Some(range);
    }

    pub fn new(
        cs: CoordSys,
        data: SparseData,
        bin_count: usize,
        color_0: JsValue,
        color_1: JsValue,
    ) -> Result<PathViewer, JsValue> {
        let get_channel = |color: &JsValue, c: &str| {
            let val = js_sys::Reflect::get(color, &c.into());
            val.ok().and_then(|v| v.as_f64())
        };
        let get_color = |color: &JsValue| -> Option<[f32; 4]> {
            let r = get_channel(color, "r")? as f32;
            let g = get_channel(color, "g")? as f32;
            let b = get_channel(color, "b")? as f32;
            let a = get_channel(color, "a")? as f32;

            if [r, g, b, a].into_iter().any(|c| c > 1.0) {
                return None;
            }

            Some([r, g, b, a])
        };

        let err_text =
            "Color must be provided as an object { r: _, g: _, b: _, a: _ }";

        let (color_0, color_1) = get_color(&color_0)
            .zip(get_color(&color_1))
            .ok_or_else(|| JsValue::from_str(err_text))?;

        let color_map = Box::new(move |val: f32| {
            let [rf, gf, bf, af] = if val > 1.0 { color_0 } else { color_1 };

            let r = (255.0 * rf) as u8;
            let g = (255.0 * gf) as u8;
            let b = (255.0 * bf) as u8;
            let a = (255.0 * af) as u8;
            [r, g, b, a]
        });

        let bins = vec![0f32; bin_count];

        let canvas = OffscreenCanvas::new(bin_count as u32, 1)?;

        Ok(PathViewer {
            cs,
            data,

            color_map,

            bins,
            canvas,
            last_sampled_range: None,
            target_canvas: None,
        })
    }

    pub fn set_target_canvas(&mut self, canvas: OffscreenCanvas) {
        web_sys::console::log_1(&format!("setting canvas").into());
        self.target_canvas = Some(canvas);
    }

    /*
    pub fn new_canvas(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        let canvas = OffscreenCanvas::new(width, height)?;
        self.canvas = Some(canvas);
        Ok(())
    }
    */
}

impl PathViewer {
    fn render_into_offscreen_canvas(&self) {
        let Some(view_range) = self.last_sampled_range.clone() else {
            return;
        };

        let view_size = *view_range.end() - *view_range.start();

        let bin_count = self.bins.len().min(view_size as usize);

        // draw pixel data into single-row offscreen canvas
        let mut pixel_data: Vec<u8> = vec![0; bin_count * 4];
        let pixels = pixel_data.chunks_exact_mut(4);

        for (color, val) in pixels.zip(&self.bins) {
            let c = (self.color_map)(*val);
            color.clone_from_slice(&c);
        }

        let memory = wasm_bindgen::memory();
        let px_len = pixel_data.len() as u32;
        let pixels_ptr = pixel_data.as_ptr() as *const u8;

        let image_data =
            create_image_data_impl(memory, pixels_ptr, px_len).unwrap();
        let ctx = self.canvas.get_context("2d").ok().flatten().unwrap();
        let ctx = ctx
            .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()
            .unwrap();

        let _ = ctx.put_image_data(&image_data, 0.0, 0.0);
    }
}

#[wasm_bindgen]
impl PathViewer {
    pub fn render_into_new_buffer(&self) -> Box<[u8]> {
        let mut pixel_data: Vec<u8> = vec![0; self.bins.len() * 4];

        web_sys::console::log_1(
            &format!("pixel data length: {}", pixel_data.len()).into(),
        );

        let pixels = pixel_data.chunks_exact_mut(4);

        for (color, val) in pixels.zip(&self.bins) {
            let c = (self.color_map)(*val);
            color.clone_from_slice(&c);
        }

        pixel_data.into_boxed_slice()
    }

    pub fn draw_to_canvas(&self) {
        let Some(view_range) = self.last_sampled_range.clone() else {
            return;
        };

        let (tgt_canvas, tgt_ctx) =
            if let Some(canvas) = self.target_canvas.as_ref() {
                let ctx = canvas.get_context("2d").ok().flatten().unwrap();
                let ctx = ctx
                    .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()
                    .unwrap();

                (canvas, ctx)
            } else {
                return;
            };

        self.render_into_offscreen_canvas();

        let view_size = *view_range.end() - *view_range.start();

        let bin_count = (view_size as usize).min(self.bins.len());
        // let src_width = (view_size as usize).min(bin_count) as f64;
        let src_width = (self.canvas.width() as f64).min(bin_count as f64);

        // let src_width = self.canvas.width() as f64;
        let dst_width = tgt_canvas.width() as f64;
        let dst_height = tgt_canvas.height() as f64;

        web_sys::console::log_1(&format!("src_width: {src_width}").into());
        web_sys::console::log_1(&format!("view size: {view_size}\ndst_width: {dst_width}\ndst_height: {dst_height}").into());

        tgt_ctx.set_image_smoothing_enabled(false);

        tgt_ctx.draw_image_with_offscreen_canvas_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &self.canvas,
            0.,
            0.,
            src_width,
            1.,
            0., 0.,
            dst_width,
            dst_height,
            );
    }
}

#[derive(Debug, Clone)]
#[wasm_bindgen]
pub struct CoordSys {
    node_order: PrimitiveArray<u32>,
    // TODO offsets should probably be i64; maybe generic
    step_offsets: OffsetsBuffer<i32>,
    // step_offsets: PrimitiveArray<u32>,
}

#[wasm_bindgen]
impl CoordSys {
    pub fn global_from_arrow_gfa(graph: &ArrowGFAWrapped) -> Self {
        let node_count = graph.0.segment_count();

        let node_order = arrow2::array::UInt32Array::from_iter(
            (0..node_count as u32).map(Some),
        );

        let step_offsets = graph.0.segment_sequences_array().offsets().clone();

        Self {
            node_order,
            step_offsets,
        }
    }

    pub fn max(&self) -> u64 {
        *self.step_offsets.last() as u64
    }
}

impl CoordSys {
    pub fn bp_to_step_range(
        &self,
        start: u64,
        end: u64,
    ) -> std::ops::RangeInclusive<usize> {
        let start_i = self
            .step_offsets
            .buffer()
            .binary_search_by_key(&start, |&o| o as u64);
        let end_i = self
            .step_offsets
            .buffer()
            .binary_search_by_key(&end, |&o| o as u64);

        let start_out = start_i.unwrap_or_else(|i| i - 1);
        let end_out = end_i.unwrap_or_else(|i| i - 1);

        start_out..=end_out
    }

    pub fn sample_impl(
        &self,
        bp_range: std::ops::RangeInclusive<u64>,
        data_indices: &[u32],
        data: &[f32],
        bins: &mut [f32],
    ) {
        web_sys::console::log_1(&format!("in sample_impl").into());

        // find range in step index using bp_range
        let indices = self.bp_to_step_range(*bp_range.start(), *bp_range.end());

        // slice `data` according to step range
        let s_i = *indices.start();
        let e_i = *indices.end();

        // `indices` is inclusive
        let len = (e_i + 1) - s_i;
        // let data_slice = data.sliced(s_i, len);

        let bp_range_len = (*bp_range.end() + 1) - *bp_range.start();
        let bin_size = bp_range_len / bins.len() as u64;

        // web_sys::console::log_1(&format!("

        let data_ix_start = data_indices
            .binary_search(&(s_i as u32))
            .unwrap_or_else(|i| if i == 0 { 0 } else { i - 1 })
            as usize;

        let make_bin_range = {
            let bin_count = bins.len();
            let s = *bp_range.start();
            let e = *bp_range.end();

            move |bin_i: usize| -> std::ops::RangeInclusive<u64> {
                let i = (bin_i.min(bin_count - 1)) as u64;
                let left = s + i * bin_size;
                let right = s + (i + 1) * bin_size;
                left..=right
            }
        };

        let bin_ranges =
            (0..bins.len()).map(make_bin_range).collect::<Vec<_>>();

        web_sys::console::log_1(&format!("building iterators").into());

        let data_iter = {
            let data_indices = &data_indices[data_ix_start..];
            let data = &data[data_ix_start..];
            std::iter::zip(data_indices, data).map(|(i, v)| (*i as usize, *v))
        };

        let mut bin_length = 0u64;

        web_sys::console::log_1(
            &format!("iterating data ({} steps)", data_iter.len()).into(),
        );

        let mut bin_lengths = vec![0u64; bins.len()];

        for (i, (c_i, val)) in data_iter.enumerate() {
            // web_sys::console::log_1(&format!("step {i}").into());

            let seg_offset =
                *self.step_offsets.buffer().get(c_i).unwrap() as u64;

            if c_i > e_i {
                break;
            }

            let next_seg_offset =
                *self.step_offsets.buffer().get(c_i + 1).unwrap() as u64;

            let mut offset = seg_offset;

            loop {
                // let local_offset = offset - *bp_range.start();
                let local_offset =
                    offset.checked_sub(*bp_range.start()).unwrap_or(0);
                let this_bin = (local_offset / bin_size) as usize;

                if this_bin >= bins.len() {
                    break;
                }

                let cur_bin_range = &bin_ranges[this_bin];
                let next_bin_offset = *cur_bin_range.end() + 1;

                let boundary = next_seg_offset.min(next_bin_offset);
                let this_len = boundary - offset;

                bins[this_bin] += val * this_len as f32;
                bin_lengths[this_bin] += this_len;

                offset = boundary;

                if offset == next_seg_offset {
                    break;
                }
            }
        }

        for (val, len) in bins.iter_mut().zip(bin_lengths) {
            if len != 0 {
                *val = *val / len as f32;
            } else {
                *val = 0f32;
            }
        }

        web_sys::console::log_1(&format!("sample_impl done").into());
    }
}

#[wasm_bindgen]
pub struct SparseData {
    indices: arrow2::array::UInt32Array,
    data: arrow2::array::Float32Array,
}

#[wasm_bindgen]
pub fn arrow_gfa_depth_data(
    graph: &ArrowGFAWrapped,
    path_name: &str,
) -> Result<SparseData, JsValue> {
    let graph = &graph.0;

    let path_index = graph
        .path_name_index(path_name)
        .ok_or::<JsValue>("Path not found".into())?;

    let sprs_vec = graph.path_vector_sparse_u32(path_index);

    let (indices, data) = sprs_vec.into_raw_storage();

    use arrow2::array::{Float32Array, UInt32Array};

    Ok(SparseData {
        indices: UInt32Array::from_vec(indices),
        data: Float32Array::from_iter(data.into_iter().map(|v| Some(v as f32))),
    })
}

#[wasm_bindgen]
pub fn generate_depth_data(
    graph: &PathIndexWrap,
    path_name: &str,
) -> Result<SparseData, JsValue> {
    let graph = &graph.0;
    let path = graph
        .path_names
        .get_by_right(path_name)
        .ok_or::<JsValue>("Path not found".into())?;

    let steps = &graph.path_steps[path.ix()];

    // exploiting that i'm sampling from the global coordinate system for now
    // let indices = graph.path_node_sets[path.ix()].iter().collect::<Vec<_>>();

    let mut depth_map: BTreeMap<u32, f32> = BTreeMap::default();

    for step in steps {
        *depth_map.entry(step.node().ix() as u32).or_default() += 1.0;
    }

    let (indices, data): (Vec<_>, Vec<_>) = depth_map.into_iter().unzip();

    use arrow2::array::{Float32Array, UInt32Array};

    Ok(SparseData {
        indices: UInt32Array::from_vec(indices),
        data: Float32Array::from_vec(data),
    })
}

#[wasm_bindgen]
impl CoordSys {
    pub fn sample_range(
        &self,
        bp_start: JsValue,
        bp_end: JsValue,
        data: &SparseData,
        // data_indices: &[u32],
        // data: &[f32],
        bins: &mut [f32],
    ) {
        let bp_start = bp_start.as_f64().unwrap() as u64;
        let bp_end = bp_end.as_f64().unwrap() as u64;

        let range = (bp_start as u64)..=(bp_end as u64);
        self.sample_impl(
            range,
            &data.indices.values(),
            &data.data.values(),
            bins,
        );
    }
}

pub struct CoordSysWindows<'c> {
    coord_sys: &'c CoordSys,

    bp_range: std::ops::Range<u64>,
}

pub fn hashed_rgb(name: &str) -> [u8; 3] {
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let hash = hasher.finalize();

    let r = hash[24];
    let g = hash[8];
    let b = hash[16];

    [r, g, b]
}

pub fn path_name_hash_color(path_name: &str) -> [f32; 3] {
    let [path_r, path_g, path_b] = hashed_rgb(path_name);

    let r_f = (path_r as f32) / std::u8::MAX as f32;
    let g_f = (path_g as f32) / std::u8::MAX as f32;
    let b_f = (path_b as f32) / std::u8::MAX as f32;

    let sum = r_f + g_f + b_f;

    let r_f = r_f / sum;
    let g_f = g_f / sum;
    let b_f = b_f / sum;

    let f = (1.0 / r_f.max(g_f).max(b_f)).min(1.5);

    let r_u = (255. * (r_f * f).min(1.0)).round();
    let g_u = (255. * (g_f * f).min(1.0)).round();
    let b_u = (255. * (b_f * f).min(1.0)).round();

    let r_f = (r_u as f32) / std::u8::MAX as f32;
    let g_f = (g_u as f32) / std::u8::MAX as f32;
    let b_f = (b_u as f32) / std::u8::MAX as f32;

    [r_f, g_f, b_f]
}
