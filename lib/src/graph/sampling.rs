use std::collections::BTreeMap;

use super::{Node, PathId, PathIndex};

pub trait PathData<T> {
    fn get_path(&self, path_id: PathId) -> &[T];
}

pub fn sample_path_data_into_buffer<D>(
    index: &PathIndex,
    data: &D,
    paths: impl IntoIterator<Item = PathId>,
    bins: usize,
    view_range: std::ops::Range<u64>,
    out: &mut [u8],
) where
    D: PathData<f32>,
{
    let paths = paths.into_iter().collect::<Vec<_>>();

    // the part that holds the row size & total size
    let prefix_size = std::mem::size_of::<u32>() * 4;

    let bins = ((view_range.end - view_range.start) as usize).min(bins);

    let elem_size = std::mem::size_of::<f32>();
    let needed_size = prefix_size + elem_size * bins * paths.len();

    // TODO: return Err when `out` has to be reallocated
    assert!(out.len() >= needed_size);

    let row_size = bins;
    let total_size = row_size * paths.len();

    out[0..16].clone_from_slice(bytemuck::cast_slice(&[
        total_size as u32,
        row_size as u32,
        0,
        0,
    ]));

    let data_offset = 16;

    let bin_range = {
        let s = view_range.start;
        let e = view_range.end;
        let len = e - s;

        let bin_size = len / bins as u64;

        move |bin_ix: usize| {
            let start = s + bin_size * bin_ix as u64;
            let end = start + bin_size;
            start..end
        }
    };

    let row_size = elem_size * row_size;

    for (ix, path_id) in paths.into_iter().enumerate() {
        let offset = data_offset + ix * row_size;
        let range = offset..(offset + row_size);

        let path_data = data.get_path(path_id);
        let buf_row: &mut [f32] = bytemuck::cast_slice_mut(&mut out[range]);

        for (buf_val, bin_ix) in buf_row.iter_mut().zip(0..bins) {
            let range = bin_range(bin_ix);
            let iter =
                index.path_data_pan_range_iter(range, path_id, path_data);

            let mut sum_len = 0;
            let mut sum_val = 0.0;

            for ((_node, len), val) in iter {
                sum_len += len.0;
                sum_val += *val * len.0 as f32;
            }

            *buf_val = sum_val / sum_len as f32;
        }
    }
}

pub struct PathDepthData {
    pub node_depth_per_path: Vec<Vec<f32>>,
}

impl PathData<f32> for PathDepthData {
    fn get_path(&self, path_id: PathId) -> &[f32] {
        &self.node_depth_per_path[path_id.ix()]
    }
}

impl PathDepthData {
    pub fn new(path_index: &PathIndex) -> Self {
        let mut data = Vec::new();

        for (path_id, _node_set) in path_index.path_node_sets.iter().enumerate()
        {
            let mut path_data: BTreeMap<Node, f32> = BTreeMap::default();
            for step in path_index.path_steps[path_id].iter() {
                *path_data.entry(step.node()).or_default() += 1.0;
            }
            let path_data =
                path_data.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
            data.push(path_data);
        }

        Self {
            node_depth_per_path: data,
        }
    }
}
