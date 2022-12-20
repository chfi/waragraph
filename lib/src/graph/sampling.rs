use std::collections::BTreeMap;

use super::{Node, PathIndex};

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

pub struct PathDepthData {
    pub node_depth_per_path: Vec<Vec<f32>>,
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
