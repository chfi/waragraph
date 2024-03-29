use std::collections::HashMap;

use ash::vk;
use crossbeam::atomic::AtomicCell;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, ImageIx, ImageViewIx,
    PipelineIx, RenderPassIx, SamplerIx, ShaderIx, VkContext, VkEngine,
};

use anyhow::Result;

pub struct Postprocessing {
    effects: HashMap<rhai::ImmutableString, EffectDef>,

    // attachments: HashMap<rhai::ImmutableString, EffectAttachments>,
    pub nn_sampler: SamplerIx,
    pub lin_sampler: SamplerIx,
}

impl Postprocessing {
    pub fn initialize(engine: &mut VkEngine) -> Result<Self> {
        let (nn_sampler, lin_sampler) =
            engine.with_allocators(|ctx, res, _alloc| {
                let nn_sampler = Self::create_nn_sampler(ctx, res)?;
                let lin_sampler = Self::create_lin_sampler(ctx, res)?;

                Ok((nn_sampler, lin_sampler))
            })?;

        Ok(Self {
            effects: Default::default(),
            // attachments: Default::default(),
            nn_sampler,
            lin_sampler,
        })
    }

    fn create_nn_sampler(
        ctx: &VkContext,
        res: &mut GpuResources,
    ) -> Result<SamplerIx> {
        let sampler = {
            let sampler_info = vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .anisotropy_enable(false)
                // .anisotropy_enable(true)
                // .max_anisotropy(16.0)
                .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                .mip_lod_bias(0.0)
                .min_lod(0.0)
                .max_lod(1.0)
                .unnormalized_coordinates(false)
                .build();

            res.insert_sampler(ctx, sampler_info)?
        };

        Ok(sampler)
    }

    fn create_lin_sampler(
        ctx: &VkContext,
        res: &mut GpuResources,
    ) -> Result<SamplerIx> {
        let sampler = {
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
                .unnormalized_coordinates(false)
                .build();

            res.insert_sampler(ctx, sampler_info)?
        };

        Ok(sampler)
    }
}

#[derive(Clone, Copy)]
pub struct EffectDef {
    pub pass: RenderPassIx,
    pub pipeline: PipelineIx,

    pub frag: ShaderIx,
}

pub struct EffectAttachments {
    pub dims: [u32; 2],

    pub img: ImageIx,
    pub view: ImageViewIx,

    pub format: vk::Format,
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

    pub fn transition_to_read(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        let src_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        let dst_mask = vk::AccessFlags::SHADER_READ;

        let src_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        let dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;

        let old_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        let new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        self.transition(
            device, res, cmd, src_mask, dst_mask, src_stage, dst_stage,
            old_layout, new_layout,
        );
    }

    pub fn transition_to_write(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        let src_mask = vk::AccessFlags::SHADER_READ;
        let dst_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

        let src_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        let dst_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;

        let old_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let new_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        self.transition(
            device, res, cmd, src_mask, dst_mask, src_stage, dst_stage,
            old_layout, new_layout,
        );
    }

    fn transition(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
        src_mask: vk::AccessFlags,
        dst_mask: vk::AccessFlags,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        unsafe {
            let img_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(src_mask)
                .dst_access_mask(dst_mask)
                .old_layout(old_layout)
                .new_layout(new_layout)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(res[self.img].image)
                .build();

            let image_barriers = [img_barrier];

            device.cmd_pipeline_barrier(
                cmd,
                src_stage,
                dst_stage,
                vk::DependencyFlags::BY_REGION,
                &[],
                &[],
                &image_barriers,
            );
        }
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

static POSTPROCESS_VERT_SHADER: AtomicCell<Option<ShaderIx>> =
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
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
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

            frag,
        })
    }
}

pub struct EffectInstance {
    pub def: EffectDef,

    pub sets: Vec<DescSetIx>,

    pub attachments: EffectAttachments,
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
            "shaders/viewer_2d/nodes_deferred_effect.frag.spv",
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
            // vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            // vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::SHADER_WRITE,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            vk::ImageLayout::UNDEFINED,
            // vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );

        /*
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
        */

        Ok(())
    })?;

    Ok((result, fb))
}
