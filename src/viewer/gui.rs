use bstr::ByteSlice;
use parking_lot::RwLock;
use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, BufferIx, DescSetIx, FrameResources, FramebufferIx,
    GpuResources, PipelineIx, RenderPassIx, VkEngine,
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

type LabelId = u64;

// pub struct GuiLayer {
//     labels: FxHashSet<LabelId>,
// }

pub struct GuiSys {
    pub config: ConfigMap,

    pub labels: LabelStorage,
    pub label_updates: sled::Subscriber,

    pub rects: Vec<[f32; 4]>,
    // pub rhai_module: Arc<rhai::Module>,

    // pub on_resize: RhaiBatchFn2<i64, i64>,

    // pub draw_labels: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    // pub draw_shapes: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    pub pass: RenderPassIx,
    pub pipeline: PipelineIx,

    buf_id: BufId,
    pub buf_ix: BufferIx,
}

impl GuiSys {
    const VX_BUF_NAME: &'static str = "waragraph:gui:vertices";

    pub fn update_buffer(&self, buffers: &BufferStorage) -> Result<()> {
        let vx_count = self.rects.len() * 6;
        let mut vertices: Vec<[f32; 2]> = Vec::with_capacity(vx_count);

        for &[x, y, w, h] in self.rects.iter() {
            vertices.push([x, y]);
            vertices.push([x, y + h]);
            vertices.push([x + w, y]);

            vertices.push([x, y + h]);
            vertices.push([x + w, y + h]);
            vertices.push([x + w, y]);
        }

        buffers.insert_data(self.buf_id, &vertices)?;

        Ok(())
    }

    pub fn init(
        engine: &mut VkEngine,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        width: u32,
        height: u32,
        // height: u32,
    ) -> Result<Self> {
        let mut config = ConfigMap::default();

        let mut labels = LabelStorage::new(&db)?;
        let label_updates = labels.tree.watch_prefix(b"t:");

        let (pass_ix, pipeline_ix) = {
            // let format = engine.swapchain_props.format.format;
            let format = vk::Format::R8G8B8A8_UNORM;

            engine.with_allocators(|ctx, res, _| {
                let pass = res.create_line_render_pass(
                    ctx,
                    format,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::GENERAL,
                )?;

                let vert = res.load_shader(
                    "shaders/rect.vert.spv",
                    vk::ShaderStageFlags::VERTEX,
                )?;
                let frag = res.load_shader(
                    "shaders/rect_flat_color.frag.spv",
                    vk::ShaderStageFlags::FRAGMENT,
                )?;

                let pass_ix = res.insert_render_pass(pass);
                let vx = res.insert_shader(vert);
                let fx = res.insert_shader(frag);

                let pass = res[pass_ix];

                let pipeline_ix =
                    res.create_graphics_pipeline_tmp(ctx, vx, fx, pass)?;

                Ok((pass_ix, pipeline_ix))
            })?
        };

        dbg!();
        let buf_id = buffers.allocate_buffer_with_usage(
            engine,
            &db,
            Self::VX_BUF_NAME,
            BufFmt::FVec2,
            1023,
            vk::BufferUsageFlags::VERTEX_BUFFER
                | vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST,
        )?;
        dbg!();
        let buf_ix = buffers.get_buffer_ix(buf_id).unwrap();

        // buffers
        //     .insert_data(buf_id, &[[0f32, 0.0], [100.0, 0.0], [0.0, 100.0]])?;

        Ok(Self {
            config,

            labels,
            label_updates,

            rects: Vec::new(),

            pass: pass_ix,
            pipeline: pipeline_ix,

            buf_id,
            buf_ix,
        })
    }

    pub fn draw_impl(
        pass: RenderPassIx,
        pipeline: PipelineIx,
        framebuffer: FramebufferIx,
        vx_buf_ix: BufferIx,
        vertex_count: usize,
        extent: vk::Extent2D,
        device: &Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        let pass_info = vk::RenderPassBeginInfo::builder()
            .render_pass(res[pass])
            .framebuffer(res[framebuffer])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .clear_values(&[])
            .build();

        unsafe {
            device.cmd_begin_render_pass(
                cmd,
                &pass_info,
                vk::SubpassContents::INLINE,
            );

            let vx_buf = res[vx_buf_ix].buffer;
            let (pipeline, layout) = res[pipeline];
            let vxs = [vx_buf];
            // dev.cmd_bind_vertex_buffers(cmd, 0, &vxs, &[2]);
            device.cmd_bind_vertex_buffers(cmd, 0, &vxs, &[8]);
            // dev.cmd_bind_vertex_buffers(cmd, 0, &vxs, &[16]);

            let dims = [extent.width as f32, extent.height as f32];

            let constants = bytemuck::cast_slice(&dims);

            let stages =
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            device.cmd_push_constants(cmd, layout, stages, 0, constants);

            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: extent.width as f32,
                height: extent.height as f32,
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

            device.cmd_draw(cmd, vertex_count as u32, 1, 0, 0);

            device.cmd_end_render_pass(cmd);
        }

        //
    }

    pub fn draw(
        &self,
        framebuffer: FramebufferIx,
        extent: vk::Extent2D,
    ) -> Box<dyn Fn(&Device, &GpuResources, vk::CommandBuffer)> {
        let pass = self.pass;
        let pipeline = self.pipeline;
        let buf_ix = self.buf_ix;
        let vertex_count = self.rects.len() * 6;

        Box::new(move |dev, res, cmd| {
            Self::draw_impl(
                pass,
                pipeline,
                framebuffer,
                buf_ix,
                vertex_count,
                extent,
                dev,
                res,
                cmd,
            );
        })
    }
}
