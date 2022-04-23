use bstr::ByteSlice;
use parking_lot::RwLock;
use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, DescSetIx, FrameResources, GpuResources, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::FxHashMap;
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};
use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use super::gui::GuiSys;
use super::{PathViewer, SlotFnCache, SlotUpdateFn};

pub struct ViewerSys {
    pub config: ConfigMap,

    pub view: ViewDiscrete1D,

    pub path_viewer: PathViewer,

    pub slot_functions: Arc<RwLock<SlotFnCache>>,

    pub labels: LabelStorage,
    pub label_updates: sled::Subscriber,

    // buffers: BufferStorage,
    pub frame_resources: [FrameResources; 2],
    pub frame: FrameBuilder,
    pub rhai_module: Arc<rhai::Module>,

    pub on_resize: RhaiBatchFn2<i64, i64>,

    pub draw_foreground: RhaiBatchFn4<BatchBuilder, rhai::Array, i64, i64>,
    pub copy_to_swapchain:
        Arc<RhaiBatchFn5<BatchBuilder, DescSetIx, rhai::Map, i64, i64>>,

    key_binds: Arc<RwLock<FxHashMap<VirtualKeyCode, rhai::FnPtr>>>,
    engine: rhai::Engine,

    pub slot_rhai_module: Arc<rhai::Module>,
}

