use std::sync::Arc;

use async_trait::async_trait;

use anyhow::Result;

use waragraph_core::graph::{Bp, PathId, PathIndex};

use crate::app::resource::GraphDataCache;

// pub trait Sampler {
//     fn sample_range_into_bins(&self,
//                               bin_count: usize,
//                               path: PathId,
//                               view: std::ops::Range<Bp>,
//                               ) -> Vec<

// }

// pub struct SamplerMean {
//
// }

pub struct ArrowSampler {
    //
}

// #[async_trait]
pub trait Sampler: Send + Sync {
    fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>>;
}

pub struct PathDataSampler<'data> {
    path_index: Arc<PathIndex>,
    data: &'data [f32],
}

impl<'data> PathDataSampler<'data> {
    pub fn new(path_index: Arc<PathIndex>, data: &'data [f32]) -> Self {
        Self { path_index, data }
    }
}

#[async_trait]
impl<'data> Sampler for PathDataSampler<'data> {
    fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>> {
        let path_index = self.path_index.clone();

        let mut buf = vec![0u8; 4 * bin_count];

        let l = view.start.0;
        let r = view.end.0;
        let view_len = (r - l) as usize;
        let used_bins = view_len.min(bin_count);
        let used_slice = &mut buf[..used_bins * 4];

        waragraph_core::graph::sampling::sample_data_into_buffer(
            &path_index,
            path,
            &self.data,
            l..r,
            bytemuck::cast_slice_mut(used_slice),
        );

        Ok(buf)
    }
}

pub struct PathNodeSetSampler {
    path_index: Arc<PathIndex>,
    map: Arc<dyn Fn(PathId, u32) -> f32 + Send + Sync + 'static>,
}

impl PathNodeSetSampler {
    pub fn new(
        path_index: Arc<PathIndex>,
        map: impl Fn(PathId, u32) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self {
            path_index,
            map: Arc::new(map),
        }
    }
}

impl Sampler for PathNodeSetSampler {
    fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>> {
        let path_index = self.path_index.clone();
        let map = self.map.clone();

        let mut buf = vec![0u8; 4 * bin_count];
        let l = view.start.0;
        let r = view.end.0;
        let view_len = (r - l) as usize;
        let used_bins = view_len.min(bin_count);
        let used_slice = &mut buf[..used_bins * 4];

        let bins: &mut [f32] = bytemuck::cast_slice_mut(used_slice);

        let path_nodes = &path_index.path_node_sets[path.ix()];

        for (bin_ix, buf_val) in bins.into_iter().enumerate() {
            // pangenome space
            let range = bin_range(bin_count, &view, bin_ix);

            // get range of nodes corresponding to the pangenome `range`
            let (start, end) = path_index.pos_range_nodes(range).into_inner();
            let ix_range = (start.ix() as u32)..(end.ix() as u32 + 1);

            if path_nodes.range_cardinality(ix_range) > 0 {
                *buf_val = map(path, 1);
            } else {
                *buf_val = std::f32::NEG_INFINITY;
            }
        }

        Ok(buf)
    }
}

fn bin_range(
    bin_count: usize,
    view_range: &std::ops::Range<Bp>,
    bin_ix: usize,
) -> std::ops::Range<u64> {
    let s = view_range.start.0;
    let e = view_range.end.0;
    let len = e - s;

    let bin_size = len / bin_count as u64;

    let start = s + bin_size * bin_ix as u64;
    let end = start + bin_size;
    start..end
}
