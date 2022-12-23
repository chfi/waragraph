use ultraviolet::Vec2;
use waragraph_core::graph::PathIndex;

use anyhow::Result;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use waragraph_core::graph::sampling::{PathDepthData};

use super::BufferDesc;

pub(super) fn path_depth_data_viz_buffer(
    device: &wgpu::Device,
    index: &PathIndex,
    data: &PathDepthData,
    paths: impl IntoIterator<Item = usize>,
    view_range: std::ops::Range<u64>,
    bins: usize,
) -> Result<BufferDesc> {
    let paths = paths.into_iter().collect::<Vec<_>>();
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

pub(super) fn path_viz_buffer_test(
    device: &wgpu::Device,
    bins: usize,
) -> Result<BufferDesc> {
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