impl ViewerSys {
    pub fn init(
        engine: &mut VkEngine,
        waragraph: &Arc<Waragraph>,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        window_resources: &mut WindowResources,
        gui: &GuiSys,
        width: u32,
        // height: u32,
    ) -> Result<Self> {
        let mut txt = LabelStorage::new(&db)?;

        let label_updates = txt.tree.watch_prefix(b"t:");

        let key_binds: FxHashMap<VirtualKeyCode, rhai::FnPtr> =
            Default::default();
        let key_binds = Arc::new(RwLock::new(key_binds));

        let bind_map = key_binds.clone();

        let bind_key_closure = move |key: VirtualKeyCode, val: rhai::FnPtr| {
            log::warn!("binding {:?} -> {:?}", key, val);
            bind_map.write().insert(key, val);
        };

        let slot_fns = Arc::new(RwLock::new(SlotFnCache::default()));

        let slot_module = {
            let mut module = crate::console::data::create_rhai_module();
            crate::console::data::add_cache_fns(&mut module, &slot_fns);
            Arc::new(module)
        };

        let mut builder =
            FrameBuilder::from_script_with("paths.rhai", |engine| {
                crate::console::register_buffer_storage(db, buffers, engine);
                crate::console::append_to_engine(db, engine);

                engine.register_static_module("slot", slot_module.clone());

                engine.register_fn("bind_key", bind_key_closure.clone());
            })?;

        let config = builder.module.get_var_value::<ConfigMap>("cfg").unwrap();

        log::warn!("Config: {:?}", config);

        let arc_module = Arc::new(builder.module.clone());

        // kind of a temporary hack; the console should be a fully
        // separate system, but right now it's using the viewer label
        // system for rendering
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

        // prefix sum loop count

        {
            let graph = &waragraph;

            // let mut cache_vec: Vec<BTreeMap<Node, usize>> = Vec::new();
            let mut cache_vec: Vec<Vec<(Node, usize)>> = Vec::new();

            for path in graph.paths.iter() {
                let mut sum = 0usize;
                let mut cache = Vec::new();

                for (node_ix, val) in path.iter() {
                    let len = graph.node_lens[node_ix] as usize;
                    let val = len * (*val as usize);
                    cache.push(((node_ix as u32).into(), sum));
                    sum += val;
                }

                cache_vec.push(cache);
            }

            slot_fns.write().register_data_source_u32(
                "prefix-sum:loop-count",
                move |path, node| {
                    let cache = cache_vec.get(path)?;

                    let ix = cache
                        .binary_search_by_key(&node, |(n, _)| *n)
                        .unwrap_or_else(|x| x);

                    let (_, v) = *cache.get(ix)?;
                    Some(v as u32)
                },
            );

            let mut cache_vec: Vec<Vec<(Node, usize)>> = Vec::new();

            for path in graph.paths.iter() {
                let mut sum = 0usize;
                let mut cache = Vec::new();

                for (node_ix, _val) in path.iter() {
                    let len = graph.node_lens[node_ix] as usize;
                    let val = len;
                    cache.push(((node_ix as u32).into(), sum));
                    sum += val;
                }

                cache_vec.push(cache);
            }

            let graph = waragraph.clone();
            slot_fns.write().register_data_source_u32(
                "prefix-sum:node-len",
                move |path, node| {
                    let path_len = graph.path_lens[path];

                    let cache = cache_vec.get(path)?;

                    let ix = cache
                        .binary_search_by_key(&node, |(n, _)| *n)
                        .unwrap_or_else(|x| x);

                    let (_, v) = *cache.get(ix)?;

                    let val = (v as f32) / (path_len as f32);

                    Some((val * 255.0) as u32)
                },
            );
        }

        //

        let graph = waragraph.clone();
        slot_fns.write().register_data_source_u32(
            "loop_count",
            move |path, node| {
                let path = graph.paths.get(path)?;
                path.get(node.into()).copied()
            },
        );

        let graph = waragraph.clone();
        slot_fns.write().register_data_source_u32(
            "has_node",
            move |path, node| {
                let path = graph.paths.get(path)?;
                path.get(node.into()).map(|_| 1)
            },
        );

        // using a Rhai function for the final step in mapping values to color indices
        let mut rhai_engine =
            Self::create_engine(db, buffers, &arc_module, &slot_module);
        rhai_engine.set_optimization_level(rhai::OptimizationLevel::Full);

        let color_map = rhai::Func::<(f32,), i64>::create_from_ast(
            rhai_engine,
            builder.ast.clone_functions_only(),
            "value_color_index_map",
        );
        let color_map = Arc::new(move |v| {
            let i = (&color_map)(v).unwrap();
            i as u32
        })
            as Arc<dyn Fn(f32) -> u32 + Send + Sync + 'static>;

        let cmap = color_map.clone();

        let slot_fn_loop = slot_fns
            .write()
            .slot_fn_prefix_sum_mean_u32(
                waragraph,
                "loop_count",
                "prefix-sum:loop-count",
                move |v| (&cmap)(v),
            )
            .unwrap();

        slot_fns
            .write()
            .slot_fn_u32
            .insert("loop_count_mean".into(), slot_fn_loop);

        // let graph = waragraph.clone();
        let slot_fn_loop = slot_fns
            .write()
            .slot_fn_mid_u32("prefix-sum:node-len", |v| v)
            .unwrap();

        slot_fns
            .write()
            .slot_fn_u32
            .insert("node_length".into(), slot_fn_loop);

        let cmap = color_map.clone();
        let slot_fn_loop_mid = slot_fns
            .read()
            .slot_fn_mid_u32("loop_count", move |v| (&cmap)(v as f32))
            .unwrap();

        slot_fns
            .write()
            .slot_fn_u32
            .insert("loop_count_mid".into(), slot_fn_loop_mid);

        ////

        //
        let view = ViewDiscrete1D::new(waragraph.total_len());

        let slot_count = 16;

        let mut path_viewer = PathViewer::new(
            engine,
            &db,
            &mut txt,
            width as usize,
            slot_count,
            waragraph.paths.len(),
        )?;

        path_viewer.sample(waragraph, &view);

        Self::update_labels_impl(&config, &txt, waragraph, &path_viewer);
        // path_viewer.update_labels(&waragraph, &txt)?;

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

        builder.bind_var("out_image", out_image)?;
        builder.bind_var("out_image_view", out_view)?;
        builder.bind_var("out_desc_set", out_desc_set)?;

        engine.with_allocators(|ctx, res, alloc| {
            builder.resolve(ctx, res, alloc)?;
            Ok(())
        })?;

        // gradient buffers
        [
            ("gradient_rainbow", colorous::RAINBOW),
            ("gradient_cubehelix", colorous::CUBEHELIX),
            ("gradient_blue_purple", colorous::BLUE_PURPLE),
            ("gradient_magma", colorous::MAGMA),
            ("gradient_inferno", colorous::INFERNO),
            ("gradient_spectral", colorous::SPECTRAL),
        ]
        .into_iter()
        .for_each(|(n, g)| {
            create_gradient_buffer(engine, buffers, &db, n, g, 256)
                .expect("error creating gradient buffers");
        });

        // main draw function
        let draw_foreground = rhai::Func::<
            (BatchBuilder, rhai::Array, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine(db, buffers, &arc_module, &slot_module),
            builder.ast.clone_functions_only(),
            "foreground",
        );

        let copy_to_swapchain = rhai::Func::<
            (BatchBuilder, DescSetIx, rhai::Map, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine(db, buffers, &arc_module, &slot_module),
            builder.ast.clone_functions_only(),
            "copy_to_swapchain",
        );

        {
            let init = rhai::Func::<(), BatchBuilder>::create_from_ast(
                Self::create_engine(db, buffers, &arc_module, &slot_module),
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
            let resize =
                rhai::Func::<(i64, i64), BatchBuilder>::create_from_ast(
                    Self::create_engine(db, buffers, &arc_module, &slot_module),
                    builder.ast.clone_functions_only(),
                    "resize",
                );
            resize
        };

        let frame_resources = {
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

        let engine =
            Self::create_engine(db, buffers, &arc_module, &slot_module);

        Ok(Self {
            config,
            view,

            path_viewer,

            slot_functions: slot_fns,

            labels: txt,
            label_updates,

            // buffers,
            frame_resources,
            frame: builder,
            rhai_module: arc_module,

            on_resize,

            draw_foreground,
            copy_to_swapchain: Arc::new(copy_to_swapchain),

            key_binds,
            engine,

            slot_rhai_module: slot_module,
        })
    }

    pub fn queue_slot_updates(
        &mut self,
        graph: &Arc<Waragraph>,
        update_tx: &crossbeam::channel::Sender<(
            Arc<Vec<(Node, usize)>>,
            SlotUpdateFn<u32>,
            usize,
            (usize, usize),
            usize,
        )>,
    ) -> Result<()> {
        let slot_fns = self.slot_functions.read();

        let samples = Arc::new(self.path_viewer.sample_buf.clone());

        let update_key = self
            .config
            .map
            .read()
            .get("viz.slot_function")
            .and_then(|v| v.clone().into_immutable_string().ok())
            .unwrap_or_else(|| "unknown".into());

        let def = slot_fns
            .slot_fn_u32
            .get("loop_count_mean")
            .ok_or(anyhow!("default slot renderer not found"))?;

        let slot_fn =
            slot_fns.slot_fn_u32.get(&update_key).unwrap_or_else(|| {
                log::warn!("slot renderer `{}` not found", update_key);
                def
            });

        let paths = self.path_viewer.visible_paths(graph);

        let view = self.view;
        let cur_view = Some((view.offset, view.len));

        let view = (view.offset, view.len);

        for path in paths {
            if let Some(slot) = self.path_viewer.slots.get_slot_mut_for(path) {
                if !slot.updating.load()
                    && (slot.view != cur_view
                        || slot.width != Some(self.path_viewer.width))
                {
                    let msg = (
                        samples.clone(),
                        slot_fn.to_owned(),
                        path,
                        view,
                        self.path_viewer.width,
                    );

                    slot.width = Some(self.path_viewer.width);
                    slot.view = Some(view);
                    update_tx.send(msg)?;
                    slot.updating.store(true);
                }
            }
        }

        Ok(())
    }

    pub fn handle_input(&mut self, event: &winit::event::WindowEvent<'_>) {
        use winit::event::{VirtualKeyCode, WindowEvent};

        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(kc) = input.virtual_keycode {
                    use VirtualKeyCode as VK;

                    let view = &mut self.view;

                    let pre_len = view.len();
                    let len = view.len() as isize;

                    let mut update = false;

                    let pressed = matches!(
                        input.state,
                        winit::event::ElementState::Pressed
                    );

                    if let Some(fn_ptr) = self.key_binds.read().get(&kc) {
                        let result: RhaiResult = fn_ptr.call(
                            &self.engine,
                            &self.frame.ast,
                            (rhai::Dynamic::from(pressed),),
                        );

                        if let Err(e) = result {
                            log::error!("bound key error: {:?}", e);
                        }
                    }

                    if pressed {
                        if matches!(kc, VK::Left) {
                            view.translate(-len / 10);
                            update = true;
                            assert_eq!(pre_len, view.len());
                        } else if matches!(kc, VK::Right) {
                            view.translate(len / 10);
                            update = true;
                            assert_eq!(pre_len, view.len());
                        } else if matches!(kc, VK::Up) {
                            if view.len() > self.path_viewer.width {
                                view.resize((len - len / 9) as usize);
                            }
                            view.len = view.len.max(self.path_viewer.width);
                            update = true;
                        } else if matches!(kc, VK::Down) {
                            view.resize((len + len / 10) as usize);
                            update = true;
                        } else if matches!(kc, VK::Escape) {
                            view.reset();
                            update = true;
                        } else if matches!(kc, VK::PageUp) {
                            self.path_viewer.scroll_up();
                            update = true;
                        } else if matches!(kc, VK::PageDown) {
                            self.path_viewer.scroll_down();
                            update = true;
                        }
                    }

                    self.path_viewer.update.fetch_or(update);
                }
            }
            _ => (),
        }
    }

