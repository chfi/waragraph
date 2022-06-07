use crossbeam::atomic::AtomicCell;
use gfa::gfa::GFA;
use parking_lot::{Mutex, RwLock};

use raving::compositor::label_space::LabelSpace;
use raving::compositor::Compositor;
use raving::script::console::frame::Resolvable;
use raving::vk::{DescSetIx, VkEngine, WindowResources};

use waragraph::cli::ViewerArgs;
use waragraph::console::layout::LabelStacks;
use waragraph::console::{Console, ConsoleInput};

use ash::vk;

use flexi_logger::{Duplicate, FileSpec, Logger};

use waragraph::geometry::ListLayout;
use waragraph::graph::{Path, Waragraph};
use waragraph::util::BufferStorage;
use waragraph::viewer::app::ViewerSys;
use waragraph::viewer::debug::DebugLayers;
use waragraph::viewer::edges::{EdgeCache, EdgeVertexCache};
use waragraph::viewer::gui::tree_list::{Breadcrumbs, ListPopup};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::sync::Arc;

fn main() -> anyhow::Result<()> {
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

    let edge_cache = EdgeCache::new(&graph);
    let mut edge_cache = EdgeVertexCache::new(edge_cache);

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

    let mut compositor = Compositor::init(
        &mut engine,
        &swapchain_dims,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::GENERAL,
    )?;

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
        Some(compositor.load_pass),
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
        &compositor,
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

    console.scope.set_value("globals", rhai::Map::default());

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

    // let mut list_layout = ListLayout {
    //     origin: Point2D::new(40.0, 40.0),
    //     size: Size2D::new(500.0, 500.0),
    //     side_offsets: None,
    //     slot_height: Length::new(32.0),
    // };

    let mut debug_layers =
        DebugLayers::new(&mut engine, &mut compositor, "debug", 200)?;

    if let Err(e) = compositor.allocate_sublayers(&mut engine) {
        log::error!("Compositor error: {:?}", e);
    }

    /*
    let debug_layer_id = 0usize;

    {
        use waragraph::viewer::debug::Shape;

        let color = |r: f32, g, b| rgb::RGBA::new(r, g, b, 1.0);

        let shapes = [
            (
                Shape::rect(100.0, 100.0, 300.0, 200.0),
                color(0.7, 0.2, 0.2),
            ),
            (
                Shape::label(120.0, 120.0, "hello world"),
                color(0.0, 0.0, 0.0),
            ),
        ];

        debug_layers.fill_layer(&mut compositor, debug_layer_id, shapes)?;

        debug_layers.update(&mut engine)?;
    }
    */

    let mut recreate_swapchain = false;
    let mut recreate_swapchain_timer: Option<std::time::Instant> = None;

    let mut prev_frame_end = std::time::Instant::now();

    match console.eval(&db, &buffers, "viewer::gui_init(globals, label_space)")
    {
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
            let name = col_ix.to_string();
            let col_ix = *col_ix as i64;
            column_map.insert(name.into(), col_ix.into());
        }

        if viewer_args.bed_column.is_empty() {
            let name = 3i64.to_string();
            column_map.insert(name.into(), 3i64.into());
        }

        console
            .scope
            .push("bed_name", rhai::ImmutableString::from(bed_name));
        console
            .scope
            .push("bed_path", rhai::ImmutableString::from(bed_str));
        console.scope.push("column_map", column_map);

        // eval this script
        let script = r##"
import "script/bed" as bed;
bed::load_bed_file(bed_path, bed_name, column_map)
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

    let mut prev_frame = std::time::Instant::now();

    let should_exit = Arc::new(AtomicCell::new(false));

    {
        let exit = should_exit.clone();
        ctrlc::set_handler(move || {
            exit.store(true);
        })?;
    }

    let _update_threads = (0..4)
        .map(|_| {
            let exec = viewer.path_viewer.cache.data_msg_worker();
            let should_exit = should_exit.clone();

            std::thread::spawn(move || {
                let mut prev_error = None;
                loop {
                    if let Err(e) = exec() {
                        if !should_exit.load() {
                            let error =
                                format!("Cache data worker error: {:?}", e);

                            if prev_error.as_ref() != Some(&error) {
                                log::warn!("{}", error);
                                prev_error = Some(error);
                            }
                        }
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    let mut mouse_clicked = false;

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


                // prepare the path slot sublayer buffers
                if let Err(e) = compositor.with_layer("path-slots", |layer| {
                    let window_dims = swapchain_dims.load();
                    let slot_fns = viewer.slot_functions.read();
                    viewer.path_viewer.update_slot_sublayer(
                        &graph,
                        &mut viewer.label_space,
                        layer,
                        window_dims,
                        &viewer.config,
                        &slot_fns,
                        &buffers,
                    )?;
                    Ok(())
                }) {
                    log::warn!("path sublayer update error: {:?}", e);
                }


                {
                    let globals: Option<rhai::Map> = console.scope.get_value("globals");
                    let hovered_path =
                        match viewer.props.map.read().get("hovered_path") {
                            None => None,
                            Some(val) => {
                                if val.type_name() == "waragraph::graph::Path" {
                                    let path = val.clone_cast::<Path>();
                                    Some(path)
                                } else {
                                    None
                                }
                            }
                        };


                    if let Some(path) = hovered_path {

                        if graph.path_name(path).is_some() {

                            let [width, height] = swapchain_dims.load();
                            let [slot_offset, slot_width] =
                                viewer.slot_x_offsets(width);

                            let y0 =
                                viewer
                                .props
                                .map
                                .read()
                                .get("hovered_slot_y")
                                .unwrap()
                                .clone_cast::<i64>();

                            // let y0 = (height - 100) as f32;
                            let y0 = y0 as f32;
                            // let yd =

                            // let vis_row_count =
                            //     viewer.visible_slot_count(&graph, window_height);


                            let view = viewer.view.load();

                            compositor.with_layer("edges", |layer| {
                                if let Some(sublayer) = layer.get_sublayer_mut("lines") {
                                    let draw_data =
                                        sublayer.draw_data_mut().next().unwrap();

                                    edge_cache.update_sublayer_data_with_path(
                                        &graph,
                                        path,
                                        view,
                                        slot_offset as u32,
                                        y0 as u32,
                                        slot_width as u32,
                                        90,
                                        draw_data).unwrap();
                                }

                                Ok(())
                            }).unwrap();
                        }
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
                    "viewer::gui_update(globals, label_space, dt)",
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


                {
                    let view = viewer.view.load();

                    let slot_fns = viewer.slot_functions.read();

                    let [win_width, win_height] = swapchain_dims.load();

                    let [slot_offset, slot_width] =
                        viewer.slot_x_offsets(win_width);

                    let vis_count =
                        viewer.visible_slot_count(&graph, win_height);

                    let width = slot_width as usize;

                    if let Err(e) = viewer.path_viewer.update(
                        &mut engine,
                        &graph,
                        &mut viewer.label_space,
                        &slot_fns,
                        &viewer.config,
                        width,
                        view,
                        vis_count,
                    ) {
                        log::error!("Path viewer update error: {:?}", e);

                        use waragraph::viewer::cache::CacheError;

                        // the above update() call will handle
                        // reallocation, so any cache errors should
                        // never reach here, but make sure to crash
                        // just in case
                        if let Some(_) =
                            e.downcast_ref::<CacheError>()
                        {
                            panic!("Path viewer slot allocation failure -- this shouldn't happen!");
                        }
                    }
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

                    /*
                    let fps = (1.0 / ft) as u32;
                    viewer
                        .labels
                        .set_text_for(b"fps", &fps.to_string())
                        .unwrap();
                    */

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
                    WindowEvent::ModifiersChanged(mod_state) => {
                        waragraph::input::set_modifiers(mod_state);
                    }
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
                                // if matches!(kc, VK::Space) {
                                // }
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
                should_exit.store(true);

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
