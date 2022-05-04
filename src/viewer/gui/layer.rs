use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::context::VkContext;
use raving::vk::{
    BatchInput, BufferIx, DescSetIx, FrameResources, FramebufferIx,
    GpuResources, PipelineIx, RenderPassIx, ShaderIx, VkEngine,
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

pub struct SublayerDesc {
    pub name: rhai::ImmutableString,

    pub(super) pipeline: PipelineIx,
    pub(super) sets: Vec<DescSetIx>,
    pub(super) vertex_stride: usize,
}

impl SublayerDesc {
    pub fn new<'a>(
        ctx: &VkContext,
        res: &mut GpuResources,
        name: &str,
        vert: ShaderIx,
        frag: ShaderIx,
        pass: vk::RenderPass,
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
        })
    }
}

pub(super) fn text_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    name: &str,
    font_desc_set: DescSetIx,
    pass: vk::RenderPass,
) -> Result<SublayerDesc> {
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

    let vertex_stride = 32;

    SublayerDesc::new(
        ctx,
        res,
        "text",
        vert,
        frag,
        pass,
        vertex_stride,
        vert_input_info,
        [font_desc_set],
    )
}