    pub fn visible_slot_count(
        &self,
        graph: &Waragraph,
        window_height: u32,
    ) -> usize {
        let map = self.config.map.read();
        let get_cast = |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();
        let padding = map.get("layout.padding").unwrap().clone_cast::<i64>();
        let slot = map.get("layout.slot").unwrap().clone_cast::<rhai::Map>();

        let win_h = window_height as usize;
        let y = get_cast(&slot, "y") as usize;
        let slot_h = (get_cast(&slot, "h") + padding) as usize;

        let count = (win_h - y) / slot_h;

        let path_count = graph.path_names.len();

        count.min(path_count)
    }

    fn update_labels_impl(
        config: &ConfigMap,
        labels: &LabelStorage,
        waragraph: &Arc<Waragraph>,
        path_viewer: &PathViewer,
    ) {
        let map = config.map.read();

        let padding = map.get("layout.padding").unwrap().clone_cast::<i64>();
        let slot = map.get("layout.slot").unwrap().clone_cast::<rhai::Map>();
        let label = map.get("layout.label").unwrap().clone_cast::<rhai::Map>();

        let get_cast = |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();

        let label_x = get_cast(&label, "x");
        let label_y = get_cast(&label, "y") + get_cast(&slot, "y");

        let name_len = get_cast(&map, "layout.max_path_name_len");

        let slot_x = get_cast(&slot, "x") + label_x + padding + name_len * 8;

        labels
            .set_label_pos(b"view:start", slot_x as u32, 16)
            .unwrap();

        let h = get_cast(&slot, "h");

        let y_delta = padding + h;

        path_viewer
            .update_labels(
                waragraph,
                labels,
                [label_x as u32, label_y as u32],
                y_delta as u32,
                name_len as u8,
            )
            .unwrap();
    }

