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

#[async_trait]
pub trait Sampler: Send + Sync {
    async fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>>;
}

pub struct PathDataSampler {
    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,
    data_key: Arc<String>,
}

impl PathDataSampler {
    pub fn new(
        path_index: Arc<PathIndex>,
        data_cache: Arc<GraphDataCache>,
        data_key: &str,
    ) -> Self {
        Self {
            path_index,
            data_cache,
            data_key: Arc::new(data_key.to_string()),
        }
    }
}

#[async_trait]
impl Sampler for PathDataSampler {
    async fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>> {
        let data = self
            .data_cache
            .fetch_path_data(&self.data_key, path)
            .await?;

        let path_index = self.path_index.clone();

        let sample_vec = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 4 * bin_count];

            let l = view.start.0;
            let r = view.end.0;
            let view_len = (r - l) as usize;
            let used_bins = view_len.min(bin_count);
            let used_slice = &mut buf[..used_bins * 4];

            waragraph_core::graph::sampling::sample_data_into_buffer(
                &path_index,
                path,
                &data.path_data,
                l..r,
                bytemuck::cast_slice_mut(used_slice),
            );

            buf
        })
        .await?;

        Ok(sample_vec)
    }
}

pub struct PathNodeSetSampler {
    path_index: Arc<PathIndex>,
}

impl PathNodeSetSampler {
    pub fn new(
        path_index: Arc<PathIndex>,
        // later add function to post-compose with
    ) -> Self {
        Self {
            path_index,
            // data_cache,
        }
    }
}

#[async_trait]
impl Sampler for PathNodeSetSampler {
    async fn sample_range(
        &self,
        bin_count: usize,
        path: PathId,
        view: std::ops::Range<Bp>,
    ) -> Result<Vec<u8>> {
        let path_index = self.path_index.clone();

        let sample_vec = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 4 * bin_count];
            let l = view.start.0;
            let r = view.end.0;
            let view_len = (r - l) as usize;
            let used_bins = view_len.min(bin_count);
            let used_slice = &mut buf[..used_bins * 4];

            // TODO actually do stuff

            let bins: &mut [u32] = bytemuck::cast_slice_mut(used_slice);

            let path_nodes = &path_index.path_node_sets[path.ix()];

            for (bin_ix, buf_val) in bins.into_iter().enumerate() {
                // pangenome space
                let range = bin_range(bin_count, &view, bin_ix);

                // get range of nodes corresponding to the pangenome `range`
                let (start, end) =
                    path_index.pos_range_nodes(range).into_inner();
                // let start = node_range.start.0 as u32;
                // let end = node_range.end.0 as u32;
                let ix_range = (start.ix() as u32)..(end.ix() as u32 + 1);

                if path_nodes.range_cardinality(ix_range) > 0 {
                    *buf_val = 1;
                }
            }

            buf
        })
        .await?;

        Ok(sample_vec)
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
