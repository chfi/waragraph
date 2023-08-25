use std::sync::Arc;

use arrow2::array::PrimitiveArray;
use waragraph_core::graph::{Bp, Node, PathId, PathIndex};

use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

pub mod sampler;

#[wasm_bindgen]
pub struct PathViewer {
    graph: Arc<PathIndex>,
    path: PathId,

    bins: Vec<f32>,

    color: [f32; 4],
    // canvas: Option<HtmlCanvasElement>,
}

impl PathViewer {
    pub fn new(graph: Arc<PathIndex>, path: PathId, bin_count: usize) -> Self {
        let path_name = graph.path_names.get_by_left(&path).unwrap();
        let [r, g, b] = path_name_hash_color(path_name);

        Self {
            graph,
            path,
            bins: vec![0f32; bin_count],
            color: [r, g, b, 1.],
        }
    }
}

#[wasm_bindgen]
impl PathViewer {
    /*
    pub fn draw_to_canvas(
        &self,
        view_start: f64,
        view_end: f64,
        canvas: HtmlCanvasElement,
    ) {
        use sampler::{PathNodeSetSampler, Sampler};

        let view_start = view_start as u64;
        let view_end = view_end as u64;

        let color = self.color;

        let t0 = instant::now();
        let t = instant::Instant::now();

        let sampler =
            PathNodeSetSampler::new(self.graph.clone(), |_path, _val| 1.);

        let result = sampler.sample_range(
            self.bins.len(),
            self.path,
            Bp(view_start)..Bp(view_end),
        );
        let t1 = instant::now();
        web_sys::console::log_1(&format!("dt: {}", t1 - t0).into());

        let dt = t.elapsed().as_secs_f64();
        web_sys::console::log_1(&format!("dt: {dt}").into());

        match result {
            Ok(mut buf) => {
                let pixels: &mut [[u8; 4]] =
                    bytemuck::cast_slice_mut(buf.as_mut_slice());

                let width = pixels.len();

                for px in pixels {
                    // let [r, g, b, a] = px;
                    let val: f32 = bytemuck::cast(*px);

                    if val > 0.0 {
                        let [r, g, b, a] = self.color;
                        let r = (r * 255.0) as u8;
                        let g = (g * 255.0) as u8;
                        let b = (b * 255.0) as u8;
                        let a = (a * 255.0) as u8;

                        *px = [r, g, b, a];
                    } else {
                        *px = [0, 0, 0, 255];
                    }
                }

                // let data = Uint8ClampedArray::from(bytemuck::cast_slice(&buf));

                let data = Clamped(buf.as_slice());
                // let img_data = web_sy

                // use
                let img_data = web_sys::ImageData::new_with_u8_clamped_array(
                    data,
                    width as u32,
                )
                .unwrap();

                let window = web_sys::window().unwrap();

                let img_bitmap_promise = window
                    .create_image_bitmap_with_image_data(&img_data)
                    .unwrap();

                let ctx = canvas.get_context("2d").unwrap().unwrap();
                let ctx: CanvasRenderingContext2d = ctx.dyn_into().unwrap();

                use wasm_bindgen_futures::{
                    future_to_promise, spawn_local, JsFuture,
                };

                let future = JsFuture::from(img_bitmap_promise);

                spawn_local(async move {
                    let bitmap = future.await.unwrap();
                    let bitmap: web_sys::ImageBitmap =
                        bitmap.dyn_into().unwrap();

                    let sx = 0.;
                    let sy = 0.;

                    let sw = width as f64;
                    let sh = 1.;

                    let dx = 0.;
                    let dy = 0.;

                    let dw = canvas.width() as f64;
                    let dh = canvas.height() as f64;

                    ctx.draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                        &bitmap,
                        sx,
                        sy,
                        sw,
                        sh,
                        dx,
                        dy,
                        dw,
                        dh,
                    ).unwrap();
                    //
                });
            }
            Err(e) => {
                web_sys::console::log_1(
                    &format!("PathViewer.draw_to_canvas error: {e:?}").into(),
                );
            }
        }

        //
    }
    */
}

#[wasm_bindgen]
pub struct CoordSys {
    node_order: PrimitiveArray<u32>,
    // TODO offsets should probably be u64; maybe generic
    step_offsets: PrimitiveArray<u32>,
}

impl CoordSys {
    pub fn from_node_order(
        graph: &PathIndex,
        node_order: impl Iterator<Item = Node>,
    ) -> Self {
        let node_order =
            PrimitiveArray::from_iter(node_order.map(|n| Some(n.ix() as u32)));

        let mut step_offset_vals = Vec::with_capacity(node_order.len() + 1);

        let mut offset = 0u32;

        for &n_i in node_order.values_iter() {
            let node = Node::from(n_i as usize);
            let length = graph.node_length(node);
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

    pub fn global_from_graph(graph: &PathIndex) -> Self {
        Self::from_node_order(
            graph,
            (0..graph.node_count).map(|i| Node::from(i)),
        )
    }

    pub fn from_path(graph: &PathIndex, path: PathId) -> Self {
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
impl CoordSys {
    pub fn sample_range(
        &self,
        bp_start: u32,
        bp_end: u32,
        // data: PrimitiveArray<f32>,
        bins: &mut [f32],
    ) {
        // let range = (bp_start as u64)..=(bp_end as u64);
        // self.sample_range(range, data, bins);
    }
}

pub struct CoordSysWindows<'c> {
    coord_sys: &'c CoordSys,

    bp_range: std::ops::Range<u64>,
}

// i don't really need path data as such, rather the node lengths (graph level)

// however, i also want to support sampling in other ways, or using
// other coordinate systems

// is the node length array (prefix sum), produced by sorting the node lengths
// by the path order, the coordinate system?
fn sample_from_path(path: (), data: ()) {
    //
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