    pub fn update_labels(&self, waragraph: &Arc<Waragraph>) {
        Self::update_labels_impl(
            &self.config,
            &self.labels,
            waragraph,
            &self.path_viewer,
        )
    }

    pub fn resize(
        &mut self,
        waragraph: &Arc<Waragraph>,
        engine: &mut VkEngine,
        window_resources: &mut WindowResources,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let res_builder =
            window_resources.build(engine, width, height).unwrap();

        let slot_width = {
            let map = self.config.map.read();

            let padding =
                map.get("layout.padding").unwrap().clone_cast::<i64>();
            let slot =
                map.get("layout.slot").unwrap().clone_cast::<rhai::Map>();
            let label =
                map.get("layout.label").unwrap().clone_cast::<rhai::Map>();

            let get_cast =
                |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();

            let name_len = get_cast(&map, "layout.max_path_name_len");

            let slot_x = get_cast(&slot, "x")
                + get_cast(&label, "x")
                + padding
                + name_len * 8;

            let w = get_cast(&slot, "w");

            let width = width as i64;

            let slot_w = if w < 0 { width + w - slot_x } else { w };

            slot_w.max(0) as usize
        };

        engine
            .with_allocators(|ctx, res, alloc| {
                res_builder.insert(
                    &mut window_resources.indices,
                    ctx,
                    res,
                    alloc,
                )?;

                self.path_viewer.resize(ctx, res, alloc, slot_width, 0u32)?;

                Ok(())
            })
            .unwrap();

        {
            let slot_width = self.path_viewer.width;

            if self.view.len < slot_width {
                self.view.len = slot_width;
            }

            // txt.set_label_pos(b"view:start", 20, 16)?;
            let len_len = self.labels.label_len(b"view:len").unwrap();
            let end_len = self.labels.label_len(b"view:end").unwrap();
            let end_label_x = slot_width - (end_len * 8) - 40;
            let len_label_x = (end_label_x / 2) - len_len / 2;
            self.labels
                .set_label_pos(b"view:len", len_label_x as u32, 16)
                .unwrap();
            self.labels
                .set_label_pos(b"view:end", end_label_x as u32, 16)
                .unwrap();

            self.labels
                .set_label_pos(b"fps", 0, (height - 12) as u32)
                .unwrap();

            self.path_viewer.sample(waragraph, &self.view);
        }

        {
            let mut init_builder =
                (&self.on_resize)(width as i64, height as i64).unwrap();

            if !init_builder.init_fn.is_empty() {
                log::warn!("submitting update batches");
                let fence = engine
                    .submit_batches_fence(init_builder.init_fn.as_slice())
                    .unwrap();

                engine.block_on_fence(fence).unwrap();

                engine
                    .with_allocators(|c, r, a| {
                        init_builder.free_staging_buffers(c, r, a)
                    })
                    .unwrap();
            }
        }

        Ok(())
    }

