use ultraviolet::Vec2;
use waragraph_core::graph::PathIndex;

use anyhow::Result;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use super::{
    sampling::{PathDepthData, PathPangenomeRangeData},
    BufferDesc,
};

pub(super) fn path_depth_data_viz_buffer(
    device: &wgpu::Device,
    index: &PathIndex,
    data: &PathDepthData,
    paths: impl IntoIterator<Item = usize>,
    view_range: std::ops::Range<u64>,
    bins: usize,
) -> Result<BufferDesc> {
    let paths = paths.into_iter().collect::<Vec<_>>();

    let row_size = bins;
    let total_size = row_size * paths.len();

    let mut buf_data: Vec<u8> = Vec::new();

    buf_data.extend(bytemuck::cast_slice(&[
        total_size as u32,
        row_size as u32,
        0,
        0,
    ]));

    let mut bin_buf: Vec<f32> = Vec::with_capacity(bins);

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

    for path in paths {
        bin_buf.clear();

        for bin_ix in 0..bins {
            let range = bin_range(bin_ix);
            let val = data.get(index, path, range).unwrap_or(0.0);
            bin_buf.push(val);
        }

        buf_data.extend(bytemuck::cast_slice(&bin_buf));
    }

    let usage = wgpu::BufferUsages::STORAGE;

    let buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: buf_data.as_slice(),
        usage,
    });

    Ok(BufferDesc::new(buffer, buf_data.len()))
}

pub(super) fn path_slot_vertex_buffer(
    device: &wgpu::Device,
    paths: impl IntoIterator<Item = usize>,
) -> Result<BufferDesc> {
    let g_offset = Vec2::new(50.0, 50.0);
    let g_del = Vec2::new(0.0, 30.0);
    let g_size = Vec2::new(700.0, 20.0);

    let data = paths
        .into_iter()
        .enumerate()
        .flat_map(|(ix, _path)| {
            let mut vx = [0u8; 4 * 5];
            let pos = g_offset + g_del * ix as f32;
            vx[0..16].clone_from_slice(bytemuck::cast_slice(&[pos, g_size]));
            vx[16..].clone_from_slice(bytemuck::cast_slice(&[ix as u32]));
            vx
        })
        .collect::<Vec<_>>();

    let usage = wgpu::BufferUsages::VERTEX;

    let buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: data.as_slice(),
        usage,
    });

    Ok(BufferDesc::new(buffer, data.len()))
}

/*
pub fn path_depth_viz_buffers(
    device: &wgpu::Device,
    index: &PathIndex,
    paths: impl IntoIterator<Item = usize>,
    view_range: std::ops::Range<u64>,
    bins: usize,
) -> Result<(wgpu::Buffer, usize)> {

    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::
}
*/