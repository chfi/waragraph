//! Glyph cache and text rendering utilities

// use parking_lot::RwLock;
// use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::{BufferIx, DescSetIx, GpuResources, VkEngine};

use raving::compositor::*;

use ash::vk;

use crate::geometry::{ScreenPoint, ScreenRect};

use anyhow::Result;

use glyph_brush::*;

pub struct TextCache {
    pub brush: GlyphBrush<[u8; 48]>,

    pub font_img: ImageIx,
    pub font_img_view: ImageViewIx,
    pub font_texture_set: DescSetIx,
}

impl TextCache {
    pub fn new(engine: &mut VkEngine) -> Result<Self> {
        //

        todo!();
    }
}

pub(crate) fn glyph_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    font_desc_set: DescSetIx,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res
        .load_shader("shaders/glyph.vert.spv", vk::ShaderStageFlags::VERTEX)?;
    let frag = res.load_shader(
        "shaders/glyph.frag.spv",
        vk::ShaderStageFlags::FRAGMENT, // vk::ShaderStageFlags::VERTEX
                                        //     | vk::ShaderStageFlags::COMPUTE
                                        //     | vk::ShaderStageFlags::FRAGMENT,
    )?;

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

    let size_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(8)
        .build();

    let uv_pos_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(16)
        .build();

    let uv_size_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(3)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(24)
        .build();

    let color_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(4)
        .format(vk::Format::R32G32B32A32_SFLOAT)
        .offset(32)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs =
        [pos_desc, size_desc, uv_pos_desc, uv_size_desc, color_desc];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;
    let vertex_stride = 32;

    SublayerDef::new::<([f32; 2], [f32; 2], [f32; 2], [f32; 2], [f32; 4]), _>(
        ctx,
        res,
        "glyph",
        vert,
        frag,
        clear_pass,
        load_pass,
        vertex_offset,
        vertex_stride,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
        [],
    )
}
