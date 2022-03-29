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
use waragraph::console::{Console, ConsoleInput};

use raving::vk::util::*;

use ash::{vk, Device};

use flexi_logger::{Duplicate, FileSpec, Logger};
use gpu_allocator::vulkan::Allocator;
use parking_lot::Mutex;
use rspirv_reflect::DescriptorInfo;

use sled::IVec;
use waragraph::graph::{Node, Waragraph};
use waragraph::util::{BufFmt, BufId, BufMeta, BufferStorage, LabelStorage};
use waragraph::viewer::{
    PathViewSlot, PathViewer, SlotRenderers, ViewDiscrete1D,
};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::collections::{BTreeMap, HashMap};
use std::io::{prelude::*, BufReader};

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

fn main() -> Result<()> {
    // disable sled logging
    let spec = "debug, sled=info";
    // let spec = "debug";
    let _logger = Logger::try_with_env_or_str(spec)?
        .log_to_file(FileSpec::default())
        .duplicate_to_stderr(Duplicate::Debug)
        .start()?;

    let mut args = std::env::args();

    let _ = args.next().unwrap();

    let gfa_path = args.next().ok_or(anyhow!("Provide a GFA path"))?;

    let gfa = {
        let parser = gfa::parser::GFAParser::default();
        let gfa: GFA<usize, ()> = parser.parse_file(&gfa_path)?;
        gfa
    };

    let db_cfg = sled::Config::default()
        .temporary(true)
        .flush_every_ms(Some(10_000));
    // .path("waragraph_viewer")
    // .flush_every_ms(Some(10_000)); // probably don't even need every 10s

    let db = db_cfg.open()?;

    db.insert(b"slot_function", b"loops_mean")?;
    // db.insert(b"slot_function", b"loops_midpoint")?;
    // db.insert(b"slot_function", b"node_present")?;

    // make sure the first frame gets resampled
    db.remove(b"sample_indices")?;

    let waragraph = Arc::new(Waragraph::from_gfa(&gfa)?);

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

    let event_loop: EventLoop<()>;

    #[cfg(target_os = "linux")]
    {
        use winit::platform::unix::EventLoopExtUnix;
        log::debug!("Using X11 event loop");
        event_loop = EventLoop::new_x11()?;
    }

    #[cfg(not(target_os = "linux"))]
    {
        log::debug!("Using default event loop");
        event_loop = EventLoop::new();
    }

    // let event_loop = EventLoop::new();

    let mut console = Console::default();

    let width = 800u32;
    let height = 600u32;

    let swapchain_dims = Arc::new(AtomicCell::new([width, height]));

    let window = WindowBuilder::new()
        .with_title("Waragraph Viewer")
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .build(&event_loop)?;

    let mut engine = VkEngine::new(&window)?;

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

    let mut buffers = BufferStorage::new(&db)?;

    let fmt = BufFmt::FVec4;
    let buf_0 =
        buffers.allocate_buffer(&mut engine, &db, "storage_0", fmt, 255)?;

    let rgb = |r: f32, g: f32, b: f32| [r, g, b, 1.0];

    buffers.insert_data(buf_0, &[rgb(0.0, 0.0, 0.0), rgb(0.8, 0.0, 0.0)])?;

    let mut txt = LabelStorage::new(&db)?;

    let mut text_sub = txt.tree.watch_prefix(b"t:");

    // path_v

    txt.allocate_label(&db, &mut engine, "console")?;
    txt.set_label_pos(b"console", 4, 4)?;
    txt.set_text_for(b"console", "")?;

    txt.allocate_label(&db, &mut engine, "fps")?;
    txt.set_label_pos(b"fps", 0, 580)?;

    txt.allocate_label(&db, &mut engine, "view:start")?;
    txt.allocate_label(&db, &mut engine, "view:len")?;
    txt.allocate_label(&db, &mut engine, "view:end")?;

    txt.set_label_pos(b"view:start", 20, 16)?;
    txt.set_label_pos(b"view:len", 300, 16)?;
    txt.set_label_pos(b"view:end", 600, 16)?;

    let window_storage_set_info = {
        let info = DescriptorInfo {
            ty: rspirv_reflect::DescriptorType::STORAGE_IMAGE,
            binding_count: rspirv_reflect::BindingCount::One,
            name: "out_image".to_string(),
        };

        Some((0u32, info)).into_iter().collect::<BTreeMap<_, _>>()
    };

    let window_storage_image_layout = {
        let mut info = DescriptorLayoutInfo::default();

        let binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .stage_flags(vk::ShaderStageFlags::COMPUTE) // TODO should also be graphics
            .build();

        info.bindings.push(binding);
        info
    };

    let window_texture_set_info = {
        let info = DescriptorInfo {
            ty: rspirv_reflect::DescriptorType::SAMPLED_IMAGE,
            binding_count: rspirv_reflect::BindingCount::One,
            name: "out_image".to_string(),
        };

        Some((0u32, info)).into_iter().collect::<BTreeMap<_, _>>()
    };

    let window_texture_layout = {
        let mut info = DescriptorLayoutInfo::default();

        let binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .stage_flags(vk::ShaderStageFlags::COMPUTE) // TODO should also be graphics
            .build();

        info.bindings.push(binding);
        info
    };

    let mut win_size_resource_index = WinSizeIndices::default();

    let win_size_res_builder = move |engine: &mut VkEngine,
                                     width: u32,
                                     height: u32|
          -> Result<WinSizeResourcesBuilder> {
        let mut builder = WinSizeResourcesBuilder::default();

        let (img, view, sampled_desc_set, desc_set) =
            engine.with_allocators(|ctx, res, alloc| {
                let out_image = res.allocate_image(
                    ctx,
                    alloc,
                    width,
                    height,
                    vk::Format::R8G8B8A8_UNORM,
                    vk::ImageUsageFlags::STORAGE
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                    Some("out_image"),
                )?;

                let out_view = res.new_image_view(ctx, &out_image)?;

                let sampled_desc_set = res.allocate_desc_set_raw(
                    &window_texture_layout,
                    &window_texture_set_info,
                    |res, builder| {
                        let info = ash::vk::DescriptorImageInfo::builder()
                            .image_layout(vk::ImageLayout::GENERAL)
                            .image_view(out_view)
                            .build();

                        builder.bind_image(0, &[info]);

                        Ok(())
                    },
                )?;

                let out_desc_set = res.allocate_desc_set_raw(
                    &window_storage_image_layout,
                    &window_storage_set_info,
                    |res, builder| {
                        let info = ash::vk::DescriptorImageInfo::builder()
                            .image_layout(vk::ImageLayout::GENERAL)
                            .image_view(out_view)
                            .build();

                        builder.bind_image(0, &[info]);

                        Ok(())
                    },
                )?;

                Ok((out_image, out_view, sampled_desc_set, out_desc_set))
            })?;

        builder.images.insert("out_image".to_string(), img);
        builder
            .image_views
            .insert("out_image_view".to_string(), view);
        builder
            .desc_sets
            .insert("sampled_desc_set".to_string(), sampled_desc_set);
        builder
            .desc_sets
            .insert("out_desc_set".to_string(), desc_set);

        Ok(builder)
    };

    {
        let size = window.inner_size();
        let builder =
            win_size_res_builder(&mut engine, size.width, size.height)?;
        engine.with_allocators(|ctx, res, alloc| {
            builder.insert(&mut win_size_resource_index, ctx, res, alloc)?;
            Ok(())
        })?;
    }

    let mut view = if let Some(old_view) = db
        .get(b"view")?
        .and_then(|bytes| ViewDiscrete1D::from_bytes(&bytes))
    {
        if old_view.max == waragraph.total_len() {
            old_view
        } else {
            let mut view = ViewDiscrete1D::new(waragraph.total_len());
            view.resize(view.max() / 2);
            view
        }
    } else {
        let mut view = ViewDiscrete1D::new(waragraph.total_len());
        // view.resize(view.max() / 2);
        view
    };

    // let mut view = ViewDiscrete1D::new(waragraph.total_len());
    // view.resize(view.max() / 2);
    // view.set(0, view.max() / 2);
    let mut prev_view = None;

    db.insert(b"view", &view.as_bytes())?;

    let mut view_sub = db.watch_prefix(b"view");

    // TODO set slot_count based on available height, and row height
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

    let updater_loop_count_mean = slot_renderers
        .create_sampler_mean_arc("loop_count")
        .unwrap();
    let updater_loop_count_mid =
        slot_renderers.create_sampler_mid_arc("loop_count").unwrap();
    let updater_has_node_mid =
        slot_renderers.create_sampler_mid_arc("has_node").unwrap();

    path_viewer.sample(&waragraph, &view);

    let mut count = 0;
    for i in path_viewer.visible_indices() {
        let name = format!("path-name-{}", i);
        txt.allocate_label(&db, &mut engine, &name)?;
        count += 1;
    }
    log::error!("added {} labels!!!", count);

    path_viewer.update_labels(&waragraph, &txt)?;

    let out_image = *win_size_resource_index.images.get("out_image").unwrap();
    let out_view = *win_size_resource_index
        .image_views
        .get("out_image_view")
        .unwrap();
    let out_desc_set = *win_size_resource_index
        .desc_sets
        .get("out_desc_set")
        .unwrap();
    let sample_out_desc_set = *win_size_resource_index
        .desc_sets
        .get("sampled_desc_set")
        .unwrap();

    // let mut builder = FrameBuilder::from_script("paths.rhai")?;
    let mut builder = FrameBuilder::from_script("paths2.rhai")?;

    builder.bind_var("out_image", out_image)?;
    builder.bind_var("out_image_view", out_view)?;
    builder.bind_var("out_desc_set", out_desc_set)?;

    #[rustfmt::skip]
    let color_buffer = {
        let usage = vk::BufferUsageFlags::TRANSFER_DST
            | vk::BufferUsageFlags::STORAGE_BUFFER;

        let gradient = colorous::RAINBOW;

        let l = 4;

        waragraph::util::alloc_buffer_with(
            &mut engine,
            Some("color_buffer"),
            usage,
            true,
            0..l,
            |ix| {
                let color = gradient.eval_rational(ix, l);

                let to_bytes = |c| ((c as f32) / 255.0).to_ne_bytes();

                let r = to_bytes(color.r);
                let g = to_bytes(color.g);
                let b = to_bytes(color.b);
                let a = 1.0f32.to_ne_bytes();

                [
                    r[0], r[1], r[2], r[3],
                    g[0], g[1], g[2], g[3],
                    b[0], b[1], b[2], b[3],
                    a[0], a[1], a[2], a[3],
                ]
            },
        )?
    };

    // let color_buffer = buffers.buffers[0];
    builder.bind_var("color_buffer", color_buffer)?;
    builder.bind_var("alt_color_desc_set", buffers.desc_sets.read()[0])?;

    let fmt = BufFmt::UVec4;
    let line_buf =
        buffers.allocate_buffer(&mut engine, &db, "line_storage", fmt, 64)?;
    log::error!("line_storage -> {:?}", line_buf);

    let line = |x0: u32, y0: u32, x1: u32, y1: u32| [x0, y0, x1, y1];

    let mut lines = vec![];

    let n = 24;

    for i in 0..n {
        use std::f32::consts::TAU;
        let angle_d = TAU / (n as f32);
        let ti = (i as f32) * angle_d;
        let ti_2 = ((i + 1) as f32) * angle_d;

        let radius = 200.0;
        let o = 300.0;

        let x0 = o + ti.cos() * radius;
        let y0 = o + ti.sin() * radius;
        let x1 = o + ti_2.cos() * radius;
        let y1 = o + ti_2.sin() * radius;

        lines.push(line(x0 as u32, y0 as u32, x1 as u32, y1 as u32));
    }

    buffers.insert_data(
        line_buf,
        &lines,
        // &[line(200, 300, 500, 300), line(100, 100, 500, 400)],
    )?;

    engine.with_allocators(|ctx, res, alloc| {
        builder.resolve(ctx, res, alloc)?;
        Ok(())
    })?;
    log::warn!("is resolved: {}", builder.is_resolved());

    let clip_rects_buffer = builder
        .module
        .get_var_value::<Resolvable<BufferIx>>("clip_rects_buffer")
        .unwrap()
        .get_unwrap();

    let shader_ix = builder
        .module
        .get_var_value::<Resolvable<ShaderIx>>("path_shader")
        .unwrap();

    let shader_ix = shader_ix.get_unwrap();

    const MAX_SLOTS: usize = 256;

    let clip_desc_sets = engine.with_allocators(|ctx, res, alloc| {
        let mut desc_sets = Vec::new();

        for _ in 0..MAX_SLOTS {
            let clip_set =
                res.allocate_desc_set(shader_ix, 0, |res, builder| {
                    let buffer = &res[clip_rects_buffer];
                    let buf_info = ash::vk::DescriptorBufferInfo::builder()
                        .buffer(buffer.buffer)
                        .offset(0)
                        .range(ash::vk::WHOLE_SIZE)
                        .build();
                    let buffer_info = [buf_info];
                    builder.bind_buffer(0, &buffer_info);
                    Ok(())
                })?;

            let clip_set_ix = res.insert_desc_set(clip_set);
            desc_sets.push(clip_set_ix);
        }

        Ok(desc_sets)
    })?;

    let arc_module = Arc::new(builder.module.clone());

    // draw_labels
    let mut rhai_engine = waragraph::console::create_engine(&db, &buffers);

    rhai_engine.register_static_module("self", arc_module.clone());

    let mut draw_labels = rhai::Func::<
        (BatchBuilder, i64, i64, rhai::Array),
        BatchBuilder,
    >::create_from_ast(
        rhai_engine,
        builder.ast.clone_functions_only(),
        "draw_labels",
    );

    // let mut rhai_engine = raving::script::console::create_batch_engine();
    let mut rhai_engine = waragraph::console::create_engine(&db, &buffers);

    rhai_engine.register_static_module("self", arc_module.clone());

    let mut draw_foreground = rhai::Func::<
        (BatchBuilder, rhai::Array, rhai::Array, i64, i64, i64),
        BatchBuilder,
    >::create_from_ast(
        rhai_engine,
        builder.ast.clone_functions_only(),
        "foreground",
    );

    // let mut rhai_engine = raving::script::console::create_batch_engine();
    let mut rhai_engine = waragraph::console::create_engine(&db, &buffers);
    rhai_engine.register_static_module("self", arc_module.clone());

    let copy_to_swapchain = rhai::Func::<
        (BatchBuilder, DescSetIx, rhai::Map, i64, i64),
        BatchBuilder,
    >::create_from_ast(
        rhai_engine,
        builder.ast.clone_functions_only(),
        "copy_to_swapchain",
    );

    let copy_to_swapchain = Arc::new(copy_to_swapchain);

    {
        // let mut rhai_engine = raving::script::console::create_batch_engine();
        let mut rhai_engine = waragraph::console::create_engine(&db, &buffers);

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
            let fence =
                engine.submit_batches_fence(init_builder.init_fn.as_slice())?;

            engine.block_on_fence(fence)?;

            engine.with_allocators(|c, r, a| {
                init_builder.free_staging_buffers(c, r, a)
            })?;
        }
    }

    let update_clip_rects = {
        // let mut rhai_engine = raving::script::console::create_batch_engine();
        let mut rhai_engine = waragraph::console::create_engine(&db, &buffers);

        let arc_module = Arc::new(builder.module.clone());

        rhai_engine.register_static_module("self", arc_module.clone());

        let resize = rhai::Func::<(i64, i64), BatchBuilder>::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "resize",
        );
        resize
    };

    let mut frames = {
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

    ///////

    let start = std::time::Instant::now();

    let mut recreate_swapchain = false;
    let mut recreate_swapchain_timer: Option<std::time::Instant> = None;

    let mut prev_frame_end = std::time::Instant::now();

    let mut desc_sets = Vec::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                let frame_start = std::time::Instant::now();

                buffers.allocate_queued(&mut engine).unwrap();

                buffers.fill_updated_buffers(&mut engine.resources).unwrap();

                if Some(view) != prev_view || path_viewer.should_update() {
                    prev_view = Some(view);

                    {
                        let range = view.range();
                        let start = range.start.to_string();
                        let end = range.end.to_string();
                        let len = view.len().to_string();

                        txt.set_text_for(b"view:start", &start).unwrap();
                        txt.set_text_for(b"view:len", &len).unwrap();
                        txt.set_text_for(b"view:end", &end).unwrap();
                    }

                    path_viewer.sample(&waragraph, &view);
                }

                while let Ok(ev) =
                    view_sub.next_timeout(Duration::from_micros(10))
                {
                    match ev {
                        sled::Event::Insert { key, value } => {
                            if let Some(new_view) =
                                ViewDiscrete1D::from_bytes(value.as_ref())
                            {
                                view = new_view;
                            }
                        }
                        _ => (),
                    }
                }

                path_viewer.update_labels(&waragraph, &txt).unwrap();

                if path_viewer.has_new_samples() {
                    let update_key = db.get(b"slot_function").ok().flatten();

                    let updater = if let Some(key) = update_key {
                        match key.as_ref() {
                            b"loops_mean" => &updater_loop_count_mean,
                            b"loops_midpoint" => &updater_loop_count_mid,
                            b"node_present" => &updater_has_node_mid,
                            _ => {
                                log::warn!("unknown slot_function");
                                &updater_has_node_mid
                            }
                        }
                    } else {
                        log::warn!("missing slot_function");
                        &updater_has_node_mid
                    };

                    path_viewer.update_from_alt(&mut engine.resources, updater);
                }

                let mut updates: HashMap<IVec, IVec> = HashMap::default();

                while let Ok(ev) =
                    text_sub.next_timeout(Duration::from_micros(10))
                {
                    match ev {
                        sled::Event::Insert { key, value } => {
                            updates.insert(key, value);
                        }
                        sled::Event::Remove { key } => {
                            // do nothing yet
                        }
                    }
                    //
                }

                for (key, value) in updates {
                    let id = u64::read_from(key[2..].as_ref()).unwrap();
                    let buf_ix = txt.buffer_for_id(id).unwrap().unwrap();
                    let buffer = &mut engine.resources[buf_ix];
                    let slice = buffer.mapped_slice_mut().unwrap();
                    let len = value.len();

                    slice[0..4].clone_from_slice(&(len as u32).to_ne_bytes());

                    slice[4..]
                        .chunks_mut(4)
                        .zip(value.iter())
                        .for_each(|(chk, &b)| chk.fill(b));
                }

                let t = start.elapsed().as_secs_f32();

                let f_ix = engine.current_frame_number();

                let frame = &mut frames[f_ix % raving::vk::FRAME_OVERLAP];

                let size = window.inner_size();

                let slot_width = path_viewer.width;

                let label_sets = {
                    txt.label_names
                        .values()
                        .map(|&id| {
                            use rhai::Dynamic as Dyn;
                            let mut data = rhai::Map::default();
                            let set = txt.desc_set_for_id(id).unwrap().unwrap();
                            let (x, y) = txt.get_label_pos_id(id).unwrap();
                            data.insert("x".into(), Dyn::from_int(x as i64));
                            data.insert("y".into(), Dyn::from_int(y as i64));
                            data.insert("desc_set".into(), Dyn::from(set));
                            Dyn::from_map(data)
                        })
                        .collect::<Vec<_>>()
                };

                desc_sets.clear();
                desc_sets.extend(
                    path_viewer.slots.iter().zip(clip_desc_sets.iter()).map(
                        |(slot, clip)| {
                            let slot_set_ix = slot.desc_set();
                            let mut map = rhai::Map::default();
                            map.insert(
                                "clip".into(),
                                rhai::Dynamic::from(*clip),
                            );
                            map.insert(
                                "slot".into(),
                                rhai::Dynamic::from(slot_set_ix),
                            );
                            rhai::Dynamic::from_map(map)
                            // desc_sets.push(rhai::Dynamic::from_map(map));
                        },
                    ),
                );

                let batch_builder = BatchBuilder::default();
                let fg_batch = draw_foreground(
                    batch_builder,
                    label_sets.clone(),
                    desc_sets.clone(),
                    slot_width as i64,
                    size.width as i64,
                    size.height as i64,
                )
                .unwrap();
                let fg_batch_fn = fg_batch.build();

                let batch_builder = BatchBuilder::default();
                let labels_batch = draw_labels(
                    batch_builder,
                    size.width as i64,
                    size.height as i64,
                    label_sets,
                )
                .unwrap();
                let labels_batch_fn = labels_batch.build();

                let fg_batch = Box::new(
                    move |dev: &Device,
                          res: &GpuResources,
                          _input: &BatchInput,
                          cmd: vk::CommandBuffer| {
                        fg_batch_fn(dev, res, cmd);
                        labels_batch_fn(dev, res, cmd);
                    },
                ) as Box<_>;

                let copy_to_swapchain = copy_to_swapchain.clone();

                let copy_swapchain_batch = Box::new(
                    move |dev: &Device,
                          res: &GpuResources,
                          input: &BatchInput,
                          cmd: vk::CommandBuffer| {
                        let mut cp_swapchain = rhai::Map::default();

                        cp_swapchain.insert(
                            "storage_set".into(),
                            rhai::Dynamic::from(input.storage_set.unwrap()),
                        );

                        cp_swapchain.insert(
                            "img".into(),
                            rhai::Dynamic::from(input.swapchain_image.unwrap()),
                        );

                        let batch_builder = BatchBuilder::default();

                        let batch = copy_to_swapchain(
                            batch_builder,
                            sample_out_desc_set,
                            cp_swapchain,
                            size.width as i64,
                            size.height as i64,
                        );

                        if let Err(e) = &batch {
                            log::error!("copy_to_swapchain error: {:?}", e);
                        }

                        let batch = batch.unwrap();
                        let batch_fn = batch.build();
                        batch_fn(dev, res, cmd)
                    },
                ) as Box<_>;

                let batches = [&fg_batch, &copy_swapchain_batch];

                let deps = vec![
                    None,
                    Some(vec![(0, vk::PipelineStageFlags::COMPUTE_SHADER)]),
                    // Some(vec![(1, vk::PipelineStageFlags::COMPUTE_SHADER)]),
                ];

                if recreate_swapchain_timer.is_none() && !recreate_swapchain {
                    let render_success = engine
                        .draw_from_batches(frame, &batches, deps.as_slice(), 1)
                        .unwrap();

                    if !render_success {
                        recreate_swapchain = true;
                    }

                    let ft = prev_frame_end.elapsed().as_secs_f64();
                    let fps = (1.0 / ft) as u32;
                    txt.set_text_for(b"fps", &fps.to_string()).unwrap();
                    prev_frame_end = std::time::Instant::now();
                }
            }
            Event::RedrawEventsCleared => {
                let should_recreate = recreate_swapchain_timer
                    .map(|t| t.elapsed().as_millis() > 50)
                    .unwrap_or_default();

                if should_recreate || recreate_swapchain {
                    recreate_swapchain = false;

                    let size = window.inner_size();

                    if size.width == 0 || size.height == 0 {
                        recreate_swapchain_timer =
                            Some(std::time::Instant::now());
                    } else {
                        log::debug!(
                            "Recreating swapchain with window size {:?}",
                            size
                        );

                        engine
                            .recreate_swapchain(Some([size.width, size.height]))
                            .unwrap();

                        swapchain_dims.store(engine.swapchain_dimensions());

                        {
                            let res_builder = win_size_res_builder(
                                &mut engine,
                                size.width,
                                size.height,
                            )
                            .unwrap();

                            engine
                                .with_allocators(|ctx, res, alloc| {
                                    res_builder.insert(
                                        &mut win_size_resource_index,
                                        ctx,
                                        res,
                                        alloc,
                                    )?;

                                    path_viewer.resize(
                                        ctx,
                                        res,
                                        alloc,
                                        size.width as usize,
                                        0u32,
                                    )?;

                                    Ok(())
                                })
                                .unwrap();

                            {
                                let slot_width = path_viewer.width;

                                // txt.set_label_pos(b"view:start", 20, 16)?;
                                let len_len =
                                    txt.label_len(b"view:len").unwrap();
                                let end_len =
                                    txt.label_len(b"view:end").unwrap();
                                let end_label_x =
                                    slot_width - (end_len * 8) - 40;
                                let len_label_x =
                                    (end_label_x / 2) - len_len / 2;
                                txt.set_label_pos(
                                    b"view:len",
                                    len_label_x as u32,
                                    16,
                                )
                                .unwrap();
                                txt.set_label_pos(
                                    b"view:end",
                                    end_label_x as u32,
                                    16,
                                )
                                .unwrap();

                                txt.set_label_pos(
                                    b"fps",
                                    0,
                                    (size.height - 12) as u32,
                                )
                                .unwrap();

                                path_viewer.sample(&waragraph, &view);

                                // waragraph.sample_node_lengths_db(
                                //     slot_width,
                                //     &view,
                                //     &mut samples_db,
                                // );

                                // db.update_and_fetch("sample_indices", |_| {
                                //     Some(samples_db.as_bytes())
                                // })
                                // .unwrap();
                            }

                            {
                                let mut init_builder = update_clip_rects(
                                    size.width as i64,
                                    size.height as i64,
                                )
                                .unwrap();

                                if !init_builder.init_fn.is_empty() {
                                    log::warn!("submitting update batches");
                                    let fence = engine
                                        .submit_batches_fence(
                                            init_builder.init_fn.as_slice(),
                                        )
                                        .unwrap();

                                    engine.block_on_fence(fence).unwrap();

                                    engine
                                        .with_allocators(|c, r, a| {
                                            init_builder
                                                .free_staging_buffers(c, r, a)
                                        })
                                        .unwrap();
                                }
                            }

                            // let mut rhai_engine =
                            //     raving::script::console::create_batch_engine();
                            let mut rhai_engine =
                                waragraph::console::create_engine(
                                    &db, &buffers,
                                );
                            rhai_engine.register_static_module(
                                "self",
                                arc_module.clone(),
                            );

                            draw_foreground = rhai::Func::<
                                (
                                    BatchBuilder,
                                    rhai::Array,
                                    rhai::Array,
                                    i64,
                                    i64,
                                    i64,
                                ),
                                BatchBuilder,
                            >::create_from_ast(
                                rhai_engine,
                                builder.ast.clone_functions_only(),
                                "foreground",
                            );
                        }

                        recreate_swapchain_timer = None;
                    }
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::ReceivedCharacter(c) => {
                    // if !(c.is_ascii_control() && c.is_ascii_whitespace()) {
                    if !c.is_ascii_control() {
                        console
                            .handle_input(
                                &db,
                                &buffers,
                                &txt,
                                ConsoleInput::AppendChar(c),
                            )
                            .unwrap();
                    }
                }
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(kc) = input.virtual_keycode {
                        use VirtualKeyCode as VK;

                        let mut view = view;

                        let pre_len = view.len();
                        let len = view.len() as isize;

                        let mut update = true;

                        if input.state == winit::event::ElementState::Pressed {
                            if matches!(kc, VK::Left) {
                                view.translate(-len / 10);
                                assert_eq!(pre_len, view.len());
                            } else if matches!(kc, VK::Right) {
                                view.translate(len / 10);
                                assert_eq!(pre_len, view.len());
                            } else if matches!(kc, VK::Up) {
                                view.resize((len - len / 9) as usize);
                            } else if matches!(kc, VK::Down) {
                                view.resize((len + len / 10) as usize);
                            } else if matches!(kc, VK::Escape) {
                                // buffers.insert_data(buf_0, &[rgb(0.0, 0.0, 0.0), rgb(1.0, 0.0, 0.0)]).unwrap();
                                view.reset();
                            } else if matches!(kc, VK::PageUp) {
                                path_viewer.scroll_up();
                            } else if matches!(kc, VK::PageDown) {
                                path_viewer.scroll_down();
                            } else if matches!(kc, VK::Return) {
                                update = false;
                                console
                                    .handle_input(
                                        &db,
                                        &buffers,
                                        &txt,
                                        ConsoleInput::Submit,
                                    )
                                    .unwrap();
                            } else if matches!(kc, VK::Back) {
                                update = false;
                                console
                                    .handle_input(
                                        &db,
                                        &buffers,
                                        &txt,
                                        ConsoleInput::Backspace,
                                    )
                                    .unwrap();
                            }
                            /*
                            } else if matches!(kc, VK::PageUp) {
                                // a temporary lil hack
                                update = false;
                                waragraph::console::eval::<()>(
                                    &db,
                                    "set_view_offset(0)",
                                )
                                .unwrap();
                            } else if matches!(kc, VK::PageDown) {
                                let offset = view.max() - view.len();
                                view.set(offset, len as usize);
                            }
                                */
                        }

                        if update {
                            let view_bytes = view.as_bytes();
                            db.update_and_fetch(b"view", |_| Some(&view_bytes))
                                .unwrap();
                        }
                    }
                    //
                }
                WindowEvent::CloseRequested => {
                    log::debug!("WindowEvent::CloseRequested");
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                WindowEvent::Resized { .. } => {
                    recreate_swapchain_timer = Some(std::time::Instant::now());
                }
                _ => (),
            },
            Event::LoopDestroyed => {
                log::debug!("Event::LoopDestroyed");

                unsafe {
                    let queue = engine.queues.thread.queue;
                    engine.context.device().queue_wait_idle(queue).unwrap();
                };

                let ctx = &engine.context;
                let res = &mut engine.resources;
                let alloc = &mut engine.allocator;

                res.cleanup(ctx, alloc).unwrap();
            }
            _ => (),
        }
    });

    Ok(())
}
