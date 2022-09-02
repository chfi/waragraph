use raving::vk::context::VkContext;
use raving::vk::{
    DescSetIx, GpuResources, ImageViewIx, 
};

use raving::compositor::*;

use ash::vk;

use anyhow::Result;

use zerocopy::AsBytes;

pub fn image_vertex(
    src_offset: [f32; 2],
    src_size: [f32; 2],
    dst_offset: [f32; 2],
    dst_size: [f32; 2],
) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.clone_from_slice(
        [src_offset, src_size, dst_offset, dst_size].as_bytes(),
    );
    out
}

pub fn create_image_desc_set(
    res: &mut GpuResources,
    compositor: &mut Compositor,
    img_view: ImageViewIx,
) -> Result<DescSetIx> {
    let sublayer_def = &compositor.sublayer_defs["sample_image"];

    let frag = res[sublayer_def.clear_pipeline].fragment.unwrap();

    let (layout_info, set_info) = {
        let shader = &res[frag];

        let layout_info = shader.set_layout_info(0)?;
        let set_info = shader.set_infos[&0].clone();

        (layout_info, set_info)
    };

    let desc_set = res.allocate_desc_set_raw(
        &layout_info,
        &set_info,
        |res, desc_builder| {
            let image_info = vk::DescriptorImageInfo::builder()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(res[img_view])
                .build();
            desc_builder.bind_image(0, &[image_info]);
            Ok(())
        },
    )?;

    let set_ix = res.insert_desc_set(desc_set);

    Ok(set_ix)
}

pub(super) fn image_sublayer(
    ctx: &VkContext,
    res: &mut GpuResources,
    clear_pass: vk::RenderPass,
    load_pass: vk::RenderPass,
) -> Result<SublayerDef> {
    let vert = res.load_shader(
        "shaders/compositor/sample_image.vert.spv",
        vk::ShaderStageFlags::VERTEX,
    )?;
    let frag = res.load_shader(
        "shaders/compositor/sample_image.frag.spv",
        vk::ShaderStageFlags::FRAGMENT,
    )?;

    let vert = res.insert_shader(vert);
    let frag = res.insert_shader(frag);

    let vertex_size = std::mem::size_of::<[f32; 8]>();

    let sampler_set: DescSetIx = {
        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .anisotropy_enable(false)
            // .anisotropy_enable(true)
            // .max_anisotropy(16.0)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(1.0)
            .unnormalized_coordinates(true)
            .build();

        let sampler = res.insert_sampler(ctx, sampler_info)?;

        let (layout_info, set_info) = {
            let shader = &res[frag];

            let layout_info = shader.set_layout_info(0)?;
            let set_info = shader.set_infos[&0].clone();

            (layout_info, set_info)
        };

        let desc_set = res.allocate_desc_set_raw(
            &layout_info,
            &set_info,
            |res, desc_builder| {
                let sampler_info = vk::DescriptorImageInfo::builder()
                    .sampler(res[sampler])
                    .build();
                desc_builder.bind_image(0, &[sampler_info]);
                Ok(())
            },
        )?;

        let set_ix = res.insert_desc_set(desc_set);

        set_ix
    };

    let vert_binding_desc = vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(std::mem::size_of::<[f32; 8]>() as u32)
        .input_rate(vk::VertexInputRate::INSTANCE)
        .build();

    let dst_offset_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(0)
        .build();

    let dst_size_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(8)
        .build();

    let src_offset_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(2)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(16)
        .build();

    let src_size_desc = vk::VertexInputAttributeDescription::builder()
        .binding(0)
        .location(3)
        .format(vk::Format::R32G32_SFLOAT)
        .offset(24)
        .build();

    let vert_binding_descs = [vert_binding_desc];
    let vert_attr_descs = [
        dst_offset_desc,
        dst_size_desc,
        src_offset_desc,
        src_size_desc,
    ];

    let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&vert_binding_descs)
        .vertex_attribute_descriptions(&vert_attr_descs);

    let vertex_offset = 0;

    SublayerDef::new::<([[f32; 2]; 4]), _>(
        ctx,
        res,
        "image",
        vert,
        frag,
        clear_pass,
        load_pass,
        vertex_offset,
        vertex_size,
        true,
        Some(6),
        None,
        vert_input_info,
        None,
        [sampler_set],
    )
}
