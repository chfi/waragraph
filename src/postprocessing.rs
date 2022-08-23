use ash::vk;
use crossbeam::atomic::AtomicCell;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, ImageIx, ImageViewIx,
    PipelineIx, RenderPassIx, ShaderIx, VkContext, VkEngine,
};

use anyhow::Result;

#[derive(Clone, Copy)]
pub struct EffectDef {
    pass: RenderPassIx,
    pipeline: PipelineIx,
}

pub struct EffectAttachments {
    dims: [u32; 2],

    img: ImageIx,
    view: ImageViewIx,

    format: vk::Format,
}

impl EffectAttachments {
    pub fn img_usage() -> vk::ImageUsageFlags {
        vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::STORAGE
    }

    pub fn new(
        engine: &mut VkEngine,
        format: vk::Format,
        dims: [u32; 2],
    ) -> Result<Self> {
        let [width, height] = dims;

        let result = engine.with_allocators(|ctx, res, alloc| {
            let img = res.allocate_image(
                ctx,
                alloc,
                width,
                height,
                format,
                Self::img_usage(),
                Some("effect_attachment"),
            )?;

            let view = res.new_image_view(ctx, &img)?;

            let img = res.insert_image(img);
            let view = res.insert_image_view(view);

            Ok(Self {
                dims,
                img,
                view,
                format,
            })
        })?;

        Ok(result)
    }
}

pub fn create_basic_effect_pass(
    ctx: &VkContext,
    res: &mut GpuResources,
) -> Result<RenderPassIx> {
    let format = vk::Format::R8G8B8A8_UNORM;

    let attch_desc = vk::AttachmentDescription::builder()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .build();

    let attch_descs = [attch_desc];

    let attch_ref = vk::AttachmentReference::builder()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .build();

    let attch_refs = [attch_ref];

    let subpass_desc = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&attch_refs)
        .build();

    let subpass_descs = [subpass_desc];

    let render_pass_info = vk::RenderPassCreateInfo::builder()
        .attachments(&attch_descs)
        .subpasses(&subpass_descs)
        .build();

    let render_pass =
        unsafe { ctx.device().create_render_pass(&render_pass_info, None) }?;

    let pass = res.insert_render_pass(render_pass);

    Ok(pass)
}

static POSTPROCESS_VERT_SPIRV_SRC: &'static [u8] =
    raving::include_shader!("postprocessing/base.vert.spv");

const POSTPROCESS_VERT_SHADER: AtomicCell<Option<ShaderIx>> =
    AtomicCell::new(None);

impl EffectDef {
    pub fn new(
        ctx: &VkContext,
        res: &mut GpuResources,
        pass_ix: RenderPassIx,
        frag: ShaderIx,
    ) -> Result<Self> {
        if POSTPROCESS_VERT_SHADER.load().is_none() {
            let shader = res
                .new_shader(
                    bytemuck::cast_slice(POSTPROCESS_VERT_SPIRV_SRC),
                    vk::ShaderStageFlags::VERTEX,
                )
                .expect("Error loading postprocessing vertex shader");

            let shader = res.insert_shader(shader);

            POSTPROCESS_VERT_SHADER.store(Some(shader));
        }

        let vert = POSTPROCESS_VERT_SHADER.load().unwrap();

        let vert_input_info = vk::PipelineVertexInputStateCreateInfo::default();

        let rasterizer_info =
            vk::PipelineRasterizationStateCreateInfo::builder()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1.0)
                // .cull_mode(vk::CullModeFlags::BACK)
                // .cull_mode(vk::CullModeFlags::FRONT)
                .cull_mode(vk::CullModeFlags::NONE)
                // .front_face(vk::FrontFace::CLOCKWISE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false)
                .depth_bias_constant_factor(0.0)
                .depth_bias_clamp(0.0)
                .depth_bias_slope_factor(0.0)
                .build();

        // TODO: attachments should be figured out programmatically,
        // but I'll stick to a single hardcoded color attachment for now

        let color_blend_attachment =
            vk::PipelineColorBlendAttachmentState::builder()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(true)
                .build();

        let color_blend_attachments = [color_blend_attachment];

        let color_blending_info =
            vk::PipelineColorBlendStateCreateInfo::builder()
                .attachments(&color_blend_attachments)
                .blend_constants([0.0, 0.0, 0.0, 0.0])
                .build();

        let pass = res[pass_ix];

        let pipeline = res.create_graphics_pipeline_impl(
            ctx,
            vert,
            frag,
            pass,
            &vert_input_info,
            &rasterizer_info,
            &color_blending_info,
        )?;

        Ok(Self {
            pass: pass_ix,
            pipeline,
        })
    }
}

pub struct EffectInstance {
    def: EffectDef,

    sets: Vec<DescSetIx>,
}
