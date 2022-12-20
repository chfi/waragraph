use ultraviolet::Vec2;
use waragraph_core::graph::PathIndex;

use anyhow::Result;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use waragraph_core::graph::sampling::{PathDepthData, PathPangenomeRangeData};

use super::BufferDesc;

pub(super) fn path_viz_buffer_test(
    device: &wgpu::Device,
    bins: usize,
) -> Result<BufferDesc> {
    let mut rows_data: Vec<Vec<u8>> = Vec::new();

    // fn mk_row(f: impl Fn(usize) -> f32) -> Vec<f32> {
    //     let row_size = 100;
    //     (0..row_size).map(f).collect::<Vec<_>>()
    // }

    let row_size = 100;

    let mk_row = |f: fn(usize) -> f32| (0..row_size).map(f).collect::<Vec<_>>();

    let row_size = 100;

    let row0 = mk_row(|i| (i / 10) as f32);
    let mut row1 = row0.clone();
    row1.reverse();
    let row2 = mk_row(|i| if i < 100 / 2 { 4.0 } else { 8.0 });


    let total_size = 3 * row_size;

    let mut buf_data: Vec<u8> = Vec::with_capacity(total_size + 4 * 4);
    
    buf_data.extend(bytemuck::cast_slice(&[
        total_size as u32,
        row_size as u32,
        0,
        0,
    ]));

    buf_data.extend_from_slice(bytemuck::cast_slice(&row0));
    buf_data.extend_from_slice(bytemuck::cast_slice(&row1));
    buf_data.extend_from_slice(bytemuck::cast_slice(&row2));
    
    let usage = wgpu::BufferUsages::STORAGE;

    let buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: buf_data.as_slice(),
        usage,
    });

    Ok(BufferDesc::new(buffer, buf_data.len()))
}

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
            let data = &data.node_depth_per_path[path];
            let iter = index.path_data_pan_range_iter(range, path, data);

            let mut sum_len = 0;
            let mut sum_val = 0.0;

            for ((_node, len), val) in iter {
                sum_len += len.0;
                sum_val += *val * len.0 as f32;
            }

            bin_buf.push(sum_val / sum_len as f32);
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
