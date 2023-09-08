use std::{collections::BTreeMap, sync::Arc};

use arrow2::array::PrimitiveArray;
use waragraph_core::graph::{Bp, Node, PathId, PathIndex};

use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, OffscreenCanvas,
};

use crate::PathIndexWrap;

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
    cs: Arc<CoordSys>,
    data: SparseData,
    bins: Vec<f32>,

    // graph: Arc<PathIndex>,
    // path: PathId,
    color_0: [f32; 4],
    color_1: [f32; 4],

    canvas: Option<OffscreenCanvas>,
}

#[wasm_bindgen]
impl PathViewer {
    // #[wasm_bindgen(getter)]
    // pub fn coord_sys(&self) -> Arc<CoordSys> {
    //     self.cs.clone()
    // }

    #[wasm_bindgen(getter)]
    pub fn get_bin_data(&self) -> Box<[f32]> {
        self.bins.clone().into_boxed_slice()
    }

    pub fn sample_range(&mut self, bp_start: JsValue, bp_end: JsValue) {
        let bp_start = bp_start.as_f64().unwrap() as u64;
        let bp_end = bp_end.as_f64().unwrap() as u64;

        let range = (bp_start as u64)..=(bp_end as u64);
        self.cs.sample_impl(
            range,
            &self.data.indices,
            &self.data.data,
            &mut self.bins,
        );
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

        let bins = vec![0f32; bin_count];

        Ok(PathViewer {
            cs: Arc::new(cs),
            data,
            bins,
            color_0,
            color_1,

            canvas: None,
        })
    }

    pub fn set_canvas(&mut self, canvas: OffscreenCanvas) {
        web_sys::console::log_1(&format!("setting canvas").into());
        self.canvas = Some(canvas);
    }

    pub fn new_canvas(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        let canvas = OffscreenCanvas::new(width, height)?;
        self.canvas = Some(canvas);
        Ok(())
    }

    // pub fn new(graph: Arc<PathIndex>, path: PathId, bin_count: usize) -> Self {
    //     let path_name = graph.path_names.get_by_left(&path).unwrap();
    //     let [r, g, b] = path_name_hash_color(path_name);

    //     Self {
    //         graph,
    //         path,
    //         bins: vec![0f32; bin_count],
    //         color: [r, g, b, 1.],
    //     }
    // }
}

#[wasm_bindgen]
impl PathViewer {
    pub fn transfer_canvas_control_to_self(
        &mut self,
        canvas: HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        let offscreen = canvas.transfer_control_to_offscreen()?;

        self.canvas = Some(offscreen);

        Ok(())
    }

    pub fn render_into_new_buffer(&self) -> Box<[u8]> {
        let mut pixel_data: Vec<u8> = vec![0; self.bins.len() * 4];

        web_sys::console::log_1(
            &format!("pixel data length: {}", pixel_data.len()).into(),
        );

        let pixels = pixel_data.chunks_exact_mut(4);

        for (color, val) in pixels.zip(&self.bins) {
            // let [rf, gf, bf, af] = if *val > 0.5 {
            let [rf, gf, bf, af] = if *val > 1.0 {
                self.color_0
            } else {
                self.color_1
            };

            color[0] = (255.0 * rf) as u8;
            color[1] = (255.0 * gf) as u8;
            color[2] = (255.0 * bf) as u8;
            color[3] = (255.0 * af) as u8;
        }

        pixel_data.into_boxed_slice()
    }

    pub fn canvas_test(&self) {
        let (canvas, ctx) = if let Some(canvas) = self.canvas.as_ref() {
            let ctx = canvas.get_context("2d").ok().flatten().unwrap();
            let ctx = ctx
                .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()
                .unwrap();

            (canvas, ctx)
        } else {
            return;
        };

        ctx.set_fill_style(&"blue".into());
        ctx.fill_rect(20.0, 20.0, 80.0, 80.0);
    }

    pub fn draw_to_canvas(&self) {
        web_sys::console::log_1(&format!("getting canvas...").into());
        let (canvas, ctx) = if let Some(canvas) = self.canvas.as_ref() {
            let ctx = canvas.get_context("2d").ok().flatten().unwrap();
            let ctx = ctx
                .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()
                .unwrap();

            (canvas, ctx)
        } else {
            return;
        };

        let w = canvas.width();
        let h = canvas.height();

        web_sys::console::log_1(&format!("render_into_new_buffer").into());

        let pixels = self.render_into_new_buffer();
        let px_len = pixels.len() as u32;

        web_sys::console::log_1(&format!("create image data").into());

        let memory = wasm_bindgen::memory();

        let pixels_ptr = pixels.as_ptr() as *const u8;

        web_sys::console::log_1(
            &format!(
                "pixel 0: [{},{},{},{}]",
                pixels[0], pixels[1], pixels[2], pixels[3],
            )
            .into(),
        );

        let image_data = create_image_data_impl(memory, pixels_ptr, px_len);
        // let image_data = create_image_data_impl(pixels, px_len / 4);

        // let image_data = create_image_data_impl(pixels, px_len);

        match image_data {
            Ok(image_data) => {
                // web_sys::console::log_1(&e);

                web_sys::console::log_1(&format!("putting image data").into());

                let _ = ctx.put_image_data_with_dirty_x_and_dirty_y_and_dirty_width_and_dirty_height(
                &image_data,
                0., 0.,
                0., 0.,
                w as f64,
                h as f64,
                );
            }
            Err(e) => {
                web_sys::console::log_1(
                    &format!("error creating image data").into(),
                );
                let is_undef = e.is_undefined();
                let is_null = e.is_null();
                web_sys::console::log_2(&"error!!!".into(), &e);
                web_sys::console::log_1(
                    &format!(
                        "error undefined {is_undef}, error null {is_null}"
                    )
                    .into(),
                );
            }
        }
    }
}

