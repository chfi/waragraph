use waragraph_core::graph::{Node, PathIndex};

pub trait PathDataSource<T> {
    fn get(&self, node: Node) -> Option<T>;
}

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
