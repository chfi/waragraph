use crossbeam::atomic::AtomicCell;
use raving::vk::context::VkContext;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, PipelineIx, RenderPassIx,
    ShaderIx, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::{FxHashMap, FxHashSet};
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};
use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufId, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use rhai::plugin::*;

pub struct Compositor {
    window_dims: Arc<AtomicCell<[u32; 2]>>,
    sublayer_defs: BTreeMap<rhai::ImmutableString, SublayerDef>,

    pub pass: RenderPassIx,

    pub layer: Layer,
}

impl Compositor {
    pub fn init(
        engine: &mut VkEngine,
        window_dims: &Arc<AtomicCell<[u32; 2]>>,
        font_desc_set: DescSetIx,
    ) -> Result<Self> {
        let mut sublayer_defs = BTreeMap::default();

        let mut layer = Layer {
            sublayers: Vec::new(),
        };

        let pass = engine.with_allocators(|ctx, res, alloc| {
            let format = vk::Format::R8G8B8A8_UNORM;
            let pass = res.create_line_render_pass(
                ctx,
                format,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::GENERAL,
            )?;

            let pass_ix = res.insert_render_pass(pass);

            let text_def = text_sublayer(ctx, res, font_desc_set, pass)?;
            let rect_def = rect_palette_sublayer(ctx, res, pass)?;

            sublayer_defs.insert(text_def.name.clone(), text_def);
            sublayer_defs.insert(rect_def.name.clone(), rect_def);

            Ok(pass_ix)
        })?;

        Ok(Self {
            window_dims: window_dims.clone(),
            sublayer_defs,
            pass,
            layer,
        })
    }

    pub fn draw<'a>(
        &'a self,
        framebuffer: FramebufferIx,
        extent: vk::Extent2D,
    ) -> Box<dyn Fn(&Device, &GpuResources, vk::CommandBuffer) + 'a> {
        let draw = move |device: &Device,
                         res: &GpuResources,
                         cmd: vk::CommandBuffer| {
            let pass_info = vk::RenderPassBeginInfo::builder()
                .render_pass(res[self.pass])
                .framebuffer(res[framebuffer])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                })
                .clear_values(&[])
                .build();

            unsafe {
                device.cmd_begin_render_pass(
                    cmd,
                    &pass_info,
                    vk::SubpassContents::INLINE,
                );

                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: extent.width as f32,
                    height: extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                };

                let viewports = [viewport];

                device.cmd_set_viewport(cmd, 0, &viewports);

                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                };
                let scissors = [scissor];

                device.cmd_set_scissor(cmd, 0, &scissors);

                for sublayer in self.layer.sublayers.iter() {
                    log::warn!("drawing sublayer {}", sublayer.def_name);
                    let def =
                        self.sublayer_defs.get(&sublayer.def_name).unwrap();

                    let vertices = sublayer.vertex_buffer;

                    let sets = sublayer.sets.iter().copied();
                    let vx_count = sublayer.vertex_count;
                    let i_count = sublayer.instance_count;

                    def.draw(
                        vertices, vx_count, i_count, sets, extent, device, res,
                        cmd,
                    );
                }

                device.cmd_end_render_pass(cmd);
            }
        };

        Box::new(draw)
    }

    pub fn push_sublayer(
        &mut self,
        engine: &mut VkEngine,
        def_name: &str,
        sets: impl IntoIterator<Item = DescSetIx>,
    ) -> Result<()> {
        let def = self.sublayer_defs.get(def_name).ok_or(anyhow!(
            "could not find sublayer definition `{}`",
            def_name
        ))?;

        let capacity = 1024 * 1024;

        let vertex_buffer = engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
            let usage = vk::BufferUsageFlags::VERTEX_BUFFER
                // | vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST;
            let buffer = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                def.vertex_stride,
                capacity,
                usage,
                Some(&format!("sublayer {}", def_name)),
            )?;

            let buf_ix = res.insert_buffer(buffer);

            Ok(buf_ix)
        })?;

        let sublayer = Sublayer {
            def_name: def.name.clone(),

            instance_count: def.default_instance_count.unwrap_or_default(),
            vertex_count: def.default_vertex_count.unwrap_or_default(),
            per_instance: def.per_instance,

            vertex_stride: def.vertex_stride,
            vertex_data: Vec::new(),
            vertex_buffer,

            sets: sets.into_iter().collect(),
        };

        self.layer.sublayers.push(sublayer);

        Ok(())
    }
}

pub struct Layer {
    pub sublayers: Vec<Sublayer>,
}

pub struct Sublayer {
    pub def_name: rhai::ImmutableString,

    vertex_stride: usize,

    vertex_count: usize,
    instance_count: usize,
    per_instance: bool,

    vertex_data: Vec<u8>,

    vertex_buffer: BufferIx,

    sets: Vec<DescSetIx>,
}

impl Sublayer {
    pub fn update_sets(
        &mut self,
        new_sets: impl IntoIterator<Item = DescSetIx>,
    ) {
        self.sets.clear();
        self.sets.extend(new_sets);
    }

    pub fn update_vertices_raw(
        &mut self,
        data: &[u8],
        vertex_count: usize,
        instance_count: usize,
    ) {
        // assert!(data.len() % vertex_count == 0);
        self.vertex_data.clear();
        self.vertex_data.extend_from_slice(data);
        self.vertex_count = vertex_count;
        self.instance_count = instance_count;
    }

