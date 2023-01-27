use ultraviolet::Vec2;
use waragraph_core::graph::{PathId, PathIndex};

use anyhow::Result;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use waragraph_core::graph::sampling::PathDepthData;

use crate::app::resource::{FStats, GraphPathData};

use super::BufferDesc;

pub(super) fn path_sampled_data_viz_buffer<S>(
    device: &wgpu::Device,
    index: &PathIndex,
    data: &GraphPathData<f32, S>,
    paths: impl IntoIterator<Item = PathId>,
    view_range: std::ops::Range<u64>,
    bins: usize,
) -> Result<BufferDesc> {
    let paths = paths.into_iter().collect::<Vec<_>>();
    println!("creating buffer for {} paths", paths.len());
    let prefix_size = std::mem::size_of::<u32>() * 4;
    let elem_size = std::mem::size_of::<f32>();
    let max_size = prefix_size + elem_size * bins * paths.len();

    let mut buf = vec![0u8; max_size];

    waragraph_core::graph::sampling::sample_path_data_into_buffer(
        index, data, paths, bins, view_range, &mut buf,
    );

    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

    let buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: buf.as_slice(),
        usage,
    });

    Ok(BufferDesc::new(buffer, buf.len()))
}
