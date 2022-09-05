use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    BufferIx, DescSetIx, FramebufferIx, GpuResources, ImageIx, ImageViewIx,
    PipelineIx, RenderPassIx, SamplerIx, ShaderIx, VkContext, VkEngine,
};

use anyhow::Result;

use crate::{geometry::graph::GraphLayout, graph::Waragraph};

// Type for handling resources and drawing for the deferred graph renderer
pub struct GraphRenderer {
    pass: RenderPassIx,
    pipeline: PipelineIx,

    pub attachments: DeferredAttachments,
    framebuffer: vk::Framebuffer,

    vertex_buffer: BufferIx,
}

impl GraphRenderer {
    pub fn initialize(
        engine: &mut VkEngine,
        graph: &Waragraph,
        layout: &GraphLayout<(), ()>,
        dims: [u32; 2],
    ) -> Result<Self> {
        let attachments = DeferredAttachments::new(engine, dims)?;

        let (vertex_buffer, pass, pipeline, framebuffer) = engine
            .with_allocators(|ctx, res, alloc| {
                let pass_ix = Self::create_pass(ctx, res)?;

                let pass = res[pass_ix];
                let pipeline = Self::create_pipeline(ctx, res, pass)?;

                let framebuffer = attachments.framebuffer(ctx, res, pass_ix)?;

                let usage = vk::BufferUsageFlags::VERTEX_BUFFER;

                let stride = 5 * 4;

                let mut vx_buf = res.allocate_buffer(
                    ctx,
                    alloc,
                    gpu_allocator::MemoryLocation::CpuToGpu,
                    stride,
                    graph.node_count(),
                    usage,
                    Some("Deferred Node Vertex Buffer"),
                )?;

                {
                    let dst = vx_buf.mapped_slice_mut().unwrap();

                    for (node_ix, len) in graph.node_lens.iter().enumerate() {
                        let v_ix = node_ix * 2;
                        let p0 = layout.vertices[v_ix];
                        let p1 = layout.vertices[v_ix + 1];

                        let start = node_ix * stride;
                        let end = start + stride;

                        dst[start..(start + 16)].clone_from_slice(
                            bytemuck::cast_slice(&[p0.x, p0.y, p1.x, p1.y]),
                        );
                        dst[(start + 16)..end]
                            .clone_from_slice(bytemuck::cast_slice(&[*len]));
                    }
                }

                let vx_buf_ix = res.insert_buffer(vx_buf);

                Ok((vx_buf_ix, pass_ix, pipeline, framebuffer))
            })?;

        Ok(Self {
            pass,
            pipeline,

            attachments,
            framebuffer,

            vertex_buffer,
        })
    }

