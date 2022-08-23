use std::collections::HashMap;

use ash::vk;
use crossbeam::atomic::AtomicCell;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, ImageIx, ImageViewIx,
    PipelineIx, RenderPassIx, ShaderIx, VkContext, VkEngine,
};

use anyhow::Result;

pub struct Postprocessing {
    effects: HashMap<rhai::ImmutableString, EffectDef>,

    attachments: HashMap<rhai::ImmutableString, EffectAttachments>,
}

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
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        format: vk::Format,
        dims: [u32; 2],
    ) -> Result<Self> {
        let [width, height] = dims;

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
    }

    pub fn framebuffer(
        &self,
        ctx: &VkContext,
        res: &mut GpuResources,
        pass: RenderPassIx,
    ) -> Result<vk::Framebuffer> {
        let view = res[self.view];

        let attchs = [view];

        let [width, height] = self.dims;

        res.create_framebuffer(ctx, pass, &attchs, width, height)
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

    attachments: EffectAttachments,
}

impl EffectInstance {
    pub fn draw(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        input: DescSetIx,
        framebuffer: vk::Framebuffer,
        cmd: vk::CommandBuffer,
    ) -> Result<()> {
        //

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 0.0],
            },
        }];

        let [width, height] = self.attachments.dims;

        let pass = res[self.def.pass];

        let extent = vk::Extent2D { width, height };

        let pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(pass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .clear_values(&clear_values)
            .build();

        let (pipeline, layout) = res[self.def.pipeline].pipeline_and_layout();

        unsafe {
            device.cmd_begin_render_pass(
                cmd,
                &pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
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

            /*
            let vx_bufs = [res[vertex_buf].buffer];
            let offsets = [0];
            device.cmd_bind_vertex_buffers(cmd, 0, &vx_bufs, &offsets);
            device.cmd_bind_index_buffer(
                cmd,
                res[index_buf].buffer,
                0,
                vk::IndexType::UINT32,
            );
            */

            let desc_sets = [res[input]];
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &desc_sets,
                &[],
            );

            let const_vals = [extent.width as f32, extent.height as f32];

            let constants: &[u8] = bytemuck::cast_slice(&const_vals);

            device.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                constants,
            );

            device.cmd_draw(cmd, 3, 1, 0, 0);

            device.cmd_end_render_pass(cmd);
        }

        Ok(())
    }
}

pub fn test_effect_instance(
    engine: &mut VkEngine,
) -> Result<(EffectInstance, vk::Framebuffer)> {
    let format = vk::Format::R8G8B8A8_UNORM;

    let (result, fb) = engine.with_allocators(|ctx, res, alloc| {
        let pass = create_basic_effect_pass(ctx, res)?;

        let frag = res.load_shader(
            "shaders/postprocessing/nodes_deferred_effect.frag.spv",
            vk::ShaderStageFlags::FRAGMENT,
        )?;

        let frag = res.insert_shader(frag);

        let def = EffectDef::new(ctx, res, pass, frag)?;

        let attchs =
            EffectAttachments::new(ctx, res, alloc, format, [1024, 1024])?;

        let framebuffer = attchs.framebuffer(ctx, res, pass)?;

        let effect = EffectInstance {
            def,
            attachments: attchs,
            sets: Vec::new(),
        };

        Ok((effect, framebuffer))
    })?;

    engine.submit_queue_fn(|ctx, res, alloc, cmd| {
        let img = &res[result.attachments.img];

        VkEngine::transition_image(
            cmd,
            ctx.device(),
            img.image,
            vk::AccessFlags::empty(),
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );

        Ok(())
    })?;

    Ok((result, fb))
}
