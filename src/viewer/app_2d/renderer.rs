use ash::vk;
use raving::vk::{
    GpuResources, ImageIx, ImageViewIx, PipelineIx, RenderPassIx, VkContext,
    VkEngine,
};

use anyhow::Result;

// Type for handling resources and drawing for the deferred graph renderer
pub struct GraphRenderer {
    pass: RenderPassIx,
    pipeline: PipelineIx,

    attachments: DeferredAttachments,
}

impl GraphRenderer {
    pub fn initialize(engine: &mut VkEngine, dims: [u32; 2]) -> Result<Self> {
        let (pass, pipeline, attachments) =
            engine.with_allocators(|ctx, res, alloc| {
                let pass_ix = Self::create_pass(ctx, res)?;

                let pass = res[pass_ix];
                let pipeline = Self::create_pipeline(ctx, res, pass)?;

                let attachments = DeferredAttachments::new(engine, dims)?;

                Ok((pass_ix, pipeline, attachments))
            })?;

        Ok(Self {
            pass,
            pipeline,
            attachments,
        })
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
            .attachment(0)
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
            .location(1)
            .format(vk::Format::R32_UINT)
            .offset(16)
            .build();

        let vert_binding_descs = [vert_binding_desc];

        let vert_attr_descs = [p0_desc, p1_desc, node_len_desc];

        let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vert_binding_descs)
            .vertex_attribute_descriptions(&vert_attr_descs);

        // let vertex_offset = 0;

        let pipeline = res.create_graphics_pipeline(
            ctx,
            vert_ix,
            frag_ix,
            pass,
            &vert_input_info,
        )?;

        Ok(pipeline)
    }
}

pub struct DeferredAttachments {
    dims: [u32; 2],

    /// Render target for node/step IDs
    node_index_img: ImageIx,
    node_index_view: ImageViewIx,

    node_uv_img: ImageIx,
    node_uv_view: ImageViewIx,
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
                vk::PipelineStageFlags::FRAGMENT_SHADER,
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
                vk::PipelineStageFlags::FRAGMENT_SHADER,
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