    pub fn draw_first_pass(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        // vertex_buf: BufferIx,
        index_buf: BufferIx,
        ubo: DescSetIx,
        index_count: u32,
        instance_count: u32,
        node_width: f32,
        cmd: vk::CommandBuffer,
    ) -> Result<()> {
        //

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    int32: [-1, 0, 0, 0],
                },
            },
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
                },
            },
        ];

        let vertex_buf = self.vertex_buffer;

        let [width, height] = self.attachments.dims;

        let pass = res[self.pass];

        let extent = vk::Extent2D { width, height };

        let pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(pass)
            .framebuffer(self.framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .clear_values(&clear_values)
            .build();

        let vertices = &res[vertex_buf];
        let indices = &res[index_buf];

        let (pipeline, layout) = res[self.pipeline].pipeline_and_layout();

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

            let vx_bufs = [res[vertex_buf].buffer];
            let offsets = [0];
            device.cmd_bind_vertex_buffers(cmd, 0, &vx_bufs, &offsets);
            device.cmd_bind_index_buffer(
                cmd,
                res[index_buf].buffer,
                0,
                vk::IndexType::UINT32,
            );

            let desc_sets = [res[ubo]];
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &desc_sets,
                &[],
            );

            // let dims = [extent.width as f32, extent.height as f32];
            // let node_width = [node_width];
            let const_vals =
                [extent.width as f32, extent.height as f32, node_width];

            let constants: &[u8] = bytemuck::cast_slice(&const_vals);

            device.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                constants,
            );

            device.cmd_draw_indexed(cmd, index_count, instance_count, 0, 0, 0);

            device.cmd_end_render_pass(cmd);
        }

        Ok(())
    }

    pub fn first_pass_barrier(
        &self,
        ctx: &VkContext,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        unsafe {
            let dev = ctx.device();

            let src_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
            let dst_mask = vk::AccessFlags::SHADER_READ;

            let src_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
            let dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;

            let barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(src_mask)
                .dst_access_mask(dst_mask)
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED);

            let index_img = res[self.attachments.node_index_img].image;
            let uv_img = res[self.attachments.node_uv_img].image;

            let mut index_barrier = barrier.clone();
            index_barrier.image = index_img;

            let mut uv_barrier = barrier.clone();
            uv_barrier.image = uv_img;

            let image_barriers = [index_barrier, uv_barrier];

            dev.cmd_pipeline_barrier(
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

    fn create_pass(
        ctx: &VkContext,
        res: &mut GpuResources,
    ) -> Result<RenderPassIx> {
        let index_attch_desc = vk::AttachmentDescription::builder()
            .format(DeferredAttachments::NODE_INDEX_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .build();

        let uv_attch_desc = vk::AttachmentDescription::builder()
            .format(DeferredAttachments::NODE_UV_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .build();

        let attch_descs = [index_attch_desc, uv_attch_desc];

        let index_attch_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();

        let uv_attch_ref = vk::AttachmentReference::builder()
            .attachment(1)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();

        let attch_refs = [index_attch_ref, uv_attch_ref];

        let subpass_desc = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&attch_refs)
            .build();

        let subpass_descs = [subpass_desc];

        /*
        let subpass_dep = vk::SubpassDependency::builder()
        //.src_subpass(vk::SUBPASS_EXTERNAL)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        |vk::PipelineStageFlags::COMPUTE_SHADER)
        .src_access_mask()
        */

        // let subpass_deps = [];

        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attch_descs)
            .subpasses(&subpass_descs)
            .build();

        let render_pass = unsafe {
            ctx.device().create_render_pass(&render_pass_info, None)
        }?;

        let pass = res.insert_render_pass(render_pass);

        Ok(pass)
    }

    fn create_pipeline(
        ctx: &VkContext,
        res: &mut GpuResources,
        pass: vk::RenderPass,
    ) -> Result<PipelineIx> {
        //

        let vert = res.load_shader(
            "shaders/viewer_2d/nodes_deferred.vert.spv",
            vk::ShaderStageFlags::VERTEX,
        )?;

        let frag = res.load_shader(
            "shaders/viewer_2d/nodes_deferred.frag.spv",
            vk::ShaderStageFlags::FRAGMENT,
        )?;

        let vert_ix = res.insert_shader(vert);
        let frag_ix = res.insert_shader(frag);

        let vertex_stride = std::mem::size_of::<([f32; 4], u32)>();

        let vert_binding_desc = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(vertex_stride as u32)
            .input_rate(vk::VertexInputRate::INSTANCE)
            .build();

        let p0_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0)
            .build();

        let p1_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(8)
            .build();

        let node_len_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R32_UINT)
            .offset(16)
            .build();

        let vert_binding_descs = [vert_binding_desc];

        let vert_attr_descs = [p0_desc, p1_desc, node_len_desc];

        let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vert_binding_descs)
            .vertex_attribute_descriptions(&vert_attr_descs);

        // let vertex_offset = 0;
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

        let index_blend_attachment =
            vk::PipelineColorBlendAttachmentState::builder()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(false)
                .build();

        let uv_blend_attachment =
            vk::PipelineColorBlendAttachmentState::builder()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                // .color_write_mask(
                //     vk::ColorComponentFlags::R | vk::ColorComponentFlags::G,
                // )
                .blend_enable(false)
                .build();

        let color_blend_attachments =
            [index_blend_attachment, uv_blend_attachment];

        let color_blending_info =
            vk::PipelineColorBlendStateCreateInfo::builder()
                // .logic_op_enable(false)
                // .logic_op(vk::LogicOp::COPY)
                .attachments(&color_blend_attachments)
                .blend_constants([0.0, 0.0, 0.0, 0.0])
                .build();

        let pipeline = res.create_graphics_pipeline_impl(
            ctx,
            vert_ix,
            frag_ix,
            pass,
            &vert_input_info,
            &rasterizer_info,
            &color_blending_info,
        )?;

        Ok(pipeline)
    }
}