    pub fn update_vertices_array<const N: usize, I>(
        &mut self,
        new: I,
    ) -> Result<()>
    where
        I: IntoIterator<Item = [u8; N]>,
    {
        assert!(N == self.vertex_stride);
        if self.per_instance {
            self.vertex_data.clear();
            self.instance_count = 0;
        } else {
            self.vertex_data.clear();
            self.vertex_count = 0;
        }

        for slice in new.into_iter() {
            self.vertex_data.extend_from_slice(&slice);

            if self.per_instance {
                self.instance_count += 1;
            } else {
                self.vertex_count += 1;
            }
        }

        Ok(())
    }

    pub fn update_vertices<'a, I>(&mut self, new: I) -> Result<()>
    where
        I: IntoIterator<Item = &'a [u8]> + 'a,
    {
        if self.per_instance {
            self.vertex_data.clear();
            self.instance_count = 0;
        } else {
            self.vertex_data.clear();
            self.vertex_count = 0;
        }

        for slice in new.into_iter() {
            if slice.len() != self.vertex_stride {
                anyhow::bail!(
                    "slice length {} doesn't match vertex stride {}",
                    slice.len(),
                    self.vertex_stride
                );
            }

            self.vertex_data.extend_from_slice(slice);

            if self.per_instance {
                self.instance_count += 1;
            } else {
                self.vertex_count += 1;
            }
        }

        Ok(())
    }

    pub fn write_buffer(&mut self, res: &mut GpuResources) -> Option<()> {
        assert!(self.vertex_data.len() % self.vertex_stride == 0);

        let buf = &mut res[self.vertex_buffer];
        let slice = buf.mapped_slice_mut()?;
        let len = self.vertex_data.len();
        slice[0..len].clone_from_slice(&self.vertex_data);
        Some(())
    }
}

pub struct SublayerDef {
    pub name: rhai::ImmutableString,

    pub(super) pipeline: PipelineIx,
    pub(super) sets: Vec<DescSetIx>,
    pub(super) vertex_stride: usize,

    per_instance: bool,

    vertex_offset: usize,
    default_vertex_count: Option<usize>,
    default_instance_count: Option<usize>,

    elem_type: std::any::TypeId,
}

impl SublayerDef {
    pub fn draw(
        &self,
        vertices: BufferIx,
        vertex_count: usize,
        instance_count: usize,
        sets: impl IntoIterator<Item = DescSetIx>,
        extent: vk::Extent2D,
        device: &Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        let (pipeline, layout) = res[self.pipeline];

        unsafe {
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );

            let vx_buf_ix = vertices;
            let vx_buf = res[vx_buf_ix].buffer;
            let vxs = [vx_buf];
            device.cmd_bind_vertex_buffers(
                cmd,
                0,
                &vxs,
                &[self.vertex_offset as u64],
            );

            let dims = [extent.width as f32, extent.height as f32];
            let constants = bytemuck::cast_slice(&dims);

            let stages =
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            device.cmd_push_constants(cmd, layout, stages, 0, constants);

            let descriptor_sets = self
                .sets
                .iter()
                .copied()
                .chain(sets.into_iter())
                .map(|s| res[s])
                .collect::<Vec<_>>();

            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &descriptor_sets,
                &[],
            );

            device.cmd_draw(
                cmd,
                vertex_count as u32,
                instance_count as u32,
                0,
                0,
            );
        }
    }

    pub fn new<'a, T, S>(
        ctx: &VkContext,
        res: &mut GpuResources,
        name: &str,
        vert: ShaderIx,
        frag: ShaderIx,
        pass: vk::RenderPass,
        vertex_offset: usize,
        vertex_stride: usize,
        per_instance: bool,
        default_vertex_count: Option<usize>,
        default_instance_count: Option<usize>,
        vert_input_info: vk::PipelineVertexInputStateCreateInfoBuilder<'a>,
        sets: S,
    ) -> Result<Self>
    where
        // T: std::any::Any + Copy + AsBytes,
        S: IntoIterator<Item = DescSetIx>,
        T: std::any::Any + Copy,
    {
        let pipeline = res.create_graphics_pipeline(
            ctx,
            vert,
            frag,
            pass,
            vert_input_info,
        )?;

        Ok(Self {
            name: name.into(),

            pipeline,
            sets: sets.into_iter().collect(),
            vertex_stride,
            vertex_offset,
            per_instance,

            default_vertex_count,
            default_instance_count,

            elem_type: std::any::TypeId::of::<T>(),
        })
    }
}

pub(super) fn rect_palette_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res.load_shader(
        "shaders/tri_2d_window.vert.spv",
        vk::ShaderStageFlags::VERTEX,
    )?;
    let frag = res.load_shader(
        "shaders/rect_flat_color.frag.spv",
        vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(std::mem::size_of::<[f32; 3]>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)
        .build();

    let pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)
        .build();

    let ix_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32_SFLOAT)
        .offset(8)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [pos_desc, ix_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 12;
    let vertex_stride = 12;

    SublayerDef::new::<([f32; 4], u32), _>(
        ctx,
        res,
        "rect-palette",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
        false,
        None,
        Some(1),
        vert_input_info,
        None,
    )
}

pub(super) fn text_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    font_desc_set: DescSetIx,
    pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert =
        res.load_shader("shaders/text.vert.spv", vk::ShaderStageFlags::VERTEX)?;
    let frag = res
        .load_shader("shaders/text.frag.spv", vk::ShaderStageFlags::FRAGMENT)?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(std::mem::size_of::<[f32; 8]>() as u32)
        .input_rate(vk::VertexInputRate::INSTANCE)
        .build();

    let pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)
        .build();

    let ix_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32_UINT)
        .offset(8)
        .build();

    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(16)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [pos_desc, ix_desc, color_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;
    let vertex_stride = 32;

    SublayerDef::new::<([f32; 2], [u32; 2], [f32; 4]), _>(
        ctx,
        res,
        "text",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        [font_desc_set],
    )
}
