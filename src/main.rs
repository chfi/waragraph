use crossbeam::atomic::AtomicCell;
use gfa::gfa::GFA;
use parking_lot::{Mutex, RwLock};

use raving::compositor::label_space::LabelSpace;
use raving::compositor::Compositor;
use raving::script::console::frame::Resolvable;
use raving::vk::{DescSetIx, VkEngine, WindowResources};

use waragraph::cli::ViewerArgs;
use waragraph::console::data::{AnnotationSet, BedColumn};
use waragraph::console::layout::{LabelLayout, LabelStacks};
use waragraph::console::{Console, ConsoleInput};

use ash::vk;

use flexi_logger::{Duplicate, FileSpec, Logger};

use sled::IVec;
use waragraph::graph::{Node, Path, Waragraph};
use waragraph::util::{BufferStorage, LabelStorage};
use waragraph::viewer::app::ViewerSys;
use waragraph::viewer::gui::tree_list::{Breadcrumbs, ListPopup, TreeList};
use waragraph::viewer::{SlotUpdateFn, ViewDiscrete1D};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::collections::HashMap;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};

use rand::prelude::*;

fn main() -> Result<()> {
    let viewer_args: ViewerArgs = argh::from_env();

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

    let gfa_path = &viewer_args.gfa_path;

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

    let mut compositor = Compositor::init(&mut engine, &swapchain_dims)?;

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
        Some(compositor.pass),
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
        &graph,
        &graph_module,
        &db,
        &mut buffers,
        &mut window_resources,
        width,
    )?;

    let mut console = Console::init(&mut engine, &mut compositor)?;

    console.ast = Arc::new(viewer.frame.ast.clone_functions_only());

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

    let font_desc_set = {
        let font_desc_set =
            viewer.frame.module.get_var("font_desc_set").unwrap();
        let r = font_desc_set.cast::<Resolvable<DescSetIx>>();
        r.get().unwrap()
    };

    buffers.allocate_queued(&mut engine)?;
    buffers.fill_updated_buffers(&mut engine.resources)?;

    let label_space = LabelSpace::new(&mut engine, "test-labels", 1024 * 1024)?;

    let label_space = Arc::new(RwLock::new(label_space));
    console
        .scope
        .push_constant("label_space", label_space.clone());

    waragraph::viewer::gui::layer::add_sublayer_defs(
        &mut engine,
        &mut compositor,
        font_desc_set,
    )?;

    let popup_list =
        ListPopup::new(&mut engine, &mut compositor, "popup", 300.0, 100.0)?;

    let popup_list = Arc::new(RwLock::new(popup_list));

    let popup_module = {
        let mut module = rhai::Module::new();

        let state_stack = popup_list.read().popup_stack.clone();

        module.set_native_fn(
            "popup",
            move |values: rhai::Array, fn_ptr: rhai::FnPtr| {
                if let Some(mut stack) = state_stack.try_write() {
                    let values = rhai::Dynamic::from_array(values);
                    stack.push((values, fn_ptr));
                } else {
                    log::error!(
                        "attempted recursive lock of popup state stack"
                    );
                }
                Ok(())
            },
        );

        Arc::new(module)
    };

    console.modules.insert("popup".into(), popup_module.clone());

    {
        let module =
            waragraph::viewer::gui::layer::create_rhai_module(&compositor);
        console.modules.insert("ui".into(), Arc::new(module));
    }

    for name in ["popup", "ui", "clipboard", "graph"] {
        let module = console.modules.get(name).unwrap();
        viewer.engine.register_static_module(name, module.clone());
    }

    let mut recreate_swapchain = false;
    let mut recreate_swapchain_timer: Option<std::time::Instant> = None;

    let mut prev_frame_end = std::time::Instant::now();

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

    match console.eval(&db, &buffers, "viewer::gui_init(label_space)") {
        Ok(v) => {
            log::warn!("success: {:?}", v);
        }
        Err(e) => {
            log::error!("gui on init eval error!! {:?}", e);
        }
    }

    let mut label_stacks: Option<LabelStacks> = None;

    if let Some(bed_path) = &viewer_args.bed_path {
        let bed_str = bed_path.to_str().unwrap();

        let bed_name = bed_path.file_stem().and_then(|s| s.to_str()).unwrap();

        let mut column_map = rhai::Map::default();

        for col_ix in viewer_args.bed_column.iter() {
            let name = format!("{}:{}", bed_name, col_ix);
            let col_ix = *col_ix as i64;
            column_map.insert(name.into(), col_ix.into());
        }

        if viewer_args.bed_column.is_empty() {
            let name = format!("{}:{}", bed_name, 3);
            column_map.insert(name.into(), 3i64.into());
        }

        console
            .scope
            .push("bed_path", rhai::ImmutableString::from(bed_str));
        console.scope.push("column_map", column_map);

        // eval this script
        let script = r##"
import "script/bed" as bed;
bed::load_bed_file(bed_path, column_map)
"##;

        match console.eval(&db, &buffers, &script) {
            Ok(val) => {
                console.scope.push("bed_file", val);
            }
            Err(e) => {
                log::error!("console error {:?}", e);
            }
        }
    }

    if let Some(script_path) = &viewer_args.run_script {
        match console.eval_file(&db, &buffers, &script_path) {
            Ok(val) => {
                console.scope.push("run_result", val);
            }
            Err(e) => {
                log::error!(
                    "Error when executing script `{:?}`: {:#?}",
                    script_path,
                    e
                );
            }
        }
    }

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

    let should_exit = Arc::new(AtomicCell::new(false));

    {
        let exit = should_exit.clone();
        ctrlc::set_handler(move || {
            exit.store(true);
        })?;
    }

    let mut mouse_clicked = false;

    let mut layout_update_since = 0.0;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                let delta_time = prev_frame.elapsed().as_secs_f32();

                /*
                layout_update_since += delta_time;

                if layout_update_since > 0.05 {


                    let [width, _] = swapchain_dims.load();

                    let [slot_offset, slot_width] =
                        viewer.slot_x_offsets(width);

                    let view = viewer.view.load();

                    label_layout.step(slot_width, layout_update_since);

                    label_layout
                        .update_layer(
                            &mut compositor,
                            slot_offset,
                            slot_width,
                            view.offset,
                            view.len,
                            view.max,
                        )
                        .unwrap();

                    let _ = label_layout
                        .label_space
                        .write_buffer(&mut engine.resources);

                    layout_update_since = 0.0;
                }
                */

                prev_frame = std::time::Instant::now();

                {
                    let labels = label_space.read();
                    let _ = labels.write_buffer(&mut engine.resources);
                }

                if let Err(e) = console.update_layer(
                    &mut engine.resources,
                    &mut compositor,
                    [0.0, 0.0],
                ) {
                    log::error!("Console compositor error: {:?}", e);
                }

                let mouse_pos = {
                    let (x, y) = waragraph::input::get_mouse_pos();
                    [x as f32, y as f32]
                };

                let (all_crumbs, options) = {
                    let slot_fns = viewer.slot_functions.read();

                    let options = slot_fns
                        .slot_fn_u32
                        .keys()
                        .map(|k| rhai::Dynamic::from(k.clone()))
                        .collect::<Vec<_>>();

                    let options = rhai::Dynamic::from_array(options);

                    let crumbs = Breadcrumbs::all_crumbs(&options);

                    (crumbs, options)
                };

                {
                    let mut popup = popup_list.write();

                    if let Err(e) = popup.update_layer(
                        &mut engine.resources,
                        &db,
                        &buffers,
                        &console,
                        &mut compositor,
                        mouse_pos,
                        mouse_clicked,
                    ) {
                        log::error!("popup error: {:?}", e);
                    }
                }

                if let Err(e) = compositor.allocate_sublayers(&mut engine) {
                    log::error!("Compositor error: {:?}", e);
                }

                if let Err(e) = compositor.write_layers(&mut engine.resources) {
                    log::error!("Compositor error: {:?}", e);
                }

                // console scope updates
                {
                    console
                        .scope
                        .set_value("dt", rhai::Dynamic::from_float(delta_time));
                }

                match console.eval(
                    &db,
                    &buffers,
                    "viewer::gui_update(label_space, dt)",
                ) {
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
                        if let Err(e) = slots.allocate_slot(
                            &mut engine,
                            &db,
                            &mut viewer.labels,
                            slot_width,
                        ) {
                            log::error!("Path slot allocation error: {:?}", e);
                        }

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
                    let [slot_offset, slot_width] =
                        viewer.slot_x_offsets(width);

                    /*
                    label_layout.reset_for_view(
                        &mut rng,
                        &viewer.view.load(),
                        slot_width,
                    );
                    */

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

                // update end

                mouse_clicked = false;

                if recreate_swapchain_timer.is_none() && !recreate_swapchain {
                    let render_success = match viewer.render(
                        &mut engine,
                        &buffers,
                        &window,
                        &window_resources,
                        &graph,
                        &compositor,
                    ) {
                        Ok(_) => true,
                        Err(e) => {
                            log::error!("Render error: {:?}", e);
                            false
                        }
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
                viewer.handle_input(&console, &event);

                match event {
                    WindowEvent::ReceivedCharacter(c) => {
                        if !c.is_ascii_control() && c.is_ascii() {
                            console
                                .handle_input(
                                    &db,
                                    &buffers,
                                    ConsoleInput::AppendChar(c),
                                )
                                .unwrap();
                        }
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        if button == winit::event::MouseButton::Left
                            && state == winit::event::ElementState::Pressed
                            && !mouse_clicked
                        {
                            log::error!("mouse clicked!");
                            mouse_clicked = true;
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
                                if matches!(kc, VK::Space) {
                                    if let Some(labels) = label_stacks.as_mut()
                                    {
                                        let [width, _] = swapchain_dims.load();
                                        let [slot_offset, slot_width] =
                                            viewer.slot_x_offsets(width);

                                        labels
                                            .update_layer(
                                                &mut compositor,
                                                &graph,
                                                viewer.view.load(),
                                                slot_offset,
                                                slot_width,
                                            )
                                            .unwrap();
                                    }
                                }
                                if matches!(kc, VK::Return) {
                                    if let Err(e) = console.handle_input(
                                        &db,
                                        &buffers,
                                        ConsoleInput::Submit,
                                    ) {
                                        log::error!("Console error: {:?}", e);
                                    }
                                } else if matches!(kc, VK::Back) {
                                    console
                                        .handle_input(
                                            &db,
                                            &buffers,
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
