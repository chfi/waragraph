use crossbeam::atomic::AtomicCell;
use euclid::SideOffsets2D;
use euclid::{point2, size2, Length};
use gfa::gfa::GFA;
use parking_lot::{Mutex, RwLock};

use rand::Rng;
use raving::compositor::label_space::LabelSpace;
use raving::compositor::Compositor;
use raving::script::console::frame::Resolvable;
use raving::vk::{DescSetIx, VkEngine, WindowResources};

use waragraph::cli::ViewerArgs;
use waragraph::console::layout::LabelStacks;
use waragraph::console::{Console, ConsoleInput};

use ash::vk;

use flexi_logger::{Duplicate, FileSpec, Logger};

use waragraph::geometry::dynamics::verlet::{
    Entity, Rail, RailLink, RailStep, VerletSolver,
};
use waragraph::geometry::dynamics::CurveLayout;
use waragraph::geometry::{ListLayout, ScreenPoint, ScreenRect};
use waragraph::graph::{Path, Waragraph};
use waragraph::text::TextCache;
use waragraph::util::BufferStorage;
use waragraph::viewer::app::ViewerSys;
use waragraph::viewer::debug::DebugLayers;
use waragraph::viewer::edges::{EdgeCache, EdgeVertexCache};
use waragraph::viewer::gui::tree_list::{Breadcrumbs, ListPopup};
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::sync::Arc;

