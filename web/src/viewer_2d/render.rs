use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use raving_wgpu::{egui, wgpu};
// use egui::mutex::Mutex;

use egui::mutex::RwLock;
use raving_wgpu::node::GraphicsNode;

use wgpu::util::{BufferInitDescriptor, DeviceExt, RenderEncoder};

use crate::color::ColorMap;

pub struct PagedBuffers {
    page_size: u64, // bytes
    stride: u64,    // bytes
    len: usize,

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

        // set the page size to the greatest multiple of `stride` smaller than `max_size`
        let max_size = (max_size / stride) * stride;

        println!("max_size: {max_size}");

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
            len: 0,
        };

        assert!(result.capacity() >= desired_capacity);

        Ok(result)
    }

    pub fn upload_slice<T: bytemuck::Pod>(
        &mut self,
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

        self.len = data.len();

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn page_size(&self) -> u64 {
        self.page_size
    }

    pub fn page_capacity(&self) -> usize {
        (self.page_size / self.stride) as usize
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

    pub fn page_ranges_iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (usize, std::ops::Range<usize>)> + 'a {
        self.pages.iter().enumerate().map(|(page_i, _buf)| {
            let len = self.len();
            let page_cap = self.page_capacity();
            let offset = page_i * page_cap;
            let end = (offset + page_cap).min(len);

            (page_i, offset..end)
        })
    }

    fn get_subpage_range(
        &self,
        index_range: std::ops::Range<usize>,
    ) -> Option<(usize, std::ops::Range<usize>)> {
        let si = index_range.start();
        let ei = index_range.end();

        let sp = si / self.page_capacity();
        let ep = ei / self.page_capacity();

        if sp != ep {
            log::warn!("get_subpage_range crossed page boundary {sp}/{ep}");
            // TODO warn if page boundary has to be crossed (wasm)
        }

        let page = sp;

        // let start_l =
        todo!();

        None
    }
    // pub fn get_subpage_ranges<'a>(
    //     &'a self,
    //     index_range: std::ops::Range<usize>,
    // ) -> Option<impl Iterator<Item = (usize, std::ops::Range<usize>)> + 'a>
    // {
    //     None
    // }

    // pub fn subpage_ranges_iter<'a>(
    //     &'a self,
    // ) -> impl Iterator<Item = (usize, std::ops::Range<usize>)> + 'a {
    //     self.pages.iter().enumerate().map(|(page_i, _buf)| {
    //         let len = self.len();
    //         let page_cap = self.page_capacity();
    //         let offset = page_i * page_cap;
    //         let end = (offset + page_cap).min(len);

    //         (page_i, offset..end)
    //     })
    // }
}

#[derive(Default, Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct DataConfig {
    page_size: u32,
    // max: u32,
}

pub(super) struct State {
    vertex_buffers: PagedBuffers,
    data_buffers: PagedBuffers,

    data_config_uniform: crate::util::Uniform<DataConfig, 4>,

    vertex_config_uniform: wgpu::Buffer,
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
        use web_sys::console;
        console::log_1(&"polyline renderer init".into());
        let shader_src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../app/shaders/path_2d_g_webgl.wgsl"
        ));

        console::log_1(&"alright".into());

        let graphics_node = raving_wgpu::node::graphics_node(
            device,
            shader_src,
            "vs_main",
            "fs_main",
            [],
            // ["u_data"],
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
                ["p0", "p1", "node_id", "node_data"].as_slice(),
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

        console::log_1(&"graphics node done".into());
        let vertex_buffers = PagedBuffers::new(
            device,
            wgpu::BufferUsages::VERTEX,
            std::mem::size_of::<[u32; 5]>() as u64,
            max_segments,
        )?;
        // WebGL2 doesn't support storage buffers
        let data_buffers = PagedBuffers::new(
            device,
            // wgpu::BufferUsages::STORAGE,
            wgpu::BufferUsages::VERTEX,
            std::mem::size_of::<[u32; 1]>() as u64,
            max_segments,
        )?;
        console::log_1(&"paged buffers done".into());

        let transform = ultraviolet::Mat4::identity();

        // let node_width = 80f32;
        let node_width = 400f32;
        let vertex_config_uniform =
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
            "Viewer 2D Color Mapping",
            color_map,
            |cm| {
                let data: [u8; 16] = bytemuck::cast(*cm);
                data
            },
        )?;

        let data_config_uniform = crate::util::Uniform::new(
            device,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "Viewer 2D Data Config",
            DataConfig {
                page_size: vertex_buffers.page_capacity() as u32,
            },
            |cm| {
                let data: [u8; 4] = bytemuck::cast(*cm);
                data
            },
        )?;

        let graphics_node = Arc::new(graphics_node);

        let state = Arc::new(RwLock::new(State {
            vertex_buffers,
            data_buffers,
            vertex_config_uniform,
            projection_uniform,

            data_config_uniform,

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
        let mut state = self.state.write();
        let seg_count = segment_data.len();
        let expected = state.segment_count;

        if seg_count != expected {
            panic!("Node data doesn't match node count: was {seg_count}, expected {expected}");
        }

        state.data_buffers.upload_slice(gpu_state, segment_data)?;

        self.has_node_data = true;

        Ok(())
    }

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
            state.vertex_config_uniform.as_entire_binding(),
        );

        //// fragment shader
        // segment color
        bindings.insert(
            "u_data".into(),
            state.data_buffers.pages[0].as_entire_binding(),
        );

        bindings.insert(
            "u_data_config".into(),
            state.data_config_uniform.buffer().as_entire_binding(),
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

        let size = viewport.size();
        pass.set_viewport(0., 0., size.x, size.y, 0., 1.);

        // "step through" the vertex and data buffers simultaneously
        // using the smaller (in elements) page size

        let vx_ranges =
            state.vertex_buffers.page_ranges_iter().map(|(p_i, range)| {
                let page = &state.vertex_buffers.pages[p_i];
                let page_cap = state.vertex_buffers.page_capacity();
                let s = (range.start % page_cap) as u32;
                let e = ((range.end - 1) % page_cap) as u32;

                let geo_slice = page.slice(..);

                let data_slice = if let Some((data_p, data_range)) =
                    state.data_buffers.get_subpage_range(range)
                {
                    let page = &state.data_buffers[data_p];
                    todo!();
                } else {
                    panic!("draw_in_pass_impl: invalid data range");
                };
                let data_slice = todo!();

                let instances = s..e;

                (geo_slice, data_slice, instances)
            });

        // let data_ranges =
        //     state.vertex_buffers.page_ranges_iter().map(|(p_i, range)| {
        //         let page = &state.vertex_buffers.pages[p_i];
        //         let page_cap = state.vertex_buffers.page_capacity();
        //         let s = (range.start % page_cap) as u32;
        //         let e = ((range.end - 1) % page_cap) as u32;

        //         (page.slice(..), s..e)
        //     });
        // .collect::<Vec<_>>();

        let vx_ranges = geo_ranges.zip(data_ranges);

        for ((geo_buf, data_buf), instances) in vx_ranges {
            pass.set_vertex_buffer(0, geo_buf);
            pass.set_vertex_buffer(1, data_buf);

            let empty_offsets = [];
            let offsets = [0];
            for (i, bind_group) in state.bind_groups.iter().enumerate() {
                // if i == 0 {
                pass.set_bind_group(i as u32, bind_group, &empty_offsets);
                // } else {
                //     pass.set_bind_group(i as u32, bind_group, &offsets);
                // }
            }

            pass.draw(0..6, instances);
        }
    }
}
