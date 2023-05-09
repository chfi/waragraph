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

pub struct DataSampler {
    // sample_range: Arc<
    //     dyn Fn(PathId, std::ops::Range<Bp>, &mut [u8]) + Send + Sync + 'static,
    // >,
}

pub fn path_data_sampler_mean(
    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,
    data_key: &str,
    // path: PathId,
    // view: std::ops::Range<Bp>,
) -> () {
    todo!();
}
