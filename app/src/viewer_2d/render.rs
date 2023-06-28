use std::collections::HashMap;

use anyhow::{Context, Result};
use raving_wgpu::node::GraphicsNode;

use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct PagedBuffers {
    page_size: u64, // bytes
    stride: u64,    // bytes

    pages: Vec<wgpu::Buffer>,
}

impl PagedBuffers {
    pub fn new(
        device: &wgpu::Device,
        mut usage: wgpu::BufferUsages,
        stride: u64,
        desired_capacity: usize, // in elements
    ) -> Result<Self> {
        let max_size = device.limits().max_buffer_size;

        // TODO set the page size to the greatest multiple of `stride` smaller than `max_size`
        let total_size = desired_capacity as u64 * stride;
        let page_size = total_size.min(max_size);
        let page_count =
            (total_size / page_size) + (total_size % page_size).max(1);

        let mut pages = Vec::new();

        usage |= wgpu::BufferUsages::COPY_DST;

        for _ in 0..page_count {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: page_size,
                usage,
                mapped_at_creation: false,
            });

            pages.push(buffer);
        }

        Ok(Self {
            page_size,
            stride,
            pages,
        })
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

        for (page, chunk) in self
            .pages
            .iter()
            .zip(data.chunks(self.page_size() as usize))
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

    pub fn pages(&self) -> &[wgpu::Buffer] {
        &self.pages
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn capacity(&self) -> usize {
        let els_per_page = (self.page_size / self.stride) as usize;
        els_per_page * self.pages.len()
    }

    pub fn total_size(&self) -> u64 {
        self.page_size * self.pages.len() as u64
    }
}

pub struct PolylineRenderer {
    graphics_node: GraphicsNode,

    vertex_buffers: PagedBuffers,
    color_buffers: PagedBuffers,

    vertex_cfg_uniform: wgpu::Buffer,
    projection_uniform: wgpu::Buffer,

    // fragment_uniform: wgpu::Buffer,
    // uniform_buffer: wgpu::Buffer,
    //
    transform: ultraviolet::Mat4,

    bind_groups: Vec<wgpu::BindGroup>,
    segment_count: usize,
    has_data: bool,
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
            8,
            max_segments,
        )?;
        let color_buffers = PagedBuffers::new(
            device,
            wgpu::BufferUsages::STORAGE,
            4,
            max_segments,
        )?;

        let transform = ultraviolet::Mat4::identity();

        let node_width = 20f32;
        let vertex_cfg_uniform =
            device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[node_width, 0f32, 0f32, 0f32]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let projection_uniform =
            device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[transform]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        Ok(Self {
            graphics_node,
            vertex_buffers,
            color_buffers,

            vertex_cfg_uniform,
            projection_uniform,

            transform: transform.into(),

            bind_groups: vec![],
            segment_count: 0,
            has_data: false,
        })
    }

    pub fn has_data(&self) -> bool {
        self.has_data
    }

    pub fn upload_data(
        &mut self,
        state: &raving_wgpu::State,
        segment_positions: &[[f32; 2]],
        segment_colors: &[[f32; 4]],
    ) -> Result<()> {
        let seg_count = segment_positions.len();

        if seg_count != segment_colors.len() {
            anyhow::bail!(
                "PolylineRenderer::upload_data: segment_positions \
                           and segment_colors must have the same length"
            );
        }

        if seg_count > self.vertex_buffers.capacity() {
            panic!("Line data would not fit buffers");
        }

        self.vertex_buffers.upload_slice(state, segment_positions)?;
        self.color_buffers.upload_slice(state, segment_colors)?;

        self.has_data = true;
        self.segment_count = segment_positions.len();

        Ok(())
    }

    pub fn has_bind_groups(&self) -> bool {
        !self.bind_groups.is_empty()
    }

    pub fn create_bind_groups(&mut self, device: &wgpu::Device) -> Result<()> {
        self.bind_groups.clear();

        let mut bindings = HashMap::default();

        // create bind groups for interface

        // projection
        bindings.insert(
            "projection".into(),
            self.projection_uniform.as_entire_binding(),
        );

        // vertex config
        bindings.insert(
            "config".into(),
            self.vertex_cfg_uniform.as_entire_binding(),
        );

        // segment color
        bindings.insert(
            "colors".into(),
            self.color_buffers.pages[0].as_entire_binding(),
        );

        let bind_groups = self
            .graphics_node
            .interface
            .create_bind_groups(device, &bindings)?;

        self.bind_groups = bind_groups;

        Ok(())
    }

    pub fn draw_in_pass<'a, 'b>(&'a self, pass: &'a mut wgpu::RenderPass<'b>)
    where
        'a: 'b,
    {
        if !self.has_data || !self.has_bind_groups() {
            return;
        }

        // iterate through the pages "correctly", setting the vertex
        // buffer & bind groups, and then drawing

        pass.set_vertex_buffer(0, self.vertex_buffers.pages[0].slice(..));

        let offsets = [];
        for (i, bind_group) in self.bind_groups.iter().enumerate() {
            pass.set_bind_group(i as u32, bind_group, &offsets);
        }

        pass.draw(0..6, 0..self.segment_count as u32);
    }
}
