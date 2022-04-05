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
    VkEngine, WinSizeIndices, WinSizeResourcesBuilder,
};

use raving::vk::resource::WindowResources;

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

    frame_resources: [FrameResources; 2],
    frame: FrameBuilder,

    on_resize: RhaiBatchFn2<i64, i64>,

    draw_labels: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    draw_foreground: RhaiBatchFn5<BatchBuilder, rhai::Array, i64, i64, i64>,
    copy_to_swapchain:
        RhaiBatchFn5<BatchBuilder, DescSetIx, rhai::Map, i64, i64>,
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
        let mut buffers = BufferStorage::new(&db)?;

        let mut txt = LabelStorage::new(&db)?;

        let mut text_sub = txt.tree.watch_prefix(b"t:");

        // path_v

        txt.allocate_label(&db, engine, "console")?;
        txt.set_label_pos(b"console", 4, 4)?;
        txt.set_text_for(b"console", "")?;

        txt.allocate_label(&db, engine, "fps")?;
        txt.set_label_pos(b"fps", 0, 580)?;

        txt.allocate_label(&db, engine, "view:start")?;
        txt.allocate_label(&db, engine, "view:len")?;
        txt.allocate_label(&db, engine, "view:end")?;

        txt.set_label_pos(b"view:start", 20, 16)?;
        txt.set_label_pos(b"view:len", 300, 16)?;
        txt.set_label_pos(b"view:end", 600, 16)?;

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
                .create_sampler_mean_round("loop_count")
                .unwrap(),
        );

        slot_renderer_cache.insert(
            "updater_loop_count_mid".to_string(),
            slot_renderers.create_sampler_mid("loop_count").unwrap(),
        );

        slot_renderer_cache.insert(
            "updater_has_node_mid".to_string(),
            slot_renderers.create_sampler_mid("has_node").unwrap(),
        );

        //
        let view = ViewDiscrete1D::new(waragraph.total_len());

        // let slot_count = 64;
        let slot_count = 32;

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
            txt.allocate_label(&db, engine, &name)?;
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

        let mut builder = FrameBuilder::from_script("paths.rhai")?;

        builder.bind_var("out_image", out_image)?;
        builder.bind_var("out_image_view", out_view)?;
        builder.bind_var("out_desc_set", out_desc_set)?;

        engine.with_allocators(|ctx, res, alloc| {
            builder.resolve(ctx, res, alloc)?;
            Ok(())
        })?;

        [
            ("gradient_rainbow", colorous::RAINBOW),
            ("gradient_cubehelix", colorous::CUBEHELIX),
            ("gradient_blue_purple", colorous::BLUE_PURPLE),
            ("gradient_magma", colorous::MAGMA),
        ]
        .into_iter()
        .for_each(|(n, g)| {
            create_gradient_buffer(engine, &mut buffers, &db, n, g, 256)
                .expect("error creating gradient buffers");
        });

        let arc_module = Arc::new(builder.module.clone());

        // draw_labels
        let mut rhai_engine = crate::console::create_engine(&db, &buffers);
        rhai_engine.register_static_module("self", arc_module.clone());
        let draw_labels = rhai::Func::<
            (BatchBuilder, i64, i64, rhai::Array),
            BatchBuilder,
        >::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "draw_labels",
        );

        // main draw function
        let mut rhai_engine = crate::console::create_engine(&db, &buffers);
        rhai_engine.register_static_module("self", arc_module.clone());
        let draw_foreground = rhai::Func::<
            (BatchBuilder, rhai::Array, i64, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "foreground",
        );

        let mut rhai_engine = crate::console::create_engine(&db, &buffers);
        rhai_engine.register_static_module("self", arc_module.clone());
        let copy_to_swapchain = rhai::Func::<
            (BatchBuilder, DescSetIx, rhai::Map, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "copy_to_swapchain",
        );

        // let copy_to_swapchain = Arc::new(copy_to_swapchain);

        {
            // let mut rhai_engine = raving::script::console::create_batch_engine();
            let mut rhai_engine = crate::console::create_engine(&db, &buffers);

            let arc_module = Arc::new(builder.module.clone());

            rhai_engine.register_static_module("self", arc_module.clone());

            let init = rhai::Func::<(), BatchBuilder>::create_from_ast(
                rhai_engine,
                builder.ast.clone_functions_only(),
                "init",
            );

            let mut init_builder = init()?;

            if !init_builder.init_fn.is_empty() {
                log::warn!("submitting init batches");
                let fence = engine
                    .submit_batches_fence(init_builder.init_fn.as_slice())?;

                engine.block_on_fence(fence)?;

                engine.with_allocators(|c, r, a| {
                    init_builder.free_staging_buffers(c, r, a)
                })?;
            }
        }

        let on_resize = {
            // let mut rhai_engine = raving::script::console::create_batch_engine();
            let mut rhai_engine = crate::console::create_engine(&db, &buffers);

            let arc_module = Arc::new(builder.module.clone());

            rhai_engine.register_static_module("self", arc_module.clone());

            let resize =
                rhai::Func::<(i64, i64), BatchBuilder>::create_from_ast(
                    rhai_engine,
                    builder.ast.clone_functions_only(),
                    "resize",
                );
            resize
        };

        let mut frame_resources = {
            let queue_ix = engine.queues.thread.queue_family_index;

            // hardcoded for now
            let semaphore_count = 3;
            let cmd_buf_count = 2;

            let mut new_frame = || {
                engine
                    .with_allocators(|ctx, res, _alloc| {
                        FrameResources::new(
                            ctx,
                            res,
                            queue_ix,
                            semaphore_count,
                            cmd_buf_count,
                        )
                    })
                    .unwrap()
            };
            [new_frame(), new_frame()]
        };

        Ok(Self {
            view,

            path_viewer,
            slot_renderers,

            slot_renderer_cache,

            labels: txt,
            buffers,

            frame_resources,
            frame: builder,

            on_resize,

            draw_labels,
            draw_foreground,
            copy_to_swapchain,
        })
    }

    // pub fn on_resize(&self) -> impl rhai::Func<(i64, i64), BatchBuilder> {

    // }
    // pub fn on_resize(&self) -> impl Fn(i64, i64) -> Result<BatchBuilder
}

pub fn create_gradient_buffer(
    engine: &mut VkEngine,
    buffers: &mut BufferStorage,
    db: &sled::Db,
    name: &str,
    gradient: colorous::Gradient,
    len: usize,
) -> Result<()> {
    let buf = buffers.allocate_buffer(engine, &db, name, BufFmt::FVec4, 256)?;

    let len = len.min(255);

    buffers.insert_data_from(
        buf,
        len,
        crate::util::gradient_color_fn(gradient, len),
    )?;

    Ok(())
}

pub type RhaiBatchFn1<A> = Box<
    dyn Fn(A) -> Result<BatchBuilder, Box<rhai::EvalAltResult>> + Send + Sync,
>;
pub type RhaiBatchFn2<A, B> = Box<
    dyn Fn(A, B) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn3<A, B, C> = Box<
    dyn Fn(A, B, C) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn4<A, B, C, D> = Box<
    dyn Fn(A, B, C, D) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn5<A, B, C, D, E> = Box<
    dyn Fn(A, B, C, D, E) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
