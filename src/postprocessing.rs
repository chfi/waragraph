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

        Ok(Self { pass: pass_ix, pipeline })
    }
}

pub struct EffectInstance {
    def: EffectDef,

    sets: Vec<DescSetIx>,
}
