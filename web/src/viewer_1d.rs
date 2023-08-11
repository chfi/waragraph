use std::sync::Arc;

use arrow2::array::PrimitiveArray;
use waragraph_core::graph::{PathId, PathIndex};

pub struct PathViewer {
    //
}

pub struct CoordSys {
    node_order: PrimitiveArray<i32>,
    step_offsets: Arc<roaring::RoaringTreemap>,
    // step_offsets: roaring::RoaringTreemap,
}

impl CoordSys {
    pub fn global_from_graph(graph: &PathIndex) -> Self {
        let node_order = PrimitiveArray::from_iter(
            (0..graph.node_count).map(|i| Some(i as i32)),
        );
        let step_offsets = Arc::new(graph.segment_offsets.clone());

        Self {
            node_order,
            step_offsets,
        }
    }

    pub fn from_path(graph: &PathIndex, path: PathId) -> Self {
        let node_order = PrimitiveArray::from_iter(
            graph.path_steps[path.ix()]
                .iter()
                .map(|i| Some(i.ix() as i32)),
        );

        let step_offsets = Arc::new(graph.path_step_offsets[path.ix()].clone());

        Self {
            node_order,
            step_offsets,
        }
    }

    // pub fn sample_range(&self, range: std::ops::Range<u64>, bins_out: &mut [u8]) {
    pub fn sample_range(
        &self,
        sample_range: std::ops::RangeInclusive<u64>,
        data: &PrimitiveArray<f32>,
        bins_out: &mut [f32],
    ) {
        let left = *sample_range.start();
        let right = *sample_range.end();

        let bin_count = bins_out.len();
        let bin_span = (right - left) as usize / bin_count;

        for (bin_ix, bin_val) in bins_out.iter_mut().enumerate() {
            let bin_start = (bin_span * bin_ix) as u64;
            let bin_end = if bin_ix == bin_count - 1 {
                right
            } else {
                (bin_span * (bin_ix + 1)) as u64
            };

            let bin_range = bin_start..bin_end;

            let bin_first_node_i = self.step_offsets.rank(bin_start);
            // this should never fail (might be an off-by-one at the last node, idk)
            let bin_first =
                self.node_order.get(bin_first_node_i as usize).unwrap();

            let bin_last_node_i = self.step_offsets.rank(bin_end);
            let bin_last =
                self.node_order.get(bin_last_node_i as usize).unwrap();

            // let bin_range = (bin_span * bin_ix)..(bin_span * (bin_ix + 1));
            // let (start, end) = sample_range.clone().into_inner();
            // let len = end - start;

            // let mut bin_size = len / bin_span;

            // let bin start =

            todo!();

            //
        }

        //
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