use rand::prelude::*;

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
    console.scope.set_value("props", viewer.props.clone());
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

    if let Err(e) = waragraph::viewer::gui::layer::add_sublayer_defs(
        &mut engine,
        &mut compositor,
        font_desc_set,
    ) {
        log::error!(
            "Error when initializing compositor sublayer definitions: {:?}",
            e
        );
        std::process::exit(1);
    }

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

    // let padding = 2f32;

    let mut debug_layers =
        DebugLayers::new(&mut engine, &mut compositor, "debug", 200)?;

    if let Err(e) = compositor.allocate_sublayers(&mut engine) {
        log::error!("Compositor error: {:?}", e);
    }

    let debug_layer_id = 0usize;
    let debug_layer_fps_id = debug_layers.create_layer(&mut compositor, 210)?;

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

    let mut text_cache = TextCache::new(&mut engine, &compositor)?;

    {
        use glyph_brush::{
            ab_glyph::FontArc, BrushAction, BrushError, GlyphBrushBuilder,
            HorizontalAlign, Layout, Section, Text,
        };

        let t0 = Text::new("this is some text at 16px");
        let t1 = Text::new("hello world at 20px!!").with_scale(20.0);
        // let t2 = Text::new("24px: ABCDEFGHIJKLMNOPQRSTUVWXYZ").with_scale(24.0);
        let t2 = Text::new("24px: abcdefghijkhlmnopqrstuvxyz").with_scale(24.0);

        let s0 = Section::default()
            .with_screen_position((100.0, 100.0))
            // .with_layout(Layout::default().h_align(HorizontalAlign::Center))
            // .with_bounds((80.0, 20.0))
            .add_text(t0);

        text_cache.queue(s0);

        // text_cache.process_queued(&mut engine, &mut compositor)?;

        log::error!("AND AGAIN");

        let s1 = Section::default()
            .with_screen_position((100.0, 200.0))
            .add_text(t1);

        let s2 = Section::default()
            .with_screen_position((100.0, 250.0))
            .add_text(t2);
        // .add_text(Text::new("yeah hello world!!!!!"));

        text_cache.queue(s1);
        text_cache.queue(s2);

        text_cache.process_queued(&mut engine, &mut compositor)?;
    }

    text_cache.upload_data(&mut engine)?;
    dbg!();

    let mut verlet = VerletSolver::new(width, height);

    waragraph::geometry::dynamics::verlet::add_test_data(&mut verlet);

    let mut prev_frame = std::time::Instant::now();

    let start_time = std::time::Instant::now();

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

    let mut last_fps = 0;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                let delta_time = prev_frame.elapsed().as_secs_f32();

                if let Err(e) = compositor.allocate_sublayers(&mut engine) {
                    log::error!("Compositor error: {:?}", e);
                }

                if let Err(e) =
                    text_cache.update_layer(&mut compositor, "command-palette", "glyphs") {
                    log::error!("Text layer update error: {:?}", e);
                }

                {
                    let [width, height] = swapchain_dims.load();
                    verlet.bounds =
                        ScreenRect::new(point2(50.0, 50.0),
                                        size2((width - 100) as f32,
                                              (height - 100) as f32));
                }

                if start_time.elapsed().as_secs_f32() > 2.0 {
                    verlet.update(delta_time);
                }

                if let Err(e) = verlet.update_layer(&mut compositor, "verlet") {
                    log::error!("Verlet layer update error: {:?}", e);
                }

                if let Err(e) = compositor.write_layers(&mut engine.resources) {
                    log::error!("Compositor error: {:?}", e);
                }

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




                {
                    if let Some(state) = viewer.path_viewer.ui_state.hovered_path_row {
                        let path = state.path;

                        if graph.path_name(path).is_some() {
                            let rect = state.data_rect;

                            let x0 = rect.min_x();
                            let y0 = rect.max_y();

                            let view = viewer.view.load();

                            compositor.with_layer("edges", |layer| {
                                if let Some(sublayer) = layer.get_sublayer_mut("lines") {
                                    let draw_data =
                                        sublayer.draw_data_mut().next().unwrap();

                                    edge_cache.update_sublayer_data_with_path(
                                        &graph,
                                        path,
                                        view,
                                        x0 as u32,
                                        y0 as u32,
                                        rect.width() as u32,
                                        90,
                                        draw_data).unwrap();
                                }

                                Ok(())
                            }).unwrap();
                        }

                    }


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
                    use waragraph::viewer::debug::{Shape, Style};

                    let [_win_width, win_height] = swapchain_dims.load();

                    /*
                    use waragraph::geometry::LayoutElement;
                    let list_layout = viewer.path_viewer.list_layout.load();

                    let color = |r: f32, g, b| rgb::RGBA::new(r, g, b, 1.0);
                    let color_a = |r: f32, g, b, a| rgb::RGBA::new(r, g, b, a);

                    let partition = 8.0 * 14.0;
                    let rows = viewer.path_viewer.slot_list.visible_rows();

                    let bg_rect = list_layout.inner_rect();

                    let (mx, my) = waragraph::input::get_mouse_pos();
                    let mpos = point2(mx as f32, my as f32);

                    let mouse_rect = list_layout
                        .apply_to_rows(rows)
                        .find_map(|(i, r, v)| {
                        let [_, r] = r.split_hor(partition);
                        r.contains(mpos)
                            .then(||
                                  (Shape::from(r),
                                   Style::fill(
                                       rgb::RGBA::new(1.0, 0.0, 0.0, 1.0))))

                    });

                    let rows = viewer.path_viewer.slot_list.visible_rows();

                    // let bg = Some((Shape::from(bg_rect), Style::fill(color_a(0.0, 0.0, 1.0, 0.6))));
                    let bg: Option<(Shape, Style)> = None;

                    let shapes = list_layout.apply_to_rows(rows).flat_map(|(_, r, v)| {

                        let c0 = color_a(0.7, 0.1, 0.1, 0.3);
                        let c1 = color_a(0.1, 0.7, 0.1, 0.3);
                        let c2 = color_a(0.8, 0.8, 0.8, 0.7);

                        let [r0, r1] = r.split_hor(partition);

                        let mouse_color = |r: ScreenRect, c| {
                            r.contains(mpos).then(|| c2).unwrap_or(c)
                        };

                        let col_0 = mouse_color(r0, c0);
                        let col_1 = mouse_color(r1, c1);

                        [
                            (
                            Shape::from(r0),
                            Style::stroke_fill(
                                color(1.0, 0.0, 0.0), col_0
                            ),
                        ),
                            (
                            Shape::from(r1),
                            Style::stroke_fill(
                                color(0.0, 1.0, 0.0), col_1
                            ),
                        )]

                    });


                    debug_layers.fill_layer(
                        &mut compositor,
                        debug_layer_id,
                        bg
                            .into_iter()
                            .chain(shapes)
                            .chain(mouse_rect)
                    ).unwrap();
                    */

                    let fps_str = last_fps.to_string();

                    let r: ScreenRect = euclid::rect(4.0,
                                                     win_height as f32 - 12.0,
                                                     8.0 * fps_str.len() as f32,
                                                     8.0);

                    let shapes = [
                        (Shape::label(r.origin.x, r.origin.y, &fps_str),
                         Style::stroke(rgb::RGBA::new(0.0, 0.0, 0.0, 1.0)))
                    ];

                    debug_layers.fill_layer(
                        &mut compositor,
                        debug_layer_fps_id,
                        shapes
                    ).unwrap();

                    debug_layers.update(&mut engine).unwrap();
                }


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
                        swapchain_dims.load(),
                        &graph,
                        &mut viewer.label_space,
                        &slot_fns,
                        &viewer.config,
                        &viewer.props,
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


                // prepare the path slot sublayer buffers
                if let Err(e) = compositor.with_layer("path-slots", |layer| {
                    let mouse_pos = {
                        let (x, y) = waragraph::input::get_mouse_pos();
                        point2(x as f32, y as f32)
                    };

                    let slot_fns = viewer.slot_functions.read();
                    viewer.path_viewer.update_slot_sublayer(
                        &graph,
                        &mut viewer.label_space,
                        layer,
                        &viewer.config,
                        &slot_fns,
                        &buffers,
                        mouse_pos,
                    )?;
                    Ok(())
                }) {
                    log::warn!("path sublayer update error: {:?}", e);
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

                    let fps = (1.0 / ft) as usize;
                    last_fps = fps;


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
