use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, DescSetIx, FrameResources, GpuResources, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn5};
use crate::graph::Waragraph;
use crate::util::{BufFmt, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::HashMap;

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

pub struct Graph3dSys {
    pub config: ConfigMap,

    // pub view: _
    pub frame_resources: [FrameResources; 2],
    pub frame: FrameBuilder,

    pub on_resize: RhaiBatchFn2<i64, i64>,

    pub draw_foreground: RhaiBatchFn5<BatchBuilder, rhai::Array, i64, i64, i64>,
    pub copy_to_swapchain:
        Arc<RhaiBatchFn5<BatchBuilder, DescSetIx, rhai::Map, i64, i64>>,
}

impl Graph3dSys {
    pub fn init(
        engine: &mut VkEngine,
        waragraph: &Arc<Waragraph>,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        window_resources: &mut WindowResources,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let mut builder =
            FrameBuilder::from_script_with("3d_viewer.rhai", |engine| {
                crate::console::register_buffer_storage(db, buffers, engine);
                crate::console::append_to_engine(db, engine);
            })?;

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
                    todo!(),
                    // "shaders/tmp.vert.spv",
                    vk::ShaderStageFlags::VERTEX,
                )?;
                let frag = res.load_shader(
                    todo!(),
                    // "shaders/tmp.frag.spv",
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

        window_resources.add_image(
            "out",
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::TRANSFER_SRC,
            [
                (vk::ImageUsageFlags::STORAGE, vk::ImageLayout::GENERAL),
                (vk::ImageUsageFlags::SAMPLED, vk::ImageLayout::GENERAL),
            ],
            Some(pass_ix),
        )?;

        todo!();
    }
}
