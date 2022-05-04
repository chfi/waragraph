use crossbeam::atomic::AtomicCell;
use gfa::gfa::GFA;
use parking_lot::{Mutex, RwLock};
use raving::vk::{VkEngine, WindowResources};
use waragraph::console::{Console, ConsoleInput};

use ash::vk;

use flexi_logger::{Duplicate, FileSpec, Logger};

use sled::IVec;
use waragraph::graph::{Node, Path, Waragraph};
use waragraph::util::{BufferStorage, LabelStorage};
use waragraph::viewer::app::ViewerSys;
use waragraph::viewer::gui::{GuiLayer, GuiSys, LabelMsg};
use waragraph::viewer::{SlotRenderers, SlotUpdateFn, ViewDiscrete1D};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::collections::HashMap;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use arboard::Clipboard;

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

    let clipboard = Arc::new(Mutex::new(arboard::Clipboard::new()?));
    let clipboard_module = {
        let clipboard = Arc::downgrade(&clipboard);

        let mut module = rhai::Module::new();

        let cb = clipboard.clone();
        module.set_native_fn("get_text", move || {
            if let Some(cb) = cb.upgrade() {
                let mut cb = cb.lock();
                if let Ok(text) = cb.get_text() {
                    return Ok(rhai::ImmutableString::from(text));
                }
            }

            Err("error getting clipboard text".into())
        });

        let cb = clipboard.clone();
        module.set_native_fn("set_text", move |text: &str| {
            if let Some(cb) = cb.upgrade() {
                let mut cb = cb.lock();
                if let Ok(()) = cb.set_text(text.into()) {
                    return Ok(());
                }
            }

            Err("error setting clipboard text".into())
        });

        Arc::new(module)
    };

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

    let graph = Arc::new(Waragraph::from_gfa(&gfa)?);
    let graph_module =
        Arc::new(waragraph::graph::script::create_graph_module(&graph));

    let should_exit = Arc::new(AtomicCell::new(false));

    {
        let exit = should_exit.clone();
        ctrlc::set_handler(move || {
            exit.store(true);
        })?;
    }

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

    let mut gui_sys = GuiSys::init(&mut engine, &db, &swapchain_dims)?;

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
        Some(gui_sys.pass),
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

    let out_framebuffer =
        *window_resources.indices.framebuffers.get("out").unwrap();

    let mut viewer = ViewerSys::init(
        &mut engine,
        &graph,
        &graph_module,
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

    console
        .modules
        .insert("clipboard".into(), clipboard_module.clone());

    console.modules.insert("graph".into(), graph_module.clone());

    console
        .modules
        .insert("slot".into(), viewer.slot_rhai_module.clone());

    console
        .modules
        .insert("gui".into(), gui_sys.rhai_module.clone());

    viewer.labels.allocate_label(&db, &mut engine, "console")?;
    viewer.labels.set_label_pos(b"console", 4, 4)?;
    viewer.labels.set_text_for(b"console", "")?;

    buffers.allocate_queued(&mut engine)?;
    buffers.fill_updated_buffers(&mut engine.resources)?;

    let color_buffer_set = {
        let id = buffers.get_id("gradient-colorbrewer-spectral").unwrap();
        buffers.get_desc_set_ix(id).unwrap()
    };

    let gui_palette_set = {
        let id = buffers.get_id("gui-palette").unwrap();
        buffers.get_desc_set_ix(id).unwrap()
    };

    let mut gui_layer = GuiLayer::new(
        &mut engine,
        &db,
        &mut buffers,
        "main_gui",
        1023,
        gui_palette_set,
    )?;

    let mut gui_tooltip_layer = GuiLayer::new(
        &mut engine,
        &db,
        &mut buffers,
        "tooltip",
        1023,
        gui_palette_set,
    )?;

    let mut gui_legend_layer = GuiLayer::new(
        &mut engine,
        &db,
        &mut buffers,
        "main_gui-legend",
        1023,
        color_buffer_set,
    )?;

    gui_sys.layers.write().insert("main_gui".into(), gui_layer);
    gui_sys
        .layers
        .write()
        .insert("tooltip".into(), gui_tooltip_layer);
    gui_sys
        .layers
        .write()
        .insert("main_gui-legend".into(), gui_legend_layer);

    gui_sys.update_layer_buffers(&buffers)?;

    let mut recreate_swapchain = false;
    let mut recreate_swapchain_timer: Option<std::time::Instant> = None;

    let mut prev_frame_end = std::time::Instant::now();

    let mut mode = Modes::PathViewer;

    // (samples, slot fn name, SlotUpdateFn, Path, view, width)
    type UpdateMsg = (
        Arc<Vec<(Node, usize)>>,
        rhai::ImmutableString,
        SlotUpdateFn<u32>,
        Path,
        (usize, usize),
        usize,
    );

    let (update_tx, update_rx) = crossbeam::channel::unbounded::<UpdateMsg>();

    // path, data, view, width
    type SlotMsg =
        (Path, rhai::ImmutableString, Vec<u32>, (usize, usize), usize);

    let (slot_tx, slot_rx) = crossbeam::channel::unbounded::<SlotMsg>();

    match console.eval(&db, &buffers, "viewer::gui_init()") {
        Ok(v) => {
            log::warn!("success: {:?}", v);
        }
        Err(e) => {
            log::error!("gui on init eval error!! {:?}", e);
        }
    }

    /*
    {
        let mut input = Vec::new();
        input.extend([
            "let p = graph::get_path(\"gi|568815592\");",
            "let n = graph::node(41);",
            "let g = graph::get_graph();",
            "let s = slot::load_bed_file(g, \"A-3105.test2.bed\");",
            "let ds_name = slot::create_data_source(s);",
            "let ds = slot::get_data_source(ds_name);",
            "let fn_name = \"bed_slot_fn\";",
            "let slot_fn = slot::new_slot_fn_from_data_source(ds_name, fn_name);",
            "slot::set_slot_color_scheme(fn_name, \"gradient-colorbrewer-spectral\");",
            "cfg.set(\"viz.slot_function\", fn_name);",
        ]);

        for line in input {
            log::warn!("evaluating `{}`", line);
            match console.eval(&db, &buffers, line) {
                Ok(v) => {
                    // log::warn!("success: {:?}", v);
                }
                Err(e) => {
                    log::error!("console error {:?}", e);
                }
            }
        }
    }
    */

    //
    let _update_threads = (0..4)
        .map(|_| {
            let input = update_rx.clone();
            let out = slot_tx.clone();

            std::thread::spawn(move || {
                let mut buffer = Vec::new();

                loop {
                    while let Ok((
                        samples,
                        slot_fn_name,
                        slot_fn,
                        path,
                        view,
                        width,
                    )) = input.recv()
                    {
                        buffer.clear();
                        buffer.extend(
                            (0..width).map(|i| slot_fn(&samples, path, i)),
                        );

                        let msg =
                            (path, slot_fn_name, buffer.clone(), view, width);
                        if let Err(e) = out.send(msg) {
                            log::error!("Update thread error: {:?}", e);
                        }
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    let mut prev_frame = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                let delta_time = prev_frame.elapsed().as_secs_f32();
                prev_frame = std::time::Instant::now();

                // console scope updates
                {
                    console
                        .scope
                        .set_value("dt", rhai::Dynamic::from_float(delta_time));
                }

                match console.eval(&db, &buffers, "viewer::gui_update(dt)") {
                    Ok(v) => {
                        // log::warn!("success: {:?}", v);
                    }
                    Err(e) => {
                        log::error!("gui on update eval error!! {:?}", e);
                    }
                }

                // handle sled-based buffer updates
                buffers.allocate_queued(&mut engine).unwrap();
                buffers.fill_updated_buffers(&mut engine.resources).unwrap();

                if let Err(e) = gui_sys.update_layer_buffers(&buffers) {
                    log::error!("GUI layer update error: {:?}", e);
                }

                while let Ok(label_msg) = gui_sys.label_msg_rx.try_recv() {
                    if let Some(layer) =
                        gui_sys.layers.write().get_mut(&label_msg.layer_name)
                    {
                        layer
                            .apply_label_msg(
                                &mut engine,
                                &db,
                                &mut gui_sys.labels,
                                label_msg,
                            )
                            .unwrap();
                    }
                }

                while let Ok((path, slot_fn_name, data, view, width)) =
                    slot_rx.try_recv()
                {
                    let slot_ix =
                        viewer.path_viewer.slots.read().get_slot_ix(path);

                    if let Some(slot_ix) = slot_ix {
                        viewer.path_viewer.apply_update(
                            &mut engine.resources,
                            slot_fn_name,
                            slot_ix,
                            &data,
                            view,
                            width,
                        );
                    }
                }

                {
                    let [_, h] = swapchain_dims.load();

                    let vis_count = viewer.visible_slot_count(&graph, h);

                    {
                        let (o, _l) = viewer.path_viewer.row_view.load();
                        viewer.path_viewer.row_view.store((o, vis_count));
                    }

                    let cap = viewer.path_viewer.slots.read().capacity();
                    let slot_width = viewer.path_viewer.width;

                    let diff = vis_count.checked_sub(cap).unwrap_or_default();
                    if diff > 0 {
                        log::warn!("allocating {} slots", diff);
                        viewer.path_viewer.force_update();
                    }

                    let mut slots = viewer.path_viewer.slots.write();
                    for _ in 0..diff {
                        let i = slots.capacity();
                        slots.allocate_slot(
                            &mut engine,
                            &db,
                            &mut viewer.labels,
                            slot_width,
                        );

                        let name = format!("path-name-{}", i);
                        viewer
                            .labels
                            .allocate_label(&db, &mut engine, &name)
                            .unwrap();
                    }

                    let paths = viewer.path_viewer.visible_paths(&graph);
                    slots.bind_paths(paths).unwrap();
                }

                let mut should_update = false;

                // path-viewer specific, dependent on previous view
                if viewer.path_viewer.should_update() {
                    should_update = true;

                    let view = viewer.view.load();
                    let range = view.range();
                    let start = range.start.to_string();
                    let end = range.end.to_string();
                    let len = view.len().to_string();

                    viewer.labels.set_text_for(b"view:start", &start).unwrap();
                    viewer.labels.set_text_for(b"view:len", &len).unwrap();
                    viewer.labels.set_text_for(b"view:end", &end).unwrap();

                    viewer.path_viewer.sample(&graph, &view);
                }

                if viewer.path_viewer.has_new_samples() || should_update {
                    if let Err(e) =
                        viewer.queue_slot_updates(&graph, &update_tx)
                    {
                        log::error!("PathViewer slot update error: {:?}", e);
                    }
                }

                // TODO: should only be called when the view has
                // scrolled, but it should also update whenever the
                // label layout changes, and there's currently no way
                // to check just for that
                viewer.update_labels(&graph);

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
                    viewer
                        .labels
                        .update(&mut engine.resources, &key, &value)
                        .unwrap();
                }

                let mut updates: HashMap<IVec, IVec> = HashMap::default();

                while let Ok(ev) = gui_sys
                    .label_updates
                    .next_timeout(Duration::from_micros(10))
                {
                    match ev {
                        sled::Event::Insert { key, value } => {
                            updates.insert(key, value);
                        }
                        _ => (),
                    }
                }

                for (key, value) in updates {
                    gui_sys
                        .labels
                        .update(&mut engine.resources, &key, &value)
                        .unwrap();
                }

                // update end

                if recreate_swapchain_timer.is_none() && !recreate_swapchain {
                    let render_success = match mode {
                        Modes::PathViewer => viewer
                            .render(
                                &mut engine,
                                &buffers,
                                &window,
                                &window_resources,
                                &graph,
                                &gui_sys,
                            )
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
                                &graph,
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
                        if !c.is_ascii_control() && c.is_ascii() {
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
                                    if let Err(e) = console.handle_input(
                                        &db,
                                        &buffers,
                                        &viewer.labels,
                                        ConsoleInput::Submit,
                                    ) {
                                        log::error!("Console error: {:?}", e);
                                    }
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
                log::debug!("Freeing resources");

                let _ = clipboard;

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

        if should_exit.load() {
            log::debug!("Ctrl-C received, exiting");
            *control_flow = winit::event_loop::ControlFlow::Exit;
        }
    });

    Ok(())
}
