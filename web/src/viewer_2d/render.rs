use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use raving_wgpu::{egui, wgpu};
// use egui::mutex::Mutex;

use egui::mutex::RwLock;
use raving_wgpu::node::GraphicsNode;

use wgpu::util::{BufferInitDescriptor, DeviceExt, RenderEncoder};

use wasm_bindgen::prelude::*;

use crate::color::ColorMap;

#[wasm_bindgen]
#[derive(Clone)]
pub struct PagedBuffers {
    page_size: u64, // bytes
    stride: u64,    // bytes
    len: usize,

    pages: Arc<Vec<wgpu::Buffer>>,
}

impl PagedBuffers {
    pub fn new(
        device: &wgpu::Device,
        mut usage: wgpu::BufferUsages,
        stride: u64,
        desired_capacity: usize, // in elements
    ) -> Result<Self> {
        log::warn!("{:#?}", device.limits());

        let max_size = device.limits().max_buffer_size;

        // set the page size to the greatest multiple of `stride` smaller than `max_size`
        let max_size = (max_size / stride) * stride;

        log::info!("max_size: {max_size}");

        log::info!("desired_capacity: {desired_capacity}");
        log::info!("stride: {stride}");
        let total_size = desired_capacity as u64 * stride;
        let page_size = total_size.min(max_size);
        let page_count =
            (total_size / page_size) + (total_size % page_size).min(1);

        log::info!("total_size: {total_size}");
        log::info!("page_size: {page_size}");
        log::info!("page_count: {page_count}");

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

        let pages = Arc::new(pages);

        let result = Self {
            page_size,
            stride,
            pages,
            len: 0,
        };

        assert!(result.capacity() >= desired_capacity);

        Ok(result)
    }

    pub fn upload_slice_to_page(
        &mut self,
        state: &raving_wgpu::State,
        page_index: usize,
        data: &[u8],
    ) -> Result<()> {
        if data.len() > self.page_size() as usize {
            anyhow::bail!(
                "Attempted to upload {} bytes to a page of size {}",
                data.len(),
                self.page_size()
            );
        }

        if page_index >= self.pages.len() {
            anyhow::bail!("Page out of bounds");
        }

        let page = &self.pages[page_index];

        state.queue.write_buffer(page, 0, data);

        Ok(())
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

        self.set_len(data.len());

        Ok(())
    }

    pub fn pages(&self) -> &Arc<Vec<wgpu::Buffer>> {
        &self.pages
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

    /// panics if `self.len()` != `other.len()`, or if
    /// `self.page_capacity()` is greater than `other.page_capacity()`
    /// (i.e. if `self` has a shorter stride than `other`)
    fn zip_as_vertex_buffer_slices<'a, 'b>(
        &'a self,
        other: &'b Self,
    ) -> impl Iterator<
        Item = (
            wgpu::BufferSlice<'a>,
            wgpu::BufferSlice<'b>,
            std::ops::Range<u32>,
        ),
    > {
        if self.len != other.len {
            panic!("zip_as_vertex_buffer_slices was called with `self.len` != `other.len`");
        }

        if self.stride < other.stride {
            panic!("zip_as_vertex_buffer_slices was called with `self.stride` < `other.stride`");
        }

        self.page_ranges_iter().filter_map(|(p_i, range)| {
            let page = &self.pages[p_i];
            let cap = self.page_capacity();
            let s = (range.start % cap) as u32;
            let e = ((range.end - 1) % cap) as u32;
            // let e = (range.end % cap) as u32;

            let instances = s..e;

            let geo_slice = page.slice(..);

            let (page_i, page_range) = other.get_subpage_range(range);
            let data_page = &other.pages[page_i];
            let data_s = page_range.start as u32;
            let data_e = page_range.end as u32;
            let data_s = (data_s as u64) * other.stride;
            let data_e = (data_e as u64) * other.stride;

            if data_s == data_e {
                return None;
            }
            let data_slice = data_page.slice(data_s..data_e);

            Some((geo_slice, data_slice, instances))
        })
    }

    fn get_subpage_range(
        &self,
        index_range: std::ops::Range<usize>,
    ) -> (usize, std::ops::Range<usize>) {
        let si = index_range.start;
        let ei = index_range.end;

        let sp = si / self.page_capacity();
        let ep = ei / self.page_capacity();

        if ep.abs_diff(sp) > 1 {
            log::warn!("get_subpage_range crossed page boundary {sp}/{ep}, page size {}", self.page_size());
        }

        let page = sp;

        let page_start = page * self.page_capacity();

        let s = si - page_start;
        let e = ei - page_start;

        (page, s..e)
    }
}

#[wasm_bindgen]
impl PagedBuffers {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
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

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = upload_page)]
    pub fn upload_page_web(
        &mut self,
        raving: &crate::RavingCtx,
        page_index: usize,
        data: &[u8],
    ) -> Result<(), JsValue> {
        self.upload_slice_to_page(&raving.gpu_state, page_index, data)
            .map_err(|err| {
                JsValue::from(format!(
                    "Error uploading to paged buffers: {err:?}"
                ))
            })
    }

    // pub fn upload_pages(&mut self, pages: js_sys::Array) {
    // }

    // pub fn upload_page(&mut self, page_index: usize, page_data: js_sys::Uint8Array) {
    // pub fn upload_page(&mut self, page_index: usize, page_data: &[u8]) {

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
    data_page_uniform: DataPageUniform,

    vertex_config_uniform: wgpu::Buffer,
    projection_uniform: wgpu::Buffer,

    color_map: crate::util::Uniform<ColorMap, 16>,

    bind_groups: Vec<wgpu::BindGroup>,
    segment_count: usize,

    graphics_node: Arc<GraphicsNode>,
}

