use crate::console::{Console, ConsoleInput};
use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use gfa::gfa::GFA;
use raving::script::console::frame::{FrameBuilder, Resolvable};
use raving::script::console::BatchBuilder;
use raving::vk::context::VkContext;
use raving::vk::descriptor::DescriptorLayoutInfo;
use raving::vk::{
    BatchInput, BufferIx, DescSetIx, FrameResources, GpuResources, ShaderIx,
    VkEngine, WinSizeIndices, WinSizeResourcesBuilder, WindowResources,
};

use raving::vk::util::*;

use ash::{vk, Device};

use flexi_logger::{Duplicate, FileSpec, Logger};
use gpu_allocator::vulkan::Allocator;
use parking_lot::Mutex;
use rspirv_reflect::DescriptorInfo;

use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufId, BufMeta, BufferStorage, LabelStorage};
use crate::viewer::{PathViewSlot, PathViewer, SlotRenderers, ViewDiscrete1D};
use sled::IVec;
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::collections::{BTreeMap, HashMap};
use std::io::{prelude::*, BufReader};

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use super::SlotUpdateFn;

pub struct ViewerSys {
    view: ViewDiscrete1D,

    path_viewer: PathViewer,
    slot_renderers: SlotRenderers,

    slot_renderer_cache: HashMap<String, SlotUpdateFn<u32>>,

    labels: LabelStorage,
    buffers: BufferStorage,

    frame: FrameBuilder,
}

impl ViewerSys {
    pub fn init(
        engine: &mut VkEngine,
        waragraph: &Arc<Waragraph>,
        db: &sled::Db,
        window_resources: &mut WindowResources,
        width: u32,
        height: u32,
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
                    "shaders/tmp.vert.spv",
                    vk::ShaderStageFlags::VERTEX,
                )?;
                let frag = res.load_shader(
                    "shaders/tmp.frag.spv",
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

        let mut slot_renderers = SlotRenderers::default();

        let graph = waragraph.clone();
        slot_renderers.register_data_source("loop_count", move |path, node| {
            let path = &graph.paths[path];
            path.get(node.into()).copied()
        });

        let graph = waragraph.clone();
        slot_renderers.register_data_source("has_node", move |path, node| {
            let path = &graph.paths[path];
            path.get(node.into()).map(|_| 1)
        });

        let mut slot_renderer_cache: HashMap<String, SlotUpdateFn<u32>> =
            HashMap::default();

        slot_renderer_cache.insert(
            "updater_loop_count_mean".to_string(),
            slot_renderers
                .create_sampler_mean_arc("loop_count")
                .unwrap(),
        );

        slot_renderer_cache.insert(
            "updater_loop_count_mid".to_string(),
            slot_renderers.create_sampler_mid_arc("loop_count").unwrap(),
        );

        slot_renderer_cache.insert(
            "updater_has_node_mid".to_string(),
            slot_renderers.create_sampler_mid_arc("has_node").unwrap(),
        );

        //
        let view = ViewDiscrete1D::new(waragraph.total_len());

        let slot_count = 20;

        let mut path_viewer = engine.with_allocators(|ctx, res, alloc| {
            PathViewer::new(
                &db,
                ctx,
                res,
                alloc,
                width as usize,
                slot_count,
                "path_slot_",
                waragraph.paths.len(),
            )
        })?;

        path_viewer.sample(&waragraph, &view);

        let mut count = 0;
        for i in path_viewer.visible_indices() {
            let name = format!("path-name-{}", i);
            txt.allocate_label(&db, &mut engine, &name)?;
            count += 1;
        }
        log::error!("added {} labels!!!", count);

        path_viewer.update_labels(&waragraph, &txt)?;

        let out_image = *window_resources.indices.images.get("out").unwrap();
        let out_view =
            *window_resources.indices.image_views.get("out").unwrap();
        let out_desc_set = *window_resources
            .indices
            .desc_sets
            .get("out")
            .and_then(|s| {
                s.get(&(
                    vk::DescriptorType::STORAGE_IMAGE,
                    vk::ImageLayout::GENERAL,
                ))
            })
            .unwrap();
        let sample_out_desc_set = *window_resources
            .indices
            .desc_sets
            .get("out")
            .and_then(|s| {
                s.get(&(
                    vk::DescriptorType::SAMPLED_IMAGE,
                    vk::ImageLayout::GENERAL,
                ))
            })
            .unwrap();

        let out_framebuffer =
            *window_resources.indices.framebuffers.get("out").unwrap();

        let mut builder = FrameBuilder::from_script("paths2.rhai")?;

        todo!();
    }
}
