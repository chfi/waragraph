use engine::script::console::frame::FrameBuilder;
use engine::script::console::BatchBuilder;
use engine::vk::{BatchInput, FrameResources, GpuResources, VkEngine};

use engine::vk::util::*;

use ash::{vk, Device};

use flexi_logger::{Duplicate, FileSpec, Logger};
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
    paths: Vec<Vec<u32>>,
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
    let mut paths = Vec::new();
    let mut path_names = Vec::new();

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
            paths.push(steps);
        }
    }

    let graph_data = GraphData {
        node_count: segments.len(),
        node_lengths: segments,

        path_names,
        paths,
    };

    /*
    let event_loop = EventLoop::new();

    let width = 800;
    let height = 600;

    let window = WindowBuilder::new()
        .with_title("Waragraph Viewer")
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .build(&event_loop)?;

    let mut engine = VkEngine::new(&window)?;
    */

    Ok(())
}

fn main_old() -> Result<()> {
    let mut args = std::env::args();

    let _ = args.next().unwrap();

    let script_path = args.next().ok_or(anyhow!("Provide a script path"))?;

    // let args: Args = argh::from_env();

    let spec = "debug";
    let _logger = Logger::try_with_env_or_str(spec)?
        .log_to_file(FileSpec::default())
        .duplicate_to_stderr(Duplicate::Debug)
        .start()?;

    /*
    let event_loop: EventLoop<()>;

    #[cfg(target_os = "linux")]
    {
        use winit::platform::unix::EventLoopExtUnix;
        event_loop = EventLoop::new_x11()?;
        // event_loop = if args.force_x11 || !instance_exts.wayland_surface {
        //     if let Ok(ev_loop) = EventLoop::new_x11() {
        //         log::debug!("Using X11 event loop");
        //         ev_loop
        //     } else {
        //         error!(
        //             "Error initializing X11 window, falling back to default"
        //         );
        //         EventLoop::new()
        //     }
        // } else {
        //     log::debug!("Using default event loop");
        //     EventLoop::new()
        // };
    }

    #[cfg(not(target_os = "linux"))]
    {
        log::debug!("Using default event loop");
        let event_loop = EventLoop::new();
    }
    */

    let event_loop = EventLoop::new();

    let width = 800;
    let height = 600;

    let window = WindowBuilder::new()
        .with_title("engine")
        .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
        .build(&event_loop)?;

    let mut engine = VkEngine::new(&window)?;

    let (out_image, out_view) = engine.with_allocators(|ctx, res, alloc| {
        let out_image = res.allocate_image(
            ctx,
            alloc,
            width,
            height,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC,
            Some("out_image"),
        )?;

        let out_view = res.create_image_view_for_image(ctx, out_image)?;

        Ok((out_image, out_view))
    })?;

    log::warn!("MODULE BUILDER");

    let mut builder = FrameBuilder::from_script(&script_path)?;

    builder.bind_var("out_image", out_image)?;
    builder.bind_var("out_view", out_view)?;

    engine.with_allocators(|ctx, res, alloc| {
        builder.resolve(ctx, res, alloc)?;
        Ok(())
    })?;
    log::warn!("is resolved: {}", builder.is_resolved());

    let mut rhai_engine = engine::script::console::create_batch_engine();

    let arc_module = Arc::new(builder.module.clone());

    rhai_engine.register_static_module("self", arc_module.clone());

    let init = rhai::Func::<(), BatchBuilder>::create_from_ast(
        rhai_engine,
        builder.ast.clone_functions_only(),
        "init",
    );

    let mut rhai_engine = engine::script::console::create_batch_engine();
    rhai_engine.register_static_module("self", arc_module.clone());

    let draw_background =
        rhai::Func::<(i64, i64, f32), BatchBuilder>::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "background",
        );

    let mut rhai_engine = engine::script::console::create_batch_engine();
    rhai_engine.register_static_module("self", arc_module);

    let draw_foreground =
        rhai::Func::<(i64, i64, f32), BatchBuilder>::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "foreground",
        );

    let mut frames = {
        let queue_ix = engine.queues.thread.queue_family_index;

        // hardcoded for now
        let semaphore_count = 3;
        let cmd_buf_count = 3;

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

    std::thread::sleep(std::time::Duration::from_millis(100));

    let start = std::time::Instant::now();

    {
        let init_builder = init()?;

        if !init_builder.init_fn.is_empty() {
            log::warn!("submitting init batches");
            let fence =
                engine.submit_batches_fence(init_builder.init_fn.as_slice())?;

            engine.block_on_fence(fence)?;
        }
    }

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

                let bg_batch = draw_background(800, 600, t).unwrap();
                let bg_batch_fn = bg_batch.build();
                let bg_rhai_batch = bg_batch_fn.clone();

                let fg_batch = draw_foreground(800, 600, t).unwrap();
                let fg_batch_fn = fg_batch.build();
                let fg_rhai_batch = fg_batch_fn.clone();

                let bg_batch = Box::new(
                    move |dev: &Device,
                          res: &GpuResources,
                          _input: &BatchInput,
                          cmd: vk::CommandBuffer| {
                        bg_rhai_batch(dev, res, cmd);
                    },
                ) as Box<_>;

                let fg_batch = Box::new(
                    move |dev: &Device,
                          res: &GpuResources,
                          _input: &BatchInput,
                          cmd: vk::CommandBuffer| {
                        fg_rhai_batch(dev, res, cmd);
                    },
                ) as Box<_>;

                let batches = [&bg_batch, &fg_batch, &copy_batch];

                let deps = vec![
                    None,
                    Some(vec![(0, vk::PipelineStageFlags::COMPUTE_SHADER)]),
                    Some(vec![(1, vk::PipelineStageFlags::COMPUTE_SHADER)]),
                ];

                // dbg!();
                let render_success = engine
                    .draw_from_batches(frame, &batches, deps.as_slice(), 2)
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
}