// per-page uniforms corresponding to PagedBuffers,
// using dynamic offsets
struct DataPageUniform {
    buffer_offset_alignment: u32,

    buffer: wgpu::Buffer,
}

impl DataPageUniform {
    fn new(device: &wgpu::Device, page_count: usize) -> Self {
        let limits = device.limits();
        let buffer_offset_alignment =
            limits.min_uniform_buffer_offset_alignment;

        let el_size = buffer_offset_alignment as usize;

        let mut buffer_data = vec![0u8; el_size * page_count];

        for (i, page_uniform) in buffer_data.chunks_mut(el_size).enumerate() {
            let uni: &mut [u32] = bytemuck::cast_slice_mut(page_uniform);
            uni[0] = i as u32;
        }

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("DataPageUniform buffer"),
            contents: buffer_data.as_slice(),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        Self {
            buffer_offset_alignment,
            buffer,
        }
    }
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
    pub fn new_with_buffers(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        position_buffers: PagedBuffers,
        color_buffers: PagedBuffers,
        // max_segments: usize,
    ) -> Result<Self> {
        let limits = device.limits();

        let shader_src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/shaders/path_2d_direct_color.wgsl" // "/../app/shaders/path_2d_g_webgl.wgsl"
        ));

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
            [
                (
                    ["p0", "p1", "node_id"].as_slice(),
                    wgpu::VertexStepMode::Instance,
                ),
                (["node_color"].as_slice(), wgpu::VertexStepMode::Instance),
            ],
            [
                (
                    "color",
                    wgpu::ColorTargetState {
                        format: surface_format,
                        // NB: webgl doesn't support independent blend ops per attch
                        // and R32Uint isn't blendable at all
                        blend: None,
                        write_mask: wgpu::ColorWrites::all(),
                    },
                ),
                (
                    "node_id",
                    wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R32Uint,
                        blend: None,
                        write_mask: wgpu::ColorWrites::all(),
                    },
                ),
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

        /*
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
        */

        let data_page_uniform =
            DataPageUniform::new(device, color_buffers.page_count());

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
                page_size: position_buffers.page_capacity() as u32,
            },
            |cm| {
                let data: [u8; 4] = bytemuck::cast(*cm);
                data
            },
        )?;

        let graphics_node = Arc::new(graphics_node);

        let state = Arc::new(RwLock::new(State {
            vertex_buffers: position_buffers,
            data_buffers: color_buffers,
            vertex_config_uniform,
            projection_uniform,

            data_config_uniform,
            data_page_uniform,

            color_map,

            segment_count: 0,
            bind_groups: vec![],

            graphics_node: graphics_node.clone(),
        }));

        Ok(Self {
            graphics_node,

            state,

            transform: transform.into(),

            has_position_data: false,
            has_node_data: false,
        })
    }

    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        max_segments: usize,
    ) -> Result<Self> {
        let position_buffers = PagedBuffers::new(
            device,
            wgpu::BufferUsages::VERTEX,
            std::mem::size_of::<[u32; 5]>() as u64,
            max_segments,
        )?;
        let color_buffers = PagedBuffers::new(
            device,
            // wgpu::BufferUsages::STORAGE,
            wgpu::BufferUsages::VERTEX,
            std::mem::size_of::<[u32; 1]>() as u64,
            max_segments,
        )?;

        Self::new_with_buffers(
            device,
            surface_format,
            position_buffers,
            color_buffers,
        )
    }

    pub fn has_data(&self) -> bool {
        self.has_position_data && self.has_node_data
    }

    pub fn update_uniforms(
        &mut self,
        queue: &wgpu::Queue,
        transform: ultraviolet::Mat4,
        window_dims: [f32; 2],
        node_width_px: f32,
    ) {
        let state = self.state.read();
        self.transform = transform;
        queue.write_buffer(
            &state.projection_uniform,
            0,
            &bytemuck::cast_slice(&[transform]),
        );

        let nw = node_width_px / window_dims[0];

        // let nw = 0.1;

        // let data: [f32; 4] = [nw, 0.0, 0.0, 0.0];
        let data: [f32; 4] = [nw; 4];
        queue.write_buffer(
            &state.vertex_config_uniform,
            0,
            bytemuck::cast_slice(&[data]),
        );
    }

    // pub fn set_transform(
    //     &mut self,
    //     queue: &wgpu::Queue,
    //     transform: ultraviolet::Mat4,
    // ) {
    //     let state = self.state.read();
    //     self.transform = transform;
    //     queue.write_buffer(
    //         &state.projection_uniform,
    //         0,
    //         &bytemuck::cast_slice(&[transform]),
    //     );
    // }
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

    pub fn upload_segment_colors(
        &mut self,
        // state: &mut State,
        gpu_state: &raving_wgpu::State,
        segment_colors: &[u32],
    ) -> Result<()> {
        let mut state = self.state.write();
        let seg_count = segment_colors.len();
        let expected = state.segment_count;

        if seg_count != expected {
            panic!("Node data doesn't match node count: was {seg_count}, expected {expected}");
        }

        state.data_buffers.upload_slice(gpu_state, segment_colors)?;

        self.has_node_data = true;

        Ok(())
    }

    pub fn create_bind_groups(&mut self, device: &wgpu::Device) -> Result<()> {
        let mut state = self.state.write();

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

        // bindings.insert(
        //     "u_data_config".into(),
        //     state.data_config_uniform.buffer().as_entire_binding(),
        // );

        // bindings.insert(
        //     "u_color_map".into(),
        //     state.color_map.buffer().as_entire_binding(),
        // );

        // bindings.insert(
        //     "t_sampler".into(),
        //     wgpu::BindingResource::Sampler(sampler),
        // );

        // bindings.insert(
        //     "t_colors".into(),
        //     wgpu::BindingResource::TextureView(color),
        // );

        let bind_groups = self
            .graphics_node
            .interface
            .create_bind_groups(device, &bindings)?;

        state.bind_groups = bind_groups;

        Ok(())
    }

    pub(super) fn draw_in_pass_with_buffers_indexed<'a: 'b, 'b, 'c: 'b>(
        state: &'a State,
        pass: &mut wgpu::RenderPass<'b>,
        viewport: egui::Rect,
        vertex_buffers: &'c PagedBuffers,
        data_buffers: &'c PagedBuffers,
        index_buffers: &'c PagedBuffers,
    ) {
        todo!();
    }

    pub(super) fn draw_in_pass_with_buffers<'a: 'b, 'b, 'c: 'b>(
        state: &'a State,
        pass: &mut wgpu::RenderPass<'b>,
        viewport: egui::Rect,
        vertex_buffers: &'c PagedBuffers,
        data_buffers: &'c PagedBuffers,
    ) {
        pass.set_pipeline(&state.graphics_node.pipeline);

        let size = viewport.size();
        pass.set_viewport(0., 0., size.x, size.y, 0., 1.);

        // "step through" the vertex and data buffers simultaneously
        // using the smaller (in elements) page size

        let vx_ranges =
            vertex_buffers.zip_as_vertex_buffer_slices(data_buffers);

        for (geo_buf, data_buf, instances) in vx_ranges {
            pass.set_vertex_buffer(0, geo_buf);
            pass.set_vertex_buffer(1, data_buf);

            let empty_offsets = [];
            for (i, bind_group) in state.bind_groups.iter().enumerate() {
                pass.set_bind_group(i as u32, bind_group, &empty_offsets);
            }

            pass.draw(0..6, instances);
        }
    }

    pub(super) fn draw_in_pass_impl<'a: 'b, 'b>(
        // &'a self,
        state: &'a State,
        pass: &mut wgpu::RenderPass<'b>,
        viewport: egui::Rect,
    ) {
        use web_sys::console;
        // iterate through the pages "correctly", setting the vertex
        // buffer & bind groups, and then drawing

        pass.set_pipeline(&state.graphics_node.pipeline);

        let size = viewport.size();
        pass.set_viewport(0., 0., size.x, size.y, 0., 1.);

        // "step through" the vertex and data buffers simultaneously
        // using the smaller (in elements) page size

        let vx_ranges = state
            .vertex_buffers
            .zip_as_vertex_buffer_slices(&state.data_buffers);

        for (geo_buf, data_buf, instances) in vx_ranges {
            pass.set_vertex_buffer(0, geo_buf);
            pass.set_vertex_buffer(1, data_buf);

            let empty_offsets = [];
            for (i, bind_group) in state.bind_groups.iter().enumerate() {
                pass.set_bind_group(i as u32, bind_group, &empty_offsets);
            }

            pass.draw(0..6, instances);
        }
    }
}
