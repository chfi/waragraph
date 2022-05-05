use raving::vk::context::VkContext;
use raving::vk::{
    BufferIx, DescSetIx, GpuResources, PipelineIx, RenderPassIx, ShaderIx,
    VkEngine,
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
    sublayer_defs: BTreeMap<rhai::ImmutableString, SublayerDef>,
}

pub struct Layer {
    sublayers: Vec<Sublayer>,
}

pub struct Sublayer {
    pub def_name: rhai::ImmutableString,

    vertex_stride: usize,
    vertex_data: Vec<u8>,

    vertex_buffer: BufferIx,
}

impl Sublayer {
    pub fn write_buffer(&mut self, res: &mut GpuResources) -> Option<()> {
        assert!(self.vertex_data.len() % self.vertex_stride == 0);
        // if self.used_bytes == 0 {
        //     return Some(());
        // }
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

    vertex_offset: usize,
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

    pub fn new<'a>(
        ctx: &VkContext,
        res: &mut GpuResources,
        name: &str,
        vert: ShaderIx,
        frag: ShaderIx,
        pass: vk::RenderPass,
        vertex_offset: usize,
        vertex_stride: usize,
        vert_input_info: vk::PipelineVertexInputStateCreateInfoBuilder<'a>,
        sets: impl IntoIterator<Item = DescSetIx>,
    ) -> Result<Self> {
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
        })
    }
}

pub(super) fn rect_palette_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert =
        res.load_shader("shaders/rect.vert.spv", vk::ShaderStageFlags::VERTEX)?;
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

    SublayerDef::new(
        ctx,
        res,
        "rect-palette",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
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

    SublayerDef::new(
        ctx,
        res,
        "text",
        vert,
        frag,
        pass,
        vertex_offset,
        vertex_stride,
        vert_input_info,
        [font_desc_set],
    )
}
