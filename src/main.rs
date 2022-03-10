use crossbeam::atomic::AtomicCell;
use gfa::gfa::GFA;
use raving::script::console::frame::{FrameBuilder, Resolvable};
use raving::script::console::BatchBuilder;
use raving::vk::context::VkContext;
use raving::vk::descriptor::DescriptorLayoutInfo;
use raving::vk::{
    BatchInput, BufferIx, FrameResources, GpuResources, ShaderIx, VkEngine,
    WinSizeIndices, WinSizeResourcesBuilder,
};

use raving::vk::util::*;

use ash::{vk, Device};

use flexi_logger::{Duplicate, FileSpec, Logger};
use gpu_allocator::vulkan::Allocator;
use parking_lot::Mutex;
use rspirv_reflect::DescriptorInfo;

use waragraph::graph::{Node, Waragraph};
use waragraph::util::LabelStorage;
use waragraph::viewer::{PathViewSlot, ViewDiscrete1D};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::collections::{BTreeMap, HashMap};
use std::io::{prelude::*, BufReader};

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

struct GraphData {
    node_count: usize,
    node_lengths: Vec<usize>,

    path_names: Vec<String>,
    path_steps: Vec<Vec<u32>>,

    path_loops: Vec<Vec<u32>>,
}

fn main() -> Result<()> {
    let spec = "debug";
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

    let waragraph = Waragraph::from_gfa(&gfa)?;

    let gfa_file = std::fs::File::open(gfa_path)?;
    let reader = BufReader::new(gfa_file);

    let mut segments = Vec::new();
    let mut path_names = Vec::new();
    let mut path_steps = Vec::new();

    for line in reader.lines() {
        let line = line?;

        let mut fields = line.split("\t");

        let ty = fields.next().unwrap();

        if line.starts_with("S") {
            let id_str = fields.next().unwrap();
            let id = id_str.parse::<u32>();

            let seq = fields.next().unwrap();
            segments.push(seq.len());
            //
        } else if line.starts_with("P") {
            let name = fields.next().unwrap();

            let steps = fields.next().unwrap();
            let steps = steps.split(",");

            let steps = steps
                .map(|s| {
                    let id_str = &s[..s.len() - 1];
                    let id = id_str.parse::<u32>().unwrap();
                    id
                })
                .collect::<Vec<_>>();

            path_names.push(name.to_string());
            path_steps.push(steps);
        }
    }

    let node_count = segments.len();

    let path_loops = path_steps
        .iter()
        .map(|steps| {
            let mut nodes = vec![0; node_count];
            for &step in steps {
                let ix = step as usize;
                nodes[ix - 1] += 1;
            }
            nodes
        })
        .collect::<Vec<_>>();

    let graph_data = GraphData {
        node_count: segments.len(),
        node_lengths: segments,

        path_names,
        path_steps,
        path_loops,
    };

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

    let width = 800u32;
    let height = 600u32;

    let swapchain_dims = Arc::new(AtomicCell::new([width, height]));

    let window = WindowBuilder::new()
        .with_title("Waragraph Viewer")
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .build(&event_loop)?;

    let mut engine = VkEngine::new(&window)?;

    let mut txt = LabelStorage::new()?;

    let mut text_sub = txt.db.watch_prefix(b"t:");

    txt.allocate_label(&mut engine, "view:start")?;
    txt.allocate_label(&mut engine, "view:len")?;
    txt.allocate_label(&mut engine, "view:end")?;

    txt.set_text_for(b"view:start", "view offset")?;
    txt.set_text_for(b"view:len", "view len")?;
    txt.set_text_for(b"view:end", "view end")?;

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

    let mut win_size_resource_index = WinSizeIndices::default();

    let win_size_res_builder = move |engine: &mut VkEngine,
                                     width: u32,
                                     height: u32|
          -> Result<WinSizeResourcesBuilder> {
        let mut builder = WinSizeResourcesBuilder::default();

        let (img, view, desc_set) =
            engine.with_allocators(|ctx, res, alloc| {
                dbg!();
                let out_image = res.allocate_image(
                    ctx,
                    alloc,
                    width,
                    height,
                    vk::Format::R8G8B8A8_UNORM,
                    vk::ImageUsageFlags::STORAGE
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                    Some("out_image"),
                )?;

                dbg!();
                let out_view = res.new_image_view(ctx, &out_image)?;

                dbg!();
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
                dbg!();

                Ok((out_image, out_view, out_desc_set))
            })?;

        builder.images.insert("out_image".to_string(), img);
        builder
            .image_views
            .insert("out_image_view".to_string(), view);
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

    let mut view = ViewDiscrete1D::new(waragraph.total_len());
    view.resize(view.max() / 2);
    // view.set(0, view.max() / 2);
    let mut prev_view = view;

    let mut samples = Vec::new();

    waragraph.sample_node_lengths(width as usize, &view, &mut samples);

    let mut path_slots = engine.with_allocators(|ctx, res, alloc| {
        let slot_count = graph_data.path_loops.len();
        let mut slots = Vec::with_capacity(slot_count);

        for i in 0..slot_count {
            let path = &waragraph.paths[i];

            let name = format!("path_slot_{}", i);
            let slot = PathViewSlot::new(
                ctx,
                res,
                alloc,
                width as usize,
                Some(&name),
                |ix| {
                    let (node, _offset) = samples[ix];
                    if path.get(node.into()).is_some() {
                        1
                    } else {
                        0
                    }
                },
            )?;

            slots.push(slot);
        }

        Ok(slots)
    })?;

    let path_bufs = engine.with_allocators(|ctx, res, alloc| {
        let loc = gpu_allocator::MemoryLocation::GpuOnly;

        let path_bufs = (0..graph_data.path_loops.len())
            .map(|ix| {
                let name = format!("path_buffer_{}", graph_data.path_names[ix]);

                let path_buf = res
                    .allocate_buffer(
                        ctx,
                        alloc,
                        loc,
                        4,
                        node_count,
                        vk::BufferUsageFlags::STORAGE_BUFFER
                            | vk::BufferUsageFlags::TRANSFER_DST,
                        Some(&name),
                    )
                    .unwrap();

                res.insert_buffer(path_buf)
            })
            .collect::<Vec<_>>();

        Ok(path_bufs)
    })?;

    {
        let staging_bufs = Mutex::new(Vec::new());

        let fill_buf_batch =
            |ctx: &VkContext,
             res: &mut GpuResources,
             alloc: &mut Allocator,
             cmd: vk::CommandBuffer| {
                let mut bufs = staging_bufs.lock();

                for (ix, &path_buf) in path_bufs.iter().enumerate() {
                    let buf = &mut res[path_buf];

                    let path_data = graph_data.path_loops[ix].as_slice();

                    let staging = buf.upload_to_self_bytes(
                        ctx,
                        alloc,
                        bytemuck::cast_slice(&path_data),
                        cmd,
                    )?;

                    bufs.push(staging);
                }

                Ok(())
            };

        let batches = vec![&fill_buf_batch as &_];

        let fence = engine.submit_batches_fence_alt(batches.as_slice())?;

        engine.block_on_fence(fence)?;

        for buf in staging_bufs.into_inner() {
            buf.cleanup(&engine.context, &mut engine.allocator).ok();
        }
    }

    let out_image = *win_size_resource_index.images.get("out_image").unwrap();
    let out_view = *win_size_resource_index
        .image_views
        .get("out_image_view")
        .unwrap();
    let out_desc_set = *win_size_resource_index
        .desc_sets
        .get("out_desc_set")
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

    builder.bind_var("color_buffer", color_buffer)?;

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

    let desc_sets = engine.with_allocators(|ctx, res, alloc| {
        let mut desc_sets = Vec::new();

        for slot in path_slots.iter() {
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
            let slot_set_ix = slot.desc_set();

            let mut map = rhai::Map::default();
            map.insert("clip".into(), rhai::Dynamic::from(clip_set_ix));
            map.insert("slot".into(), rhai::Dynamic::from(slot_set_ix));
            desc_sets.push(rhai::Dynamic::from_map(map));
        }

        Ok(desc_sets)
    })?;

    let arc_module = Arc::new(builder.module.clone());

    let mut rhai_engine = raving::script::console::create_batch_engine();
    rhai_engine.register_static_module("self", arc_module.clone());

    let mut draw_foreground = rhai::Func::<
        (BatchBuilder, rhai::Array, rhai::Array, i64, i64, i64, i64),
        BatchBuilder,
    >::create_from_ast(
        rhai_engine,
        builder.ast.clone_functions_only(),
        "foreground",
    );

    {
        let mut rhai_engine = raving::script::console::create_batch_engine();

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
        let mut rhai_engine = raving::script::console::create_batch_engine();

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

    let dims = swapchain_dims.clone();
    let copy_batch = Box::new(
        move |dev: &Device,
              res: &GpuResources,
              input: &BatchInput,
              cmd: vk::CommandBuffer| {
            let [w, h] = dims.load();

            let extent = vk::Extent3D {
                width: w,
                height: h,
                depth: 1,
            };

            copy_batch(
                out_image,
                input.swapchain_image.unwrap(),
                extent,
                dev,
                res,
                cmd,
            )
        },
    ) as Box<_>;

    ///////

    let start = std::time::Instant::now();

    let mut recreate_swapchain = false;
    let mut recreate_swapchain_timer: Option<std::time::Instant> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                // while let Ok(ev) = text_sub.next_timeout(Duration::default()) {

                if view != prev_view {
                    prev_view = view;

                    {
                        let range = view.range();
                        let start = range.start.to_string();
                        let end = range.end.to_string();
                        let len = view.len().to_string();

                        txt.set_text_for(b"view:start", &start).unwrap();
                        txt.set_text_for(b"view:len", &len).unwrap();
                        txt.set_text_for(b"view:end", &end).unwrap();
                    }

                    let slot_width = path_slots[0].width();
                    samples.clear();
                    // log::warn!("resampling paths");
                    waragraph.sample_node_lengths(
                        slot_width,
                        &view,
                        &mut samples,
                    );

                    let mut buf = Vec::with_capacity(slot_width * 4);

                    for (ix, slot) in path_slots.iter_mut().enumerate() {
                        let path = &waragraph.paths[ix];

                        slot.update_from(
                            &mut engine.resources,
                            &mut buf,
                            |ix| {
                                let (node, _offset) = samples[ix];
                                if path.get(node.into()).is_some() {
                                    1
                                } else {
                                    0
                                }
                            },
                        );
                    }
                }

                while let Ok(ev) =
                    text_sub.next_timeout(Duration::from_millis(5))
                {
                    match ev {
                        sled::Event::Insert { key, value } => {
                            log::warn!("insert! {:?}, {:?}", key, value);
                            use zerocopy::FromBytes;
                            let id = u64::read_from(key[2..].as_ref()).unwrap();
                            let buf_ix =
                                txt.buffer_for_id(id).unwrap().unwrap();
                            let buffer = &mut engine.resources[buf_ix];

                            let slice = buffer.mapped_slice_mut().unwrap();

                            let len = value.len();

                            slice[0..4]
                                .clone_from_slice(&(len as u32).to_ne_bytes());

                            for (chk, &b) in
                                slice[4..].chunks_mut(4).zip(value.iter())
                            {
                                chk[0] = b;
                                chk[1] = b;
                                chk[2] = b;
                                chk[3] = b;
                            }
                        }
                        sled::Event::Remove { key } => {
                            // do nothing yet
                        }
                    }
                    //
                }

                let t = start.elapsed().as_secs_f32();

                let f_ix = engine.current_frame_number();

                let frame = &mut frames[f_ix % raving::vk::FRAME_OVERLAP];

                let batch_builder = BatchBuilder::default();

                let size = window.inner_size();

                let slot_width = path_slots[0].width();

                let label_sets = {
                    let add = |x, y, set| {
                        use rhai::Dynamic as Dyn;
                        let mut data = rhai::Map::default();
                        data.insert("x".into(), Dyn::from_int(x));
                        data.insert("y".into(), Dyn::from_int(y));
                        data.insert("desc_set".into(), Dyn::from(set));
                        Dyn::from_map(data)
                    };

                    let end_len = txt.label_len(b"view:end").unwrap() as i64;

                    let start_set =
                        txt.desc_set_for(b"view:start").unwrap().unwrap();
                    // text_storage.get_desc_set("view:start").unwrap();

                    let len_set =
                        txt.desc_set_for(b"view:len").unwrap().unwrap();

                    let end_set =
                        txt.desc_set_for(b"view:end").unwrap().unwrap();

                    let start = add(20, 16, start_set);

                    let end_x = (slot_width as i64) - (end_len * 8) - 40;

                    let len = add(end_x / 2, 16, len_set);

                    let end = add(end_x, 16, end_set);

                    // let mut data = rhai::Map::default();
                    // data.insert("x".into(), Dyn::from_int(10));
                    // data.insert("y".into(), Dyn::from_int(10));
                    // data.insert("desc_set".into(), Dyn::from(set));
                    // vec![Dyn::from_map(data)]

                    vec![start, len, end]
                };

                let fg_batch = draw_foreground(
                    batch_builder,
                    label_sets,
                    desc_sets.clone(),
                    slot_width as i64,
                    size.width as i64,
                    size.height as i64,
                    graph_data.node_count as i64,
                )
                .unwrap();
                let fg_batch_fn = fg_batch.build();
                let fg_rhai_batch = fg_batch_fn.clone();

                let fg_batch = Box::new(
                    move |dev: &Device,
                          res: &GpuResources,
                          _input: &BatchInput,
                          cmd: vk::CommandBuffer| {
                        fg_rhai_batch(dev, res, cmd);
                    },
                ) as Box<_>;

                // let batches = [&bg_batch, &fg_batch, &copy_batch];
                let batches = [&fg_batch, &copy_batch];

                let deps = vec![
                    None,
                    Some(vec![(0, vk::PipelineStageFlags::COMPUTE_SHADER)]),
                    // Some(vec![(1, vk::PipelineStageFlags::COMPUTE_SHADER)]),
                ];

                let render_success = engine
                    .draw_from_batches(frame, &batches, deps.as_slice(), 1)
                    .unwrap();

                if !render_success {
                    recreate_swapchain = true;
                }
            }
            Event::RedrawEventsCleared => {
                let should_recreate =
                    if let Some(timer) = recreate_swapchain_timer {
                        timer.elapsed().as_millis() > 15
                    } else {
                        false
                    };

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

                                    for slot in path_slots.iter_mut() {
                                        slot.resize(
                                            ctx,
                                            res,
                                            alloc,
                                            size.width as usize,
                                            0u32,
                                        )?;
                                    }

                                    Ok(())
                                })
                                .unwrap();

                            {
                                let slot_width = path_slots[0].width();
                                samples.clear();
                                // log::warn!("resampling paths");
                                waragraph.sample_node_lengths(
                                    slot_width,
                                    &view,
                                    &mut samples,
                                );

                                let mut buf =
                                    Vec::with_capacity(slot_width * 4);

                                for (ix, slot) in
                                    path_slots.iter_mut().enumerate()
                                {
                                    let path = &waragraph.paths[ix];

                                    slot.update_from(
                                        &mut engine.resources,
                                        &mut buf,
                                        |ix| {
                                            let (node, _offset) = samples[ix];
                                            if path.get(node.into()).is_some() {
                                                1
                                            } else {
                                                0
                                            }
                                        },
                                    );
                                }

                                log::warn!(
                                    "updated {} path slots",
                                    path_slots.len()
                                );
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

                            let mut rhai_engine =
                                raving::script::console::create_batch_engine();
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
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(kc) = input.virtual_keycode {
                        use VirtualKeyCode as VK;

                        let pre_len = view.len();
                        let len = view.len() as isize;

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
                            } else if matches!(kc, VK::Space) {
                                view.reset();
                            }
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