#[derive(Clone)]
pub struct AttachmentSet {
    dims: [u32; 2],

    names: Vec<String>,
    images: Vec<ImageIx>,
    views: Vec<ImageViewIx>,
    formats: Vec<vk::Format>,

    initialized: bool,
}

impl AttachmentSet {
    fn image_usage() -> vk::ImageUsageFlags {
        vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::STORAGE
    }

    pub fn new<'a>(
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        dims: [u32; 2],
        entries: impl IntoIterator<Item = (&'a str, vk::Format)>,
    ) -> Result<Self> {
        let [width, height] = dims;

        let mut names = Vec::new();
        let mut images = Vec::new();
        let mut views = Vec::new();
        let mut formats = Vec::new();

        let usage = Self::image_usage();

        for (name, format) in entries {
            let image = res.allocate_image(
                ctx,
                alloc,
                width,
                height,
                format,
                usage,
                Some(name),
            )?;

            let view = res.new_image_view(ctx, &image)?;

            let image = res.insert_image(image);
            let view = res.insert_image_view(view);

            names.push(name.to_string());
            images.push(image);
            views.push(view);
            formats.push(format);
        }

        Ok(Self {
            dims,

            names,
            images,
            views,
            formats,

            initialized: false,
        })
    }

    pub fn initialize(&mut self, engine: &mut VkEngine) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        engine.submit_queue_fn(|ctx, res, _alloc, cmd| {
            let src_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
            let dst_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;

            let src_access = vk::AccessFlags::empty();
            let dst_access = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

            let old_layout = vk::ImageLayout::UNDEFINED;
            let new_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

            self.transition(
                ctx.device(),
                res,
                cmd,
                src_access,
                dst_access,
                src_stage,
                dst_stage,
                old_layout,
                new_layout,
            );

            Ok(())
        })?;

        self.initialized = true;

        Ok(())
    }

    pub fn transition_to_write(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        let src_access = vk::AccessFlags::SHADER_READ;
        let dst_access = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;

        let src_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        let dst_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;

        let old_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let new_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        self.transition(
            device, res, cmd, src_access, dst_access, src_stage, dst_stage,
            old_layout, new_layout,
        );
    }

    fn transition(
        &self,
        device: &ash::Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        let mut image_barriers = Vec::new();

        for &image in self.images.iter() {
            let image_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(src_access)
                .dst_access_mask(dst_access)
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
                .image(res[image].image)
                .build();

            image_barriers.push(image_barrier);
        }

        unsafe {
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

    pub fn create_desc_set_for_shader(
        &self,
        res: &mut GpuResources,
        shader: ShaderIx,
        set_ix: u32,
        sampler: SamplerIx,
    ) -> Result<DescSetIx> {
        let (layout_info, set_info) = {
            let shader = &res[shader];

            let layout_info = shader.set_layout_info(set_ix)?;
            let set_info = shader.set_infos[&set_ix].clone();

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

                for (i, &view) in self.views.iter().enumerate() {
                    let binding = 1 + i as u32;

                    let image_info = vk::DescriptorImageInfo::builder()
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image_view(res[view])
                        .build();

                    desc_builder.bind_image(binding, &[image_info]);
                }

                Ok(())
            },
        )?;

        let set_ix = res.insert_desc_set(desc_set);

        Ok(set_ix)
    }

    pub fn framebuffer(
        &self,
        ctx: &VkContext,
        res: &mut GpuResources,
        pass: RenderPassIx,
    ) -> Result<vk::Framebuffer> {
        let mut attchs = Vec::new();

        for &view in self.views.iter() {
            attchs.push(res[view]);
        }

        let [width, height] = self.dims;

        res.create_framebuffer(ctx, pass, &attchs, width, height)
    }
}

pub struct DeferredAttachments {
    pub dims: [u32; 2],

