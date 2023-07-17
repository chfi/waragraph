use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
// use egui::mutex::Mutex;

use egui::mutex::RwLock;
use raving_wgpu::node::GraphicsNode;

use wgpu::util::{BufferInitDescriptor, DeviceExt, RenderEncoder};

use crate::color::ColorMap;

pub struct PagedBuffers {
    page_size: u64, // bytes
    stride: u64,    // bytes

    pages: Vec<Arc<wgpu::Buffer>>,
}

impl PagedBuffers {
    pub fn new(
        device: &wgpu::Device,
        mut usage: wgpu::BufferUsages,
        stride: u64,
        desired_capacity: usize, // in elements
    ) -> Result<Self> {
        let max_size = device.limits().max_buffer_size;
        // let max_size = 8 * (1 << 20);
        // let max_size = 12 * (1 << 20);
        // let max_size = 4 * (1 << 20);
        println!("max_size: {max_size}");

        // TODO set the page size to the greatest multiple of `stride` smaller than `max_size`

        println!("desired_capacity: {desired_capacity}");
        println!("stride: {stride}");
        let total_size = desired_capacity as u64 * stride;
        let page_size = total_size.min(max_size);
        let page_count =
            (total_size / page_size) + (total_size % page_size).min(1);

        println!("total_size: {total_size}");
        println!("page_size: {page_size}");
        println!("page_count: {page_count}");

        let mut pages = Vec::new();

        usage |= wgpu::BufferUsages::COPY_DST;

        for _ in 0..page_count {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: page_size,
                usage,
                mapped_at_creation: false,
            });

            pages.push(Arc::new(buffer));
        }

        let result = Self {
            page_size,
            stride,
            pages,
        };

        assert!(result.capacity() >= desired_capacity);

        Ok(result)
    }

    pub fn upload_slice<T: bytemuck::Pod>(
        &self,
        state: &raving_wgpu::State,
        data: &[T],
    ) -> Result<()> {
        let el_size = std::mem::size_of::<T>();

        if el_size != self.stride as usize {
            anyhow::bail!("PagedBuffers upload error: data stride {} did not match expected stride {}",
                          el_size,
                          self.stride);
        }

        if data.len() > self.capacity() {
            anyhow::bail!("PagedBuffers upload error: data would not fit in buffer ({} > {} elements)",
                          data.len(),
                          self.capacity());
        }

        let data_bytes: &[u8] = bytemuck::cast_slice(data);

        for (page, chunk) in self
            .pages
            .iter()
            .zip(data_bytes.chunks(self.page_size() as usize))
        {
            state
                .queue
                .write_buffer(page, 0, bytemuck::cast_slice(chunk));
        }

        Ok(())
    }

    pub fn page_size(&self) -> u64 {
        self.page_size
    }

    pub fn stride(&self) -> u64 {
        self.stride
    }

    pub fn pages(&self) -> &[Arc<wgpu::Buffer>] {
        &self.pages
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn capacity(&self) -> usize {
        // let els_per_page = (self.page_size / self.stride) as usize;
        (self.page_size as usize * self.pages.len()) / self.stride as usize
    }

    pub fn total_size(&self) -> u64 {
        self.page_size * self.pages.len() as u64
    }
}

pub(super) struct State {
    vertex_buffers: PagedBuffers,
    data_buffers: PagedBuffers,

    vertex_cfg_uniform: wgpu::Buffer,
    projection_uniform: wgpu::Buffer,

    color_map: crate::util::Uniform<ColorMap, 16>,

    bind_groups: Vec<wgpu::BindGroup>,
    segment_count: usize,

    color_sampler_ids:
        Option<(wgpu::Id<wgpu::TextureView>, wgpu::Id<wgpu::Sampler>)>,

    graphics_node: Arc<GraphicsNode>,
}

pub struct PolylineRenderer {
    pub(super) graphics_node: Arc<GraphicsNode>,

