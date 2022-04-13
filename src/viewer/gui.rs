use bstr::ByteSlice;
use parking_lot::RwLock;
use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, DescSetIx, FrameResources, GpuResources, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::FxHashMap;
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};
use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

pub struct GuiSys {
    pub config: ConfigMap,

    pub labels: LabelStorage,
    pub label_updates: sled::Subscriber,

    pub rhai_module: Arc<rhai::Module>,

    pub on_resize: RhaiBatchFn2<i64, i64>,

    pub draw_labels: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    pub draw_shapes: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
}

impl GuiSys {
    pub fn init(
        engine: &mut VkEngine,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        window_resources: &mut WindowResources,
        width: u32,
        height: u32,
        // height: u32,
    ) -> Result<Self> {
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

        todo!();
    }
}