    /// Render target for node/step IDs
    pub node_index_img: ImageIx,
    pub node_index_view: ImageViewIx,

    pub node_uv_img: ImageIx,
    pub node_uv_view: ImageViewIx,
}

impl DeferredAttachments {
    pub const NODE_INDEX_FORMAT: vk::Format = vk::Format::R32_UINT;
    pub const NODE_UV_FORMAT: vk::Format = vk::Format::R32G32_SFLOAT;

    // can't be const because `|` on image usage flags isn't const
    pub fn node_index_usage() -> vk::ImageUsageFlags {
        vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::STORAGE
    }

    pub fn node_uv_usage() -> vk::ImageUsageFlags {
        vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::STORAGE
    }

    pub fn create_desc_set_for_shader(
        &self,
        res: &mut GpuResources,
        shader: ShaderIx,
        set_ix: u32,
        sampler: SamplerIx,
    ) -> Result<DescSetIx> {
        let (layout_info, set_info) = {
            let shader = &res[shader];

            let layout_info = shader.set_layout_info(set_ix)?;
            let set_info = shader.set_infos[&set_ix].clone();

            (layout_info, set_info)
        };

        let desc_set = res.allocate_desc_set_raw(
            &layout_info,
            &set_info,
            |res, desc_builder| {
                let sampler_info = vk::DescriptorImageInfo::builder()
                    .sampler(res[sampler])
                    .build();

                let id_view = res[self.node_index_view];
                let uv_view = res[self.node_uv_view];

                let index_info = vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(id_view)
                    .build();

                let uv_info = vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(uv_view)
                    .build();

                desc_builder.bind_image(0, &[sampler_info]);
                desc_builder.bind_image(1, &[index_info]);
                desc_builder.bind_image(2, &[uv_info]);

                Ok(())
            },
        )?;

        let set_ix = res.insert_desc_set(desc_set);

        Ok(set_ix)
    }

    pub fn new(engine: &mut VkEngine, dims: [u32; 2]) -> Result<Self> {
        let [width, height] = dims;

        let result = engine.with_allocators(|ctx, res, alloc| {
            let index_img = res.allocate_image(
                ctx,
                alloc,
                width,
                height,
                Self::NODE_INDEX_FORMAT,
                Self::node_index_usage(),
                Some("deferred_node_index"),
            )?;

            let uv_img = res.allocate_image(
                ctx,
                alloc,
                width,
                height,
                Self::NODE_UV_FORMAT,
                Self::node_uv_usage(),
                Some("deferred_node_uv"),
            )?;

            let index_view = res.new_image_view(ctx, &index_img)?;
            let uv_view = res.new_image_view(ctx, &uv_img)?;

            let node_index_img = res.insert_image(index_img);
            let node_index_view = res.insert_image_view(index_view);

            let node_uv_img = res.insert_image(uv_img);
            let node_uv_view = res.insert_image_view(uv_view);

            Ok(Self {
                dims,

                node_index_img,
                node_index_view,

                node_uv_img,
                node_uv_view,
            })
        })?;

        // transition images
        engine.submit_queue_fn(|ctx, res, alloc, cmd| {
            let index_img = &res[result.node_index_img];
            let uv_img = &res[result.node_uv_img];

            VkEngine::transition_image(
                cmd,
                ctx.device(),
                index_img.image,
                vk::AccessFlags::empty(),
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );

            VkEngine::transition_image(
                cmd,
                ctx.device(),
                uv_img.image,
                vk::AccessFlags::empty(),
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );

            Ok(())
        })?;

        Ok(result)
    }

    pub fn framebuffer(
        &self,
        ctx: &VkContext,
        res: &mut GpuResources,
        pass: RenderPassIx,
    ) -> Result<vk::Framebuffer> {
        let index_view = res[self.node_index_view];
        let uv_view = res[self.node_uv_view];

        let attchs = [index_view, uv_view];

        let [width, height] = self.dims;

        res.create_framebuffer(ctx, pass, &attchs, width, height)
    }

    pub fn reallocate(
        &mut self,
        engine: &mut VkEngine,
        dims: [u32; 2],
    ) -> Result<()> {
        todo!();
    }
}