    pub fn render(
        &mut self,
        engine: &mut VkEngine,
        buffers: &BufferStorage,
        window: &Window,
        window_resources: &WindowResources,
        graph: &Waragraph,
        gui: &GuiSys,
    ) -> Result<bool> {
        let f_ix = engine.current_frame_number();

        let frame = &mut self.frame_resources[f_ix % raving::vk::FRAME_OVERLAP];

        let size = window.inner_size();

        let mut label_sets = Vec::new();
        let mut desc_sets = Vec::new();

        let create_label_map = |id: u64| {
            let map = self.labels.create_label_rhai_map(id).ok()?;
            Some(rhai::Dynamic::from_map(map))
        };

        label_sets.extend(
            ["console", "fps", "view:start", "view:len", "view:end"]
                .into_iter()
                .filter_map(|name| {
                    let id = self.labels.get_label_id(name.as_bytes())?;
                    create_label_map(id)
                }),
        );

        for path in self.path_viewer.visible_paths(graph) {
            use rhai::Dynamic as Dyn;

            let mut desc_map = rhai::Map::default();
            if let Some(slot) = self.path_viewer.slots.get_slot_for(path) {
                if slot.path == Some(path) {
                    {
                        // let label_name = format!("path-name-{}", slot);
                        if let Some(map) = create_label_map(slot.label_id) {
                            label_sets.push(map);
                        }
                    }

                    let slot_set = slot.slot.desc_set();
                    desc_map.insert("slot".into(), Dyn::from(slot_set));
                } else {
                    desc_map.insert("slot".into(), Dyn::UNIT);
                }
            }
            desc_sets.push(rhai::Dynamic::from(desc_map));
        }

        // desc_sets.extend(self.path_viewer.visible_paths(graph).map(|path| {
        // }));

        let batch_builder = BatchBuilder::default();
        let fg_batch = (&self.draw_foreground)(
            batch_builder,
            desc_sets.clone(),
            size.width as i64,
            size.height as i64,
        )
        .unwrap();
        let fg_batch_fn = fg_batch.build();

        let batch_builder = BatchBuilder::default();
        let mut builder = rhai::Dynamic::from(batch_builder);
        // let labels_batch = self.draw_labels_.call

        // self.draw_labels_.call_raw(context, this_ptr, arg_values)

        self.engine
            .call_fn_raw(
                &mut rhai::Scope::default(),
                &self.frame.ast,
                false,
                true,
                "draw_labels",
                Some(&mut builder),
                [
                    rhai::Dynamic::from_int(size.width as i64),
                    rhai::Dynamic::from_int(size.height as i64),
                    rhai::Dynamic::from_array(label_sets),
                ],
            )
            .unwrap();
        // let batch = self.draw_labels_.call_raw(engine, ast, args)

        // let labels_batch = (&self.draw_labels)(
        //     batch_builder,
        //     size.width as i64,
        //     size.height as i64,
        //     label_sets,
        // )
        // .unwrap();
        let labels_batch_fn = builder.cast::<BatchBuilder>().build();
        // let labels_batch_fn = labels_batch.build();

        let extent = vk::Extent2D {
            width: size.width,
            height: size.height,
        };

        let out_framebuffer =
            *window_resources.indices.framebuffers.get("out").unwrap();

        let gui_batch_fn = gui.draw(out_framebuffer, extent);

        let mut gui_label_sets = Vec::new();

        for layer_name in gui.layer_order.read().iter() {
            if let Some(layer) = gui.layers.read().get(layer_name) {
                layer
                    .labels
                    .values()
                    .filter_map(|label| {
                        let id = label.is_visible().then(|| label.label_id)?;
                        let map = gui.labels.create_label_rhai_map(id).ok()?;
                        Some(rhai::Dynamic::from_map(map))
                    })
                    .for_each(|map| gui_label_sets.push(map));
            }
        }

        let batch_builder = BatchBuilder::default();
        let mut builder = rhai::Dynamic::from(batch_builder);

        self.engine
            .call_fn_raw(
                &mut rhai::Scope::default(),
                &self.frame.ast,
                false,
                true,
                "draw_labels",
                Some(&mut builder),
                [
                    rhai::Dynamic::from_int(size.width as i64),
                    rhai::Dynamic::from_int(size.height as i64),
                    rhai::Dynamic::from_array(gui_label_sets),
                ],
            )
            .unwrap();

        let gui_labels_batch_fn = builder.cast::<BatchBuilder>().build();
        // let gui_labels_batch_fn = (&self.draw_labels)(
        //     batch_builder,
        //     size.width as i64,
        //     size.height as i64,
        //     gui_label_sets,
        // )
        // .unwrap()
        // .build();

        let fg_batch = Box::new(
            move |dev: &Device,
                  res: &GpuResources,
                  _input: &BatchInput,
                  cmd: vk::CommandBuffer| {
                fg_batch_fn(dev, res, cmd);
                labels_batch_fn(dev, res, cmd);
                gui_batch_fn(dev, res, cmd);
                gui_labels_batch_fn(dev, res, cmd);
            },
        ) as Box<_>;

        // let gui_batch = Box::new(
        //     move |dev: &Device,
        //           res: &GpuResources,
        //           _input: &BatchInput,
        //           cmd: vk::CommandBuffer| {
        //         // fg_batch_fn(dev, res, cmd);
        //         // labels_batch_fn(dev, res, cmd);
        //     },
        // ) as Box<_>;

        // let copy_to_swapchain = self.copy_to_swapchain.clone();

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

        let copy_to_swapchain = self.copy_to_swapchain.clone();

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
            Some(vec![(
                0,
                vk::PipelineStageFlags::COMPUTE_SHADER
                    | vk::PipelineStageFlags::ALL_GRAPHICS,
            )]),
        ];

        let result =
            engine.draw_from_batches(frame, &batches, deps.as_slice(), 1)?;

        Ok(result)
    }

    fn create_engine(
        db: &sled::Db,
        buffers: &BufferStorage,
        viewer_module: &Arc<rhai::Module>,
        slot_module: &Arc<rhai::Module>,
    ) -> rhai::Engine {
        let mut rhai_engine = crate::console::create_engine(db, buffers);
        rhai_engine.register_static_module("viewer", viewer_module.clone());
        rhai_engine.register_static_module("slot", slot_module.clone());
        rhai_engine
    }
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