#[wasm_bindgen]
pub struct CoordSys {
    node_order: PrimitiveArray<u32>,
    // TODO offsets should probably be u64; maybe generic
    step_offsets: PrimitiveArray<u32>,
}

#[wasm_bindgen]
impl CoordSys {
    pub fn global_from_graph(graph: &PathIndexWrap) -> Self {
        Self::from_node_order(
            graph,
            (0..graph.0.node_count).map(|i| Node::from(i)),
        )
    }
}

impl CoordSys {
    pub fn from_node_order(
        graph: &PathIndexWrap,
        node_order: impl Iterator<Item = Node>,
    ) -> Self {
        let node_order =
            PrimitiveArray::from_iter(node_order.map(|n| Some(n.ix() as u32)));

        let mut step_offset_vals = Vec::with_capacity(node_order.len() + 1);

        let mut offset = 0u32;

        for &n_i in node_order.values_iter() {
            let node = Node::from(n_i as usize);
            let length = graph.0.node_length(node);
            step_offset_vals.push(offset);
            offset += length.0 as u32;
        }
        // `step_offsets` will contain N+1 values to encode the final length
        // might change later
        step_offset_vals.push(offset);

        let step_offsets = PrimitiveArray::from_vec(step_offset_vals);

        Self {
            node_order,
            step_offsets,
        }
    }

    pub fn from_path(graph: &PathIndexWrap, path: PathId) -> Self {
        todo!();
        // let node_order = PrimitiveArray::from_iter(
        //     graph.path_steps[path.ix()]
        //         .iter()
        //         .map(|i| Some(i.ix() as i32)),
        // );

        // let step_offsets = Arc::new(graph.path_step_offsets[path.ix()].clone());

        // Self {
        //     node_order,
        //     step_offsets,
        // }
    }

    pub fn bp_to_step_range(
        &self,
        start: u64,
        end: u64,
    ) -> std::ops::RangeInclusive<usize> {
        let start_i = self
            .step_offsets
            .values()
            .binary_search_by_key(&start, |&o| o as u64);
        let end_i = self
            .step_offsets
            .values()
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

            let seg_offset = self.step_offsets.get(c_i).unwrap() as u64;

            if c_i > e_i {
                break;
            }

            let next_seg_offset =
                self.step_offsets.get(c_i + 1).unwrap() as u64;

            let mut offset = seg_offset;

            loop {
                let local_offset = offset - *bp_range.start();
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

                /*
                if boundary == next_bin_offset {
                    // close this bin
                    // let bin_val = val * bin_length as f32;
                    if bin_length > 0 {
                        bins[this_bin] = bins[this_bin] / bin_length as f32;
                    }
                    bin_length = 0;
                }
                */

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
    }

    pub fn sample_range_impl(
        &self,
        bp_range: std::ops::RangeInclusive<u64>,
        // data: PrimitiveArray<f32>,
        data: &[f32],
        bins: &mut [f32],
    ) {
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

        // let mut data_iter = data_slice.iter().enumerate();
        let segment_slice = self.node_order.clone().sliced(s_i, len);

        let mut seg_data_iter = segment_slice.iter().zip(data);

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

        let mut bin_length = 0u64;
        let mut last_bin = 0usize;

        let mut cur_bin_range = make_bin_range(last_bin);
        let mut last_offset = 0;

        /*

        // iterate through the data, filling bins along the way
        // since the data is sorted by the node order, we're also
        // iterating through the bins -- once we see a new bin,
        // the previous is done
        for (seg_i, &val) in seg_data_iter {
            let seg_i = if let Some(seg_i) = seg_i {
                *seg_i
            } else {
                continue;
            };

            let step_i = s_i + i;
            let offset = self.step_offsets.get(step_i).unwrap();
            let next_step_offset = self.step_offsets.get(step_i + 1).unwrap();
            let len = next_step_offset - offset;

            let local_offset = offset as u64 - *bp_range.start();
            let this_bin = (local_offset / bin_size) as usize;

            let mut offset = offset as u64;

            loop {
                let cur_bin_range = make_bin_range(this_bin);
                let next_bin_offset = *cur_bin_range.end() + 1;

                // step through the node across bins, if necessary
                let boundary = (next_step_offset as u64).min(next_bin_offset);
                let this_len = boundary - offset;

                if boundary == next_bin_offset {
                    // close this bin
                    let bin_val = val * bin_length as f32;
                    bins[this_bin] = bin_val;
                    bin_length = 0;
                }

                offset = boundary;

                if offset == next_step_offset as u64 {
                    break;
                }
            }
        }
        */
    }
}

#[wasm_bindgen]
pub struct SparseData {
    indices: Vec<u32>,
    data: Vec<f32>,
}

#[wasm_bindgen]
impl SparseData {
    //
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
    // let mut depth_map = vec![0f32; indices.len()];

    for step in steps {
        *depth_map.entry(step.node().ix() as u32).or_default() += 1.0;
    }

    let (indices, data): (Vec<_>, Vec<_>) = depth_map.into_iter().unzip();

    Ok(SparseData { indices, data })
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
        self.sample_impl(range, &data.indices, &data.data, bins);
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
