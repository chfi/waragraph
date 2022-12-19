use std::collections::BTreeMap;

use waragraph_core::graph::{Node, PathIndex};

pub trait PathPangenomeRangeData<T> {
    fn get(
        &self,
        path_index: &PathIndex,
        path: usize,
        pan_range: std::ops::Range<u64>,
    ) -> Option<T>;
}

pub fn sample_path_data<'a, T>(
    index: &PathIndex,
    path_id: usize,
    data: &'a [T],
    pos: u64,
) -> Option<&'a T> {
    // pangenome pos -> node ID
    let node_ix = index.segment_offsets.rank(pos) as u32;

    let path_set = index.path_node_sets.get(path_id)?;
    if !path_set.contains(node_ix) {
        return None;
    }

    let data_id = path_set.rank(node_ix);
    data.get(data_id as usize)
}

// pub fn path_data_bin_iter<'d>(path_index: &PathIndex,
// path: usize,
// data: &'d [f32],
// pan_range: std::ops::Range<u64>,
// ) -> PathDataBinIter<'d> {

// }

pub struct PathDataBinIter<'index, 'data> {
    path_id: usize,

    segment_offsets: &'index roaring::RoaringTreemap,
    path_nodes: &'index roaring::RoaringBitmap,
    data: &'data [f32],

    ix_range: std::ops::Range<usize>,

    offset: u64,
    index: u64,
    /*
    path_index: &'index PathIndex,
    path_id: usize,
    data: &'data [f32],
    */
}

impl<'index, 'data> PathDataBinIter<'index, 'data> {
    pub(crate) fn new(
        path_index: &'index PathIndex,
        path_id: usize,
        data: &'data [f32],
        pos_range: std::ops::Range<u64>,
    ) -> Option<Self> {
        let segment_offsets = &path_index.segment_offsets;
        let path_nodes = path_index.path_node_sets.get(path_id)?;

        let start = pos_range.start;
        let end = pos_range.end;
        let offset = pos_range.start;
        let start_rank = segment_offsets.rank(start);
        let end_rank = segment_offsets.rank(end);

        let rank_range = start_rank..end_rank;

        // let index = path_nodes.rank(node_ix)

        todo!();
    }

    fn next_impl(&mut self) -> Option<(f32, u64)> {
        todo!();
    }
}

/*
impl<'index, 'data> Iterator for PathDataBinIter<'index, 'data> {
    type Item = (f32, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.ix_range.end {
            return None;
        }

        let path_set = self.path_index.path_node_sets.get(self.path_id)?;
        let data_ix = self.index;
        // let node_ix = self.path_index.

        // let node_offset = self.path_index.segment_offsets.select()
        // let value = self.data[self.index];
        // let length =

        self.index += 1;

        todo!();
    }
}
*/

pub struct PathDepthData {
    pub(crate) node_depth_per_path: Vec<Vec<f32>>,
}

impl PathDepthData {
    pub fn new(path_index: &PathIndex) -> Self {
        let mut data = Vec::new();

        for (path_id, node_set) in path_index.path_node_sets.iter().enumerate()
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

impl PathPangenomeRangeData<f32> for PathDepthData {
    fn get(
        &self,
        path_index: &PathIndex,
        path: usize,
        pan_range: std::ops::Range<u64>,
    ) -> Option<f32> {
        let s = pan_range.start;
        let e = pan_range.end;
        if e - s == 0 {
            return None;
        }

        let mid = s + (e - s) / 2;

        let data = &self.node_depth_per_path[path];
        let val = sample_path_data(path_index, path, data, mid)?;
        Some(*val)
    }
}

// pub trait PathDataSource<T> {
//     fn get(&self, node: Node) -> Option<T>;
// }

/*

sampling path prefix sum data using the pangenome space offset bitmap
and the path node set bitmap

we want to sample a range of the pangenome space into bins; we can focus
on the case of a single bin.

the data we're sampling from will, at this stage, be a prefix sum vector
constructed from some data vector on the path. the prefix sum is taken
in the node ID order, so when sampling from the pangenome space, we can
just take the values at the endpoints of the bin and divide by the bin
size.

however, there's also the fact that paths will have gaps in the pangenome
view. does that really pose a problem, though?
 */

/// panics if `data.len() != path_steps.len() + 1`
pub fn sample_path_prefix_sum_mean(
    path_index: &PathIndex,
    path_ix: usize,
    data: &[f32],
    bin_range: std::ops::Range<u64>,
) -> f32 {
    todo!();
}

/// Given a bitmap of segment offsets (defining the 1D pangenome space),
/// a view (a range of the pangenome space), and a sample count,
/// fill `samples` with `sample_count` values. The first `u32` denotes
/// the size of the `Node` within the sample, the second `u32` is the
/// sample size (in bp).
pub fn sample_pangenome_single_node(
    segment_offsets: roaring::RoaringTreemap,
    view_range: std::ops::Range<u64>,
    sample_count: usize,
    samples: &mut Vec<(Node, u32, u32)>,
) {
    samples.clear();

    let start = view_range.start;
    let end = view_range.end;
    let len = start - end;

    let sample_width = len as f64 / sample_count as f64;
    let p0 = start as f64;

    for i in 0..=sample_count {
        let p = (p0 + i as f64 * sample_width) as u64;

        let rank = segment_offsets.rank(p);
        let offset = segment_offsets.select(rank).unwrap();
        let next_offset = segment_offsets
            .select(rank + 1)
            .unwrap_or(segment_offsets.len());

        // let node_size = segment_offsets.select(n)

        let node = Node::from(rank as u32);
        let node_size = next_offset - offset;
        let sample_size = sample_width as u32;

        samples.push((node, node_size as u32, sample_size));
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn sample_single_bp_data() {
        //
    }
}
