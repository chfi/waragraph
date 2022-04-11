use crossbeam::atomic::AtomicCell;
use gfa::gfa::GFA;
use raving::vk::{VkEngine, WindowResources};
use waragraph::console::{Console, ConsoleInput};

use ash::vk;

use flexi_logger::{Duplicate, FileSpec, Logger};

use sled::IVec;
use waragraph::graph::Waragraph;
use waragraph::util::{BufferStorage, LabelStorage};
use waragraph::viewer::app::ViewerSys;
use waragraph::viewer::{SlotRenderers, ViewDiscrete1D};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::collections::HashMap;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modes {
    PathViewer,
    Graph3D,
}

fn main() -> Result<()> {
    // disable sled logging
    let spec = "debug, sled=info";
    // let spec = "debug";
    let _logger = Logger::try_with_env_or_str(spec)?
        .log_to_file(FileSpec::default().suppress_timestamp())
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

    let db = db_cfg.open()?;

    let waragraph = Arc::new(Waragraph::from_gfa(&gfa)?);

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

    let window = {
        let gfa_path = std::path::PathBuf::from(gfa_path);

        let gfa_name =
            gfa_path.file_name().and_then(|s| s.to_str()).unwrap_or("-");

        WindowBuilder::new()
            .with_title(&format!("Waragraph Viewer - {}", gfa_name))
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .build(&event_loop)?
    };

    let mut engine = VkEngine::new(&window)?;

    let mut buffers = BufferStorage::new(&db)?;

    let mut window_resources = WindowResources::new();
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
        None,
    )?;

    {
        let size = window.inner_size();
        let builder =
            window_resources.build(&mut engine, size.width, size.height)?;
        engine.with_allocators(|ctx, res, alloc| {
            builder.insert(&mut window_resources.indices, ctx, res, alloc)?;
            Ok(())
        })?;
    }

    let mut viewer = ViewerSys::init(
        &mut engine,
        &waragraph,
        &db,
        &mut buffers,
        &mut window_resources,
        width,
    )?;

    let mut console = Console::default();

    console.scope.set_value("cfg", viewer.config.clone());
    console
        .modules
        .insert("viewer".into(), viewer.rhai_module.clone());

    viewer.labels.allocate_label(&db, &mut engine, "console")?;
    viewer.labels.set_label_pos(b"console", 4, 4)?;
    viewer.labels.set_text_for(b"console", "")?;

    let mut recreate_swapchain = false;
    let mut recreate_swapchain_timer: Option<std::time::Instant> = None;

    let mut prev_frame_end = std::time::Instant::now();

    let mut mode = Modes::PathViewer;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                // handle sled-based buffer updates
                buffers.allocate_queued(&mut engine).unwrap();
                buffers.fill_updated_buffers(&mut engine.resources).unwrap();

                // path-viewer specific, dependent on previous view
                if viewer.path_viewer.should_update() {
                    let view = viewer.view;
                    let range = view.range();
                    let start = range.start.to_string();
                    let end = range.end.to_string();
                    let len = view.len().to_string();

                    viewer.labels.set_text_for(b"view:start", &start).unwrap();
                    viewer.labels.set_text_for(b"view:len", &len).unwrap();
                    viewer.labels.set_text_for(b"view:end", &end).unwrap();

                    viewer.path_viewer.sample(&waragraph, &view);
                }

                if viewer.path_viewer.has_new_samples() {
                    if let Err(e) = viewer.update_slots(&mut engine.resources) {
                        log::error!("PathViewer slot update error: {:?}", e);
                    }
                }

                // TODO: should only be called when the view has
                // scrolled, but it should also update whenever the
                // label layout changes, and there's currently no way
                // to check just for that
                viewer.update_labels(&waragraph);

                // handle sled-based label updates
                // TODO: currently console relies on this to render
                let mut updates: HashMap<IVec, IVec> = HashMap::default();

                while let Ok(ev) =
                    viewer.label_updates.next_timeout(Duration::from_micros(10))
                {
                    match ev {
                        sled::Event::Insert { key, value } => {
                            updates.insert(key, value);
                        }
                        _ => (),
                    }
                }

                for (key, value) in updates {
                    let id = u64::read_from(key[2..].as_ref()).unwrap();
                    let buf_ix =
                        viewer.labels.buffer_for_id(id).unwrap().unwrap();
                    let buffer = &mut engine.resources[buf_ix];
                    let slice = buffer.mapped_slice_mut().unwrap();
                    let len = value.len();

                    slice[0..4].clone_from_slice(&(len as u32).to_ne_bytes());

                    slice[4..]
                        .chunks_mut(4)
                        .zip(value.iter())
                        .for_each(|(chk, &b)| chk.fill(b));
                }

                // update end

                if recreate_swapchain_timer.is_none() && !recreate_swapchain {
                    let render_success = match mode {
                        Modes::PathViewer => viewer
                            .render(&mut engine, &window, &window_resources)
                            .unwrap(),
                        Modes::Graph3D => todo!(),
                    };

                    if !render_success {
                        recreate_swapchain = true;
                    }

                    let ft = prev_frame_end.elapsed().as_secs_f64();
                    let fps = (1.0 / ft) as u32;
                    viewer
                        .labels
                        .set_text_for(b"fps", &fps.to_string())
                        .unwrap();
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

                        // TODO queue this up somehow
                        viewer
                            .resize(
                                &waragraph,
                                &mut engine,
                                &mut window_resources,
                                size.width,
                                size.height,
                            )
                            .unwrap();

                        recreate_swapchain_timer = None;
                    }
                }
            }

            Event::WindowEvent { event, .. } => {
                match mode {
                    Modes::PathViewer => {
                        //
                        viewer.handle_input(&event);
                    }
                    Modes::Graph3D => todo!(),
                }

                match event {
                    WindowEvent::ReceivedCharacter(c) => {
                        if !c.is_ascii_control() {
                            console
                                .handle_input(
                                    &db,
                                    &buffers,
                                    &viewer.labels,
                                    ConsoleInput::AppendChar(c),
                                )
                                .unwrap();
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        waragraph::input::set_mouse_pos(position.x, position.y);
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        if let Some(kc) = input.virtual_keycode {
                            use VirtualKeyCode as VK;

                            if input.state
                                == winit::event::ElementState::Pressed
                            {
                                if matches!(kc, VK::Return) {
                                    console
                                        .handle_input(
                                            &db,
                                            &buffers,
                                            &viewer.labels,
                                            ConsoleInput::Submit,
                                        )
                                        .unwrap();
                                } else if matches!(kc, VK::Back) {
                                    console
                                        .handle_input(
                                            &db,
                                            &buffers,
                                            &viewer.labels,
                                            ConsoleInput::Backspace,
                                        )
                                        .unwrap();
                                }
                            }
                        }
                    }
                    WindowEvent::CloseRequested => {
                        log::debug!("WindowEvent::CloseRequested");
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                    WindowEvent::Resized { .. } => {
                        recreate_swapchain_timer =
                            Some(std::time::Instant::now());
                    }
                    _ => (),
                }
            }
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
