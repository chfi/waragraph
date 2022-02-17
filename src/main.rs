use crossbeam::atomic::AtomicCell;
use engine::script::console::frame::FrameBuilder;
use engine::script::console::BatchBuilder;
use engine::vk::context::VkContext;
use engine::vk::{BatchInput, FrameResources, GpuResources, VkEngine};

use engine::vk::util::*;

use ash::{vk, Device};

use flexi_logger::{Duplicate, FileSpec, Logger};
use gpu_allocator::vulkan::Allocator;
use rustc_hash::FxHashSet;
use winit::event::{Event, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

// #[cfg(target_os = "linux")]
// use winit::platform::unix::*;

use std::collections::HashMap;
use std::io::{prelude::*, BufReader};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

struct GraphData {
    node_count: usize,
    node_lengths: Vec<usize>,

    path_names: Vec<String>,
    path_steps: Vec<Vec<u32>>,

    path_loops: Vec<Vec<u32>>,
}

fn main() -> Result<()> {
    let mut args = std::env::args();

    let _ = args.next().unwrap();

    let gfa_path = args.next().ok_or(anyhow!("Provide a GFA path"))?;

    let spec = "debug";
    let _logger = Logger::try_with_env_or_str(spec)?
        .log_to_file(FileSpec::default())
        .duplicate_to_stderr(Duplicate::Debug)
        .start()?;

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

    let event_loop = EventLoop::new();

    let width = 800;
    let height = 600;

    let window = WindowBuilder::new()
        .with_title("Waragraph Viewer")
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .build(&event_loop)?;

    let mut engine = VkEngine::new(&window)?;

    let (out_image, out_view, path_buf) =
        engine.with_allocators(|ctx, res, alloc| {
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

            let out_view = res.create_image_view_for_image(ctx, out_image)?;

            let loc = gpu_allocator::MemoryLocation::GpuOnly;
            let path_buf = res.allocate_buffer(
                ctx,
                alloc,
                loc,
                4,
                node_count,
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                Some("path_buffer"),
            )?;

            Ok((out_image, out_view, path_buf))
        })?;

    // let path_buffer =

    {
        let path_data = graph_data.path_loops[1].clone();
        println!("uploading path_data: {:#?}", path_data);

        let staging_buf = Arc::new(AtomicCell::new(None));

        let inner = staging_buf.clone();

        let fill_buf_batch =
            move |ctx: &VkContext,
                  res: &mut GpuResources,
                  alloc: &mut Allocator,
                  cmd: vk::CommandBuffer| {
                let buf = &mut res[path_buf];

                let staging = buf.upload_to_self_bytes(
                    ctx,
                    alloc,
                    bytemuck::cast_slice(&path_data),
                    cmd,
                )?;

                inner.store(Some(staging));

                Ok(())
            };

        let batches = vec![Arc::new(fill_buf_batch) as Arc<_>];

        let fence = engine.submit_batches_fence(batches.as_slice())?;

        engine.block_on_fence(fence)?;

        staging_buf.take().and_then(|buf| {
            buf.cleanup(&engine.context, &mut engine.allocator).ok()
        });
    }

    let mut builder = FrameBuilder::from_script("paths.rhai")?;

    builder.bind_var("out_image", out_image)?;
    builder.bind_var("out_image_view", out_view)?;

    builder.bind_var("path_0", path_buf)?;

    engine.with_allocators(|ctx, res, alloc| {
        builder.resolve(ctx, res, alloc)?;
        Ok(())
    })?;
    log::warn!("is resolved: {}", builder.is_resolved());

    let arc_module = Arc::new(builder.module.clone());

    let mut rhai_engine = engine::script::console::create_batch_engine();
    rhai_engine.register_static_module("self", arc_module.clone());

    let draw_foreground =
        rhai::Func::<(i64, i64, i64), BatchBuilder>::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "foreground",
        );

    {
        let mut rhai_engine = engine::script::console::create_batch_engine();

        let arc_module = Arc::new(builder.module.clone());

        rhai_engine.register_static_module("self", arc_module.clone());

        let init = rhai::Func::<(), BatchBuilder>::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "init",
        );

        let init_builder = init()?;

        if !init_builder.init_fn.is_empty() {
            log::warn!("submitting init batches");
            let fence =
                engine.submit_batches_fence(init_builder.init_fn.as_slice())?;

            engine.block_on_fence(fence)?;
        }
    }

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

    let copy_batch = Box::new(
        move |dev: &Device,
              res: &GpuResources,
              input: &BatchInput,
              cmd: vk::CommandBuffer| {
            copy_batch(out_image, input.swapchain_image.unwrap(), dev, res, cmd)
        },
    ) as Box<_>;

    let start = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        let mut _dirty_swapchain = false;

        match event {
            Event::MainEventsCleared => {
                let t = start.elapsed().as_secs_f32();

                let f_ix = engine.current_frame_number();
                // dbg!(t);
                // dbg!(f_ix);
                let frame = &mut frames[f_ix % engine::vk::FRAME_OVERLAP];

                // let bg_batch = draw_background(800, 600, t).unwrap();
                // let bg_batch_fn = bg_batch.build();
                // let bg_rhai_batch = bg_batch_fn.clone();

                let fg_batch =
                    draw_foreground(800, 600, graph_data.node_count as i64)
                        .unwrap();
                let fg_batch_fn = fg_batch.build();
                let fg_rhai_batch = fg_batch_fn.clone();

                // let bg_batch = Box::new(
                //     move |dev: &Device,
                //           res: &GpuResources,
                //           _input: &BatchInput,
                //           cmd: vk::CommandBuffer| {
                //         bg_rhai_batch(dev, res, cmd);
                //     },
                // ) as Box<_>;

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

                // dbg!();
                let render_success = engine
                    .draw_from_batches(frame, &batches, deps.as_slice(), 1)
                    .unwrap();

                // dbg!();

                if !render_success {
                    dbg!();
                    _dirty_swapchain = true;
                }
            }
            Event::RedrawEventsCleared => {
                //
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    log::debug!("WindowEvent::CloseRequested");
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                WindowEvent::Resized { .. } => {
                    _dirty_swapchain = true;
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
