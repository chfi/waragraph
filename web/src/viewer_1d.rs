use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use arrow2::{array::PrimitiveArray, offset::OffsetsBuffer};
use waragraph_core::graph::{Bp, Node, PathId, PathIndex};

use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, OffscreenCanvas,
};

use crate::{ArrowGFAWrapped, PathIndexWrap};

use self::view::View1D;

pub mod sampler;
pub mod view;

#[wasm_bindgen(module = "/js/util.js")]
extern "C" {
    //
    #[wasm_bindgen(catch)]
    pub(crate) fn create_image_data_impl(
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

    pub fn set_offscreen_canvas_width(&mut self, width: JsValue) {
        if let Some(width) = width.as_f64() {
            let size = width as usize;
            if self.bins.len() < size {
                self.bins.resize(size, 0.0);
            }
            log::debug!("setting canvas width to {width}");
            self.canvas.set_width(size as u32);

            if let Some(target) = self.target_canvas.as_ref() {
                target.set_width(size as u32);
            }
        }
    }

    pub fn new(
        cs: &CoordSys,
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
            let [rf, gf, bf, af] = if val > 0.5 { color_1 } else { color_0 };

            let r = (255.0 * rf) as u8;
            let g = (255.0 * gf) as u8;
            let b = (255.0 * bf) as u8;
            let a = (255.0 * af) as u8;
            [r, g, b, a]
        });

        let bins = vec![0f32; bin_count];

        // web_sys::console::log_1(&format!("BIN COUNT: {bin_count}").into());
        let canvas = OffscreenCanvas::new(bin_count as u32, 1)?;

        Ok(PathViewer {
            cs: cs.clone(),
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

        let bin_subset = &self.bins[..bin_count];

        // web_sys::console::log_1(&format!("binned data: {bin_subset:?}").into());

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

        // web_sys::console::log_1(
        //     &format!("pixel data length: {}", pixel_data.len()).into(),
        // );

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

        // web_sys::console::log_1(&format!("src_width: {src_width}").into());
        // web_sys::console::log_1(&format!("view size: {view_size}\ndst_width: {dst_width}\ndst_height: {dst_height}").into());

        tgt_ctx.clear_rect(0., 0., dst_width, dst_height);

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

    pub fn path_from_arrow_gfa(
        graph: &ArrowGFAWrapped,
        path_index: u32,
    ) -> Self {
        let mut seen_nodes = HashSet::new();

        let steps = &graph.0.path_steps[path_index as usize];

        let mut node_order = Vec::with_capacity(steps.len());
        let mut step_offsets = Vec::with_capacity(steps.len());

        let mut offset = 0i32;
        step_offsets.push(offset);

        for &handle in steps.values_iter() {
            let node = handle >> 1;
            let seg_size = graph.0.segment_len(node);
            offset += seg_size as i32;

            if !seen_nodes.contains(&node) {
                node_order.push(node);
                seen_nodes.insert(node);

                step_offsets.push(offset);
            }
        }

        node_order.shrink_to_fit();
        step_offsets.shrink_to_fit();

        let node_order = arrow2::array::UInt32Array::from_vec(node_order);
        let step_offsets = OffsetsBuffer::try_from(step_offsets).unwrap();

        Self {
            node_order,
            step_offsets,
        }
    }

    pub fn max(&self) -> u64 {
        *self.step_offsets.last() as u64
    }

    pub fn max_f64(&self) -> f64 {
        self.max() as f64
    }

    #[wasm_bindgen(js_name = "bp_to_step_range")]
    pub fn bp_to_step_range_js(&self, start: u64, end: u64) -> js_sys::Object {
        let (s_i, e_i) = self.bp_to_step_range(start, end).into_inner();

        let obj = js_sys::Object::new();

        for (k, v) in [("start", s_i as f64), ("end", e_i as f64)] {
            js_sys::Reflect::set(obj.as_ref(), &k.into(), &v.into());
        }

        obj
    }

    pub fn segment_at_pos(&self, pos: u64) -> Result<u32, JsValue> {
        let pos_i = self
            .step_offsets
            .buffer()
            .binary_search_by_key(&pos, |&o| o as u64);

        let pos_out = pos_i.unwrap_or_else(|i| i - 1);

        Ok(pos_out as u32)
    }
}

impl CoordSys {
    pub fn segment_range(&self, segment: u32) -> Option<std::ops::Range<u64>> {
        let ix = segment as usize;

        if ix >= self.step_offsets.len() {
            return None;
        }

        let (start, end) = self.step_offsets.start_end(ix);

        Some((start as u64)..(end as u64))
    }

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

    pub fn sample_impl_new(
        &self,
        bp_range: std::ops::RangeInclusive<u64>,
        data_indices: &[u32],
        data: &[f32],
        bins: &mut [f32],
    ) {
        // clear bins
        // bins.iter_mut().for_each(|bin| *bin = 0.0);

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

        let mut data_iter = CoordSysDataIter::new(
            &self,
            data_indices,
            data,
            (s_i as u32)..(e_i as u32 - 1),
        );

        // let data_ix_start = data_indices
        //     .binary_search(&(s_i as u32))
        //     .unwrap_or_else(|i| if i == 0 { 0 } else { i - 1 })
        //     as usize;

        // let mut data_iter = data_indices[data_ix_start..]
        //     .iter().enumerate().map(|(i, data)| {
        //         //
        //     });

        for (bin_i, bin) in bins.iter_mut().enumerate() {
            *bin = 0.0;

            let (start, end) = make_bin_range(bin_i).into_inner();

            // iterate through data adding to bin here
        }
    }

    pub fn sample_impl(
        &self,
        bp_range: std::ops::RangeInclusive<u64>,
        data_indices: &[u32],
        data: &[f32],
        bins: &mut [f32],
    ) {
        if bins.is_empty() {
            log::error!("bins are empty -- this should never happen!");
            return;
        }

        web_sys::console::log_1(&format!("in sample_impl").into());

        for bin in bins.iter_mut() {
            *bin = 0.0;
        }

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

        for (i, range) in bin_ranges.iter().enumerate() {
            let start = range.start();
            let end = range.end();
        }

        let data_iter = {
            let data_indices = &data_indices[data_ix_start..];
            let data = &data[data_ix_start..];
            std::iter::zip(data_indices, data).map(|(i, v)| (*i as usize, *v))
        };

        let mut bin_length = 0u64;

        let mut bin_lengths = vec![0u64; bins.len()];

        for (i, (c_i, val)) in data_iter.enumerate() {
            let seg_offset =
                *self.step_offsets.buffer().get(c_i).unwrap() as u64;

            if c_i > e_i {
                break;
            }

            let next_seg_offset =
                *self.step_offsets.buffer().get(c_i + 1).unwrap() as u64;

            // log::debug!("segment length: {}", next_seg_offset - seg_offset);

            let mut offset = seg_offset;

            loop {
                let local_offset =
                    offset.checked_sub(*bp_range.start()).unwrap_or(0);

                let this_bin = (local_offset / bin_size) as usize;

                if this_bin >= bins.len() {
                    break;
                }

                let cur_bin_range = &bin_ranges[this_bin];
                let next_bin_offset = *cur_bin_range.end();

                let boundary = next_seg_offset.min(next_bin_offset);

                let start_offset = offset.max(*cur_bin_range.start());

                // this is kinda busted; just to avoid crashes & infinite loops for now : )
                if boundary < start_offset {
                    break;
                }
                let this_len = boundary - start_offset;
                // log::debug!("this_len: {this_len}");

                bins[this_bin] += val * this_len as f32;
                bin_lengths[this_bin] += this_len;

                offset = boundary;

                if start_offset == next_seg_offset {
                    // web_sys::console::log_1(
                    //     &format!("break; cur offset matches next seg, {offset} == {next_seg_offset}").into(),
                    // );

                    break;
                }
            }
        }

        // web_sys::console::log_1(&format!("{bin_lengths:?}").into());

        for (val, len) in bins.iter_mut().zip(bin_lengths) {
            if len != 0 {
                *val = *val / len as f32;
            } else {
                *val = 0f32;
            }
        }

        // web_sys::console::log_1(&format!("sample_impl done").into());
    }
}

#[wasm_bindgen]
pub struct SegmentRanges {
    ranges: Vec<[u32; 2]>,
}

// NB: views and ranges here are all global -- maybe views should be
// associated with/derived from coordinate systems...
#[wasm_bindgen]
impl SegmentRanges {
    pub fn ranges_as_u32_array(&self) -> js_sys::Uint32Array {
        let ptr = self.ranges.as_ptr();
        let len = self.ranges.len();

        let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
        js_sys::Uint32Array::new_with_byte_offset_and_length(
            &memory.buffer(),
            ptr as u32,
            len as u32,
        )
    }

    pub fn to_canvas_ranges(
        &self,
        view: &View1D,
        canvas_width: f64,
    ) -> Vec<f32> {
        let v_range = view.range_bp();
        let v_len = view.len();

        let v_start = v_range.start as u32;
        let v_end = v_range.end as u32;

        self.ranges
            .iter()
            .filter_map(|&[r_start, r_end]| {
                // if range overlaps view
                if r_end < v_start || r_start > v_end {
                    return None;
                }

                let l_u = r_start.checked_sub(v_range.start as u32)?;
                let r_u = r_end.checked_sub(v_range.start as u32)?;

                let l_n = (l_u as f64) / v_len;
                let r_n = (r_u as f64) / v_len;

                // transform range to view coordinates
                let l = (l_n * canvas_width) as f32;
                let r = (r_n * canvas_width) as f32;

                Some([l, r])
            })
            .flatten()
            .collect()
    }
}

#[wasm_bindgen]
pub fn path_slice_to_global_adj_partitions(
    path_steps: &[u32],
) -> Result<SegmentRanges, JsValue> {
    if path_steps.is_empty() {
        return Err(JsValue::from(format!("empty path slice")));
    }

    // only handling the global node order for now, so instead of
    // dealing with an explicit coordinate system just sort
    let mut sorted = path_steps.into_iter().copied().collect::<Vec<_>>();
    sorted.sort();

    log::warn!("sorted.len() {}", sorted.len());

    let mut ranges = Vec::new();

    let mut range_start = sorted[0] >> 1;
    let mut prev_seg_ix = sorted[0] >> 1;

    for handle in sorted {
        let seg_ix = handle >> 1;

        if seg_ix.abs_diff(prev_seg_ix) > 2 {
            ranges.push([range_start, prev_seg_ix]);
            range_start = seg_ix;
        }

        prev_seg_ix = seg_ix;
    }

    Ok(SegmentRanges { ranges })
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

    pub fn offset_at(&self, cs_seg: u32) -> Result<u32, JsValue> {
        let v = self.step_offsets.get(cs_seg as usize).ok_or_else(|| {
            JsValue::from(format!("Segment index {cs_seg} not found"))
        })?;

        Ok(*v as u32)
    }
}

struct CoordSysDataIter<'a, 'b> {
    coord_sys: &'a CoordSys,
    data_indices: &'b [u32],
    data_values: &'b [f32],
    // bp_range:
}

impl<'a, 'b> CoordSysDataIter<'a, 'b> {
    fn new(
        coord_sys: &'a CoordSys,
        data_indices: &'b [u32],
        data_values: &'b [f32],
        segment_range: std::ops::Range<u32>,
    ) -> Self {
        // find the indices in `data_indices` that correspond to the `segment_range`
        let data_ix_start = data_indices
            .binary_search(&segment_range.start)
            .unwrap_or_else(|i| if i == 0 { 0 } else { i - 1 })
            as usize;

        let data_ix_end = data_indices
            .binary_search(&segment_range.end)
            .unwrap_or_else(|i| if i == 0 { 0 } else { i - 1 })
            as usize;

        let data_indices = &data_indices[data_ix_start..data_ix_end];
        let data_values = &data_values[data_ix_start..data_ix_end];

        Self {
            coord_sys,
            data_indices,
            data_values,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CoordSysDataIterOutput<T> {
    segment: u32,
    bp_start: u64,
    bp_end: u64,
    value: T,
}

impl<'a, 'b> Iterator for CoordSysDataIter<'a, 'b> {
    type Item = CoordSysDataIterOutput<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data_indices.is_empty() {
            return None;
        }

        let segment = self.data_indices[0];
        let value = self.data_values[0];

        let range = self.coord_sys.segment_range(segment)?;

        if self.data_indices.len() > 1 {
            self.data_indices = &self.data_indices[1..];
            self.data_values = &self.data_values[1..];
        }

        Some(CoordSysDataIterOutput {
            segment,
            bp_start: range.start,
            bp_end: range.end,
            value,
        })
    }
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

#[wasm_bindgen]
pub fn path_name_hash_color_obj(path_name: &str) -> JsValue {
    let mut color: JsValue = js_sys::Object::new().into();

    let [r, g, b] = path_name_hash_color(path_name);

    let set_color = |chn: &str, v: f32| {
        js_sys::Reflect::set(
            &color,
            &JsValue::from_str(chn),
            &JsValue::from_f64(v as f64),
        );
    };

    set_color("r", r);
    set_color("g", g);
    set_color("b", b);
    set_color("a", 1.0);

    color
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

struct ByteIntervalIter<'a> {
    offsets: &'a OffsetsBuffer<i32>,
    byte_range: std::ops::Range<i32>,
    current_index: usize,
}

impl<'a> ByteIntervalIter<'a> {
    fn new(
        offsets: &'a OffsetsBuffer<i32>,
        byte_range: std::ops::Range<i32>,
    ) -> Self {
        let start_index = offsets
            .binary_search(&byte_range.start)
            .unwrap_or_else(|x| x);
        ByteIntervalIter {
            offsets,
            byte_range,
            current_index: start_index,
        }
    }
}

impl<'a> Iterator for ByteIntervalIter<'a> {
    type Item = (usize, std::ops::RangeInclusive<i32>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(&start_offset) = self.offsets.get(self.current_index) {
            let end_offset = *self
                .offsets
                .get(self.current_index + 1)
                .unwrap_or(&self.byte_range.end);
            if start_offset < self.byte_range.end {
                let local_start = if start_offset < self.byte_range.start {
                    self.byte_range.start - start_offset
                } else {
                    0
                };
                let local_end = if end_offset > self.byte_range.end {
                    self.byte_range.end - start_offset
                } else {
                    end_offset - start_offset
                };
                self.current_index += 1;
                return Some((self.current_index - 1, local_start..=local_end));
            }
        }
        None
    }
}

fn iter_binary_interval<'a>(
    array: &'a arrow2::array::BinaryArray<i32>,
    byte_range: std::ops::Range<i32>,
) -> impl Iterator<Item = &'a [u8]> + 'a {
    ByteIntervalIter::new(&array.offsets(), byte_range).map(
        move |(index, local_range)| {
            let (s, e) = local_range.into_inner();
            let range = (s as usize)..=(e as usize);
            &array.value(index)[range]
        },
    )
}