    pub(super) state: Arc<RwLock<State>>,

    // fragment_uniform: wgpu::Buffer,
    // uniform_buffer: wgpu::Buffer,
    //
    transform: ultraviolet::Mat4,

    has_position_data: bool,
    has_node_data: bool,
}

impl PolylineRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        max_segments: usize,
    ) -> Result<Self> {
        let shader_src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/shaders/path_2d_g.wgsl"
        ));

        let graphics_node = raving_wgpu::node::graphics_node(
            device,
            shader_src,
            "vs_main",
            "fs_main",
            wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None, // TODO fix
                // cull_mode: Some(wgpu::Face::Front),
                polygon_mode: wgpu::PolygonMode::Fill,

                strip_index_format: None,
                unclipped_depth: false,
                conservative: false,
            },
            None,
            wgpu::MultisampleState::default(),
            [(
                ["p0", "p1", "node_id"].as_slice(),
                wgpu::VertexStepMode::Instance,
            )],
            [
                (
                    "color",
                    wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::all(),
                    },
                ),
                // (
                //     "node_id",
                //     wgpu::ColorTargetState {
                //         format: wgpu::TextureFormat::R32Uint,
                //         blend: None,
                //         write_mask: wgpu::ColorWrites::all(),
                //     },
                // ),
                // (
                //     "uv",
                //     wgpu::ColorTargetState {
                //         format: wgpu::TextureFormat::Rg32Float,
                //         blend: None,
                //         write_mask: wgpu::ColorWrites::all(),
                //     },
                // ),
            ],
        )?;

        let vertex_buffers = PagedBuffers::new(
            device,
            wgpu::BufferUsages::VERTEX,
            std::mem::size_of::<[u32; 5]>() as u64,
            max_segments,
        )?;
        let data_buffers = PagedBuffers::new(
            device,
            wgpu::BufferUsages::STORAGE,
            std::mem::size_of::<[u32; 1]>() as u64,
            max_segments,
        )?;

        let transform = ultraviolet::Mat4::identity();

        let node_width = 80f32;
        let vertex_cfg_uniform =
            device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[node_width, 0f32, 0f32, 0f32]),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST,
            });

        let projection_uniform =
            device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[transform]),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST,
            });

        let color_map = ColorMap {
            value_range: [0.0, 13.0],
            color_range: [0.0, 1.0],
        };

        let color_map = crate::util::Uniform::new(
            device,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "Viewer 1D Color Mapping",
            color_map,
            |cm| {
                let data: [u8; 16] = bytemuck::cast(*cm);
                data
            },
        )?;

        let graphics_node = Arc::new(graphics_node);

        let state = Arc::new(RwLock::new(State {
            vertex_buffers,
            data_buffers,
            vertex_cfg_uniform,
            projection_uniform,

            color_map,

            segment_count: 0,
            bind_groups: vec![],

            color_sampler_ids: None,

            graphics_node: graphics_node.clone(),
        }));
        dbg!();

        Ok(Self {
            graphics_node,

            state,

            transform: transform.into(),

            has_position_data: false,
            has_node_data: false,
        })
    }

    pub fn has_data(&self) -> bool {
        self.has_position_data && self.has_node_data
    }

    pub fn set_transform(
        &mut self,
        queue: &wgpu::Queue,
        transform: ultraviolet::Mat4,
    ) {
        let state = self.state.read();
        self.transform = transform;
        queue.write_buffer(
            &state.projection_uniform,
            0,
            &bytemuck::cast_slice(&[transform]),
        );
    }
    // pub fn set_transform(&mut self,
    //                      queue: &wgpu::Queue) {

    pub fn upload_vertex_data(
        &mut self,
        // state: &mut State,
        gpu_state: &raving_wgpu::State,
        segment_positions: &[[f32; 5]], // actually ([f32; 2], [f32; 2], u32)
    ) -> Result<()> {
        let seg_count = segment_positions.len();

        let mut state = self.state.write();
        println!("segment_positions.len(): {seg_count}");
        println!("vx buf capacity: {}", state.vertex_buffers.capacity());

        if seg_count > state.vertex_buffers.capacity() {
            panic!("Line data would not fit buffers");
        }

        state
            .vertex_buffers
            .upload_slice(gpu_state, segment_positions)?;
        state.segment_count = segment_positions.len();

        self.has_position_data = true;

        Ok(())
    }

    pub fn upload_node_data(
        &mut self,
        // state: &mut State,
        gpu_state: &raving_wgpu::State,
        segment_data: &[f32],
    ) -> Result<()> {
        let state = self.state.write();
        let seg_count = segment_data.len();
        let expected = state.segment_count;

        if seg_count != expected {
            panic!("Node data doesn't match node count: was {seg_count}, expected {expected}");
        }

        state.data_buffers.upload_slice(gpu_state, segment_data)?;

        self.has_node_data = true;

        Ok(())
    }

    // pub fn has_bind_groups(&self) -> bool {
    //     !self.bind_groups.is_empty()
    // }

    // pub fn set_transform(&mut self

    pub fn create_bind_groups(
        &mut self,
        device: &wgpu::Device,
        sampler: &wgpu::Sampler,
        color: &wgpu::TextureView,
    ) -> Result<()> {
        let color_id = color.global_id();
        let sampler_id = sampler.global_id();

        let mut state = self.state.write();

        // the sampler and color are the only bindings not owned by
        // PolylineRenderer, and none of the resources owned by the
        // renderer have to be reallocated, so we only need to
        // recreate the bind groups if those differ
        if let Some((c_id, s_id)) = state.color_sampler_ids {
            if c_id == color_id && s_id == sampler_id {
                return Ok(());
            }
        }

        state.bind_groups.clear();

        // Option<(Id<wgpu::TextureView>, Id<wgpu::Sampler>)>,

        let mut bindings = HashMap::default();

        // create bind groups for interface

        //// vertex shader
        // projection
        bindings.insert(
            "projection".into(),
            state.projection_uniform.as_entire_binding(),
        );

        // vertex config
        bindings.insert(
            "config".into(),
            state.vertex_cfg_uniform.as_entire_binding(),
        );

        //// fragment shader
        // segment color
        bindings.insert(
            "u_data".into(),
            state.data_buffers.pages[0].as_entire_binding(),
        );

        bindings.insert(
            "u_color_map".into(),
            state.color_map.buffer().as_entire_binding(),
        );

        bindings.insert(
            "t_sampler".into(),
            wgpu::BindingResource::Sampler(sampler),
        );

        bindings.insert(
            "t_colors".into(),
            wgpu::BindingResource::TextureView(color),
        );

        let bind_groups = self
            .graphics_node
            .interface
            .create_bind_groups(device, &bindings)?;

        state.bind_groups = bind_groups;

        state.color_sampler_ids = Some((color_id, sampler_id));

        Ok(())
    }

    pub(super) fn draw_in_pass_impl<'a: 'b, 'b>(
        // &'a self,
        state: &'a State,
        pass: &mut wgpu::RenderPass<'b>,
        viewport: egui::Rect,
    ) {
        // iterate through the pages "correctly", setting the vertex
        // buffer & bind groups, and then drawing

        pass.set_pipeline(&state.graphics_node.pipeline);

        pass.set_vertex_buffer(0, state.vertex_buffers.pages[0].slice(..));

        let offsets = [];
        for (i, bind_group) in state.bind_groups.iter().enumerate() {
            pass.set_bind_group(i as u32, bind_group, &offsets);
        }

        let min = viewport.min;
        let size = viewport.size();

        pass.set_viewport(min.x, min.y, size.x, size.y, 0., 1.);

        pass.draw(0..6, 0..state.segment_count as u32);
    }
}
