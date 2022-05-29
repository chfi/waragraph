use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::script::console::frame::{FrameBuilder, Resolvable};
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, BufferIx, DescSetIx, FrameResources, GpuResources, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::FxHashMap;
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::data::AnnotationSet;
use crate::console::{
    Console, EvalResult, RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5,
};
use crate::graph::{Node, Path, Waragraph};
use crate::util::{BufFmt, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use super::cache::{GpuBufferCache, UpdateReqMsg};
use super::{PathViewer, SlotFnCache, SlotUpdateFn};
use raving::compositor::{Compositor, Sublayer};

pub struct ViewerSys {
    pub config: ConfigMap,

    pub view: Arc<AtomicCell<ViewDiscrete1D>>,

    pub path_viewer: PathViewer,

    pub slot_functions: Arc<RwLock<SlotFnCache>>,

    pub labels: LabelStorage,
    pub label_updates: sled::Subscriber,

    // buffers: BufferStorage,
    pub frame_resources: [FrameResources; 2],
    pub frame: FrameBuilder,
    pub rhai_module: Arc<rhai::Module>,

    pub on_resize: RhaiBatchFn2<i64, i64>,

    pub draw_foreground: RhaiBatchFn5<BatchBuilder, rhai::Array, i64, i64, i64>,
    pub copy_to_swapchain:
        Arc<RhaiBatchFn5<BatchBuilder, DescSetIx, rhai::Map, i64, i64>>,

    key_binds: Arc<RwLock<FxHashMap<VirtualKeyCode, rhai::FnPtr>>>,
    pub engine: rhai::Engine,

    pub slot_rhai_module: Arc<rhai::Module>,

    // pub annotations: BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>,
    pub annotations:
        Arc<RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>>,

    pub new_viewer: PathViewerNew,
}

impl ViewerSys {
    pub fn init(
        engine: &mut VkEngine,
        waragraph: &Arc<Waragraph>,
        graph_module: &Arc<rhai::Module>,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        window_resources: &mut WindowResources,
        width: u32,
        // height: u32,
    ) -> Result<Self> {
        let mut txt = LabelStorage::new(&db)?;

        let slot_count = 16;
        let mut path_viewer = PathViewer::new(
            engine,
            &db,
            &mut txt,
            width as usize,
            slot_count,
            waragraph.paths.len(),
        )?;

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

        let annotations: Arc<
            RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>,
        > = Arc::new(RwLock::new(BTreeMap::default()));

        let slot_module = {
            let mut module = crate::console::data::create_rhai_module();
            crate::console::data::add_module_fns(
                &mut module,
                &slot_fns,
                &annotations,
            );

            let force_update = path_viewer.force_update_fn();
            module.set_native_fn("force_update", move || {
                force_update();
                Ok(())
            });

            let row_view = path_viewer.row_view.clone();
            module.set_native_fn("list_range", move || {
                let (offset, len) = row_view.load();
                let o = offset as i64;
                let l = len as i64;
                Ok(o..l)
            });

            let slot_cache = path_viewer.slots.clone();

            module.set_native_fn("get_path_in_slot", move |slot_ix: i64| {
                let cache = slot_cache.read();

                if let Some(path) =
                    cache.slots.get(slot_ix as usize).and_then(|s| s.path)
                {
                    Ok(rhai::Dynamic::from(path))
                } else {
                    Ok(rhai::Dynamic::FALSE)
                }
            });

            Arc::new(module)
        };

        let view = Arc::new(AtomicCell::new(ViewDiscrete1D::new(
            waragraph.total_len(),
        )));

        let mut builder =
            FrameBuilder::from_script_with("paths.rhai", |engine| {
                crate::console::register_buffer_storage(db, buffers, engine);
                crate::console::append_to_engine(db, engine);

                engine.register_static_module("slot", slot_module.clone());
                engine.register_static_module("graph", graph_module.clone());

                engine.register_fn("bind_key", bind_key_closure.clone());
            })?;

        let config = builder.module.get_var_value::<ConfigMap>("cfg").unwrap();

        {
            let view_ = view.clone();
            builder
                .module
                .set_native_fn("get_view", move || Ok(view_.load()));

            let view_ = view.clone();
            let should_update = path_viewer.force_update_cell().clone();
            builder.module.set_native_fn(
                "set_view",
                move |new: ViewDiscrete1D| {
                    if new != view_.load() {
                        should_update.store(true);
                    }
                    view_.store(new);
                    Ok(())
                },
            );

            let should_update = path_viewer.force_update_cell().clone();
            let view_ = view.clone();
            builder.module.set_raw_fn(
                "with_view",
                rhai::FnNamespace::Global,
                rhai::FnAccess::Public,
                [std::any::TypeId::of::<rhai::FnPtr>()],
                move |ctx, args| {
                    let fn_ptr = std::mem::take(args[0]).cast::<rhai::FnPtr>();

                    let old_view = view_.load();
                    let mut view = rhai::Dynamic::from(old_view);

                    if let Err(e) = fn_ptr.call_raw(&ctx, Some(&mut view), []) {
                        return Err(e);
                    }

                    if let Some(view) = view.try_cast::<ViewDiscrete1D>() {
                        if old_view != view {
                            should_update.store(true);
                            view_.store(view);
                        }
                        Ok(())
                    } else {
                        return Err(
                            "Function pointer changed type of view".into()
                        );
                    }
                },
            );
        }

        // builder.module.set_native_fn(name, func)

        log::warn!("Config: {:?}", config);

        let arc_module = Arc::new(builder.module.clone());

        // kind of a temporary hack; the console should be a fully
        // separate system, but right now it's using the viewer label
        // system for rendering
        // txt.allocate_label(&db, engine, "console")?;
        // txt.set_label_pos(b"console", 4, 4)?;
        // txt.set_text_for(b"console", "")?;

        txt.allocate_label(&db, engine, "fps")?;
        txt.set_label_pos(b"fps", 0, 580)?;

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
                "prefix-sum:depth",
                move |path, node| {
                    let cache = cache_vec.get(path.ix())?;

                    let ix = cache
                        .binary_search_by_key(&node, |(n, _)| *n)
                        .unwrap_or_else(|x| x);

                    let (_, v) = *cache.get(ix)?;
                    Some(v as u32)
                },
            );

            let cache_vec = waragraph.path_sum_lens.clone();
            // let graph = waragraph.clone();
            slot_fns.write().register_data_source_u32(
                "path-position",
                move |path, node| {
                    let ix = usize::from(path);
                    // let node_ix = usize::from(node);
                    // let path_len = graph.path_lens[ix];

                    let cache = cache_vec.get(ix)?;

                    let ix =
                        cache.binary_search_by_key(&node, |(n, _)| *n).ok()?;
                    // .unwrap_or_else(|x| x);

                    let (_, v) = *cache.get(ix)?;
                    Some(v as u32)
                },
            );
        }

        //

        let graph = waragraph.clone();
        slot_fns.write().register_data_source_u32(
            "depth",
            move |path, node| {
                let path = graph.paths.get(path.ix())?;
                path.get(node.into()).copied()
            },
        );

        let graph = waragraph.clone();
        slot_fns.write().register_data_source_u32(
            "has_node",
            move |path, node| {
                let path = graph.path_nodes.get(path.ix())?;
                path.contains(node.into()).then(|| 1)
            },
        );

        let graph = waragraph.clone();
        slot_fns.write().register_data_source_u32(
            "node-id",
            move |path, node| {
                let path = graph.path_nodes.get(path.ix())?;
                let node: u32 = node.into();
                path.contains(node).then(|| node)
            },
        );

        // using a Rhai function for the final step in mapping values to color indices
        let mut rhai_engine =
            Self::create_engine_impl(db, buffers, &arc_module, &slot_module);
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
            .read()
            .slot_fn_prefix_sum_mean_u32(
                waragraph,
                "depth",
                "prefix-sum:depth",
                move |v| (&cmap)(v),
            )
            .unwrap();

        slot_fns
            .write()
            .slot_fn_u32
            .insert("depth_mean".into(), slot_fn_loop);

        let graph = waragraph.clone();
        let slot_fn_pos = slot_fns
            .read()
            .slot_fn_reduce_u32(
                waragraph,
                "path-position",
                |acc, val| acc + val,
                move |path, val| {
                    if let Some(path_len) = graph.path_lens.get(path.ix()) {
                        let len = *path_len as f64;
                        let t = val / len;
                        let v = (t * 255.0) as u32;
                        v
                    } else {
                        0
                    }
                },
            )
            .unwrap();
        /*
        let graph = waragraph.clone();
        let slot_fn_pos = slot_fns
            .read()
            .slot_fn_reduce_u32(
                waragraph,
                "path-position",
                |acc, val| acc + val,
                move |path, val| {
                    if let Some(path_len) = graph.path_lens.get(path.ix()) {
                        let len = *path_len as f64;
                        let t = val / len;
                        let v = (t * 255.0) as u32;
                        v
                    } else {
                        0
                    }
                },
            )
            .unwrap();
        */

        slot_fns
            .write()
            .slot_fn_u32
            .insert("path_position".into(), slot_fn_pos);

        slot_fns
            .write()
            .slot_color
            .insert("path_position".into(), "gradient-grayscale".into());

        let slot_fn_has_node = slot_fns
            .read()
            .slot_fn_reduce_u32(
                waragraph,
                "has_node",
                |acc, val| acc + val,
                |path, v| {
                    if v < 0.5 {
                        8
                    } else {
                        13
                    }
                },
            )
            .unwrap();

        slot_fns
            .write()
            .slot_fn_u32
            .insert("has_node".into(), slot_fn_has_node);

        slot_fns
            .write()
            .slot_color
            .insert("has_node".into(), "gui-palette".into());

        let slot_fn_id = slot_fns
            .read()
            .slot_fn_mid_u32("node-id", |v| {
                if v == 0 {
                    0
                } else {
                    let i = (v - 1) % 255;
                    1 + i
                }
            })
            .unwrap();

        slot_fns
            .write()
            .slot_fn_u32
            .insert("node_id".into(), slot_fn_id);

        slot_fns
            .write()
            .slot_color
            .insert("node_id".into(), "gradient-colorbrewer-spectral".into());

        let cmap = color_map.clone();
        let slot_fn_loop_mid = slot_fns
            .read()
            .slot_fn_mid_u32("depth", move |v| (&cmap)(v as f32))
            .unwrap();

        /*
        slot_fns
            .write()
            .slot_fn_u32
            .insert("depth_mid".into(), slot_fn_loop_mid);
        */

        ////

        path_viewer.sample(waragraph, &view.load());

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
            (BatchBuilder, rhai::Array, i64, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine_impl(db, buffers, &arc_module, &slot_module),
            builder.ast.clone_functions_only(),
            "foreground",
        );

        let copy_to_swapchain = rhai::Func::<
            (BatchBuilder, DescSetIx, rhai::Map, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine_impl(db, buffers, &arc_module, &slot_module),
            builder.ast.clone_functions_only(),
            "copy_to_swapchain",
        );

        {
            let init = rhai::Func::<(), BatchBuilder>::create_from_ast(
                Self::create_engine_impl(
                    db,
                    buffers,
                    &arc_module,
                    &slot_module,
                ),
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
                    Self::create_engine_impl(
                        db,
                        buffers,
                        &arc_module,
                        &slot_module,
                    ),
                    builder.ast.clone_functions_only(),
                    "resize",
                );
            resize
        };

        let frame_resources = {
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

        // let mut new_viewer = todo!();
        let new_viewer = PathViewerNew::new(
            engine,
            waragraph,
            view.load(),
            width as usize,
            "depth_mean".into(),
        )?;

        let engine =
            Self::create_engine_impl(db, buffers, &arc_module, &slot_module);

        Ok(Self {
            config,
            view,

            path_viewer,

            slot_functions: slot_fns,
            annotations,

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

            new_viewer,
        })
    }

    pub fn queue_slot_updates(
        &mut self,
        graph: &Arc<Waragraph>,
        update_tx: &crossbeam::channel::Sender<(
            Arc<Vec<(Node, usize)>>,
            rhai::ImmutableString,
            SlotUpdateFn<u32>,
            Path,
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
            .get("depth_mean")
            .ok_or(anyhow!("default slot renderer not found"))?;

        let slot_fn =
            slot_fns.slot_fn_u32.get(&update_key).unwrap_or_else(|| {
                log::warn!("slot renderer `{}` not found", update_key);
                def
            });

        let paths = self.path_viewer.visible_paths(graph);

        let view = self.view.load();
        let cur_view = Some((view.offset, view.len));

        let view = (view.offset, view.len);

        let mut slots = self.path_viewer.slots.write();
        for path in paths {
            // if let Some(slot) = self.path_viewer.slots.get_slot_mut_for(path) {
            if let Some(slot) = slots.get_slot_mut_for(path) {
                if !slot.updating.load()
                    && (slot.view != cur_view
                        || slot.slot_function != &update_key
                        || slot.width != Some(self.path_viewer.width))
                {
                    let msg = (
                        samples.clone(),
                        update_key.clone(),
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

    pub fn handle_input(
        &mut self,
        console: &Console,
        event: &winit::event::WindowEvent<'_>,
    ) {
        use winit::event::{VirtualKeyCode, WindowEvent};

        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(kc) = input.virtual_keycode {
                    use VirtualKeyCode as VK;

                    let mut view = self.view.load();

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
                            &console.ast,
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

                    self.view.store(view);

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

    pub fn slot_x_offsets(&self, win_width: u32) -> [f32; 2] {
        let map = self.config.map.read();

        let padding = map.get("layout.padding").unwrap().clone_cast::<i64>();
        let slot = map.get("layout.slot").unwrap().clone_cast::<rhai::Map>();
        let label = map.get("layout.label").unwrap().clone_cast::<rhai::Map>();

        let get_cast = |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();

        let label_x = get_cast(&label, "x");

        let name_len = get_cast(&map, "layout.max_path_name_len");

        let w = get_cast(&slot, "w");

        let slot_x = get_cast(&slot, "x") + label_x + padding + name_len * 8;

        let slot_w = if w < 0 {
            (win_width as i64) + w - slot_x
        } else {
            w
        };

        [slot_x as f32, slot_w as f32]
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

    pub fn create_queued_slot_fns(
        &mut self,
        db: &sled::Db,
        buffers: &BufferStorage,
        console: &Console,
    ) -> Result<()> {
        let queued =
            std::mem::take(&mut self.slot_functions.write().slot_fn_queue);

        for new in queued {
            let mut engine = console.create_engine(db, buffers);
            let ast = console.ast.clone();
            engine.set_optimization_level(rhai::OptimizationLevel::Full);

            let mut slot_fns = self.slot_functions.write();

            type DataFn = Box<
                dyn Fn(Path, Node) -> Option<rhai::Dynamic>
                    + Send
                    + Sync
                    + 'static,
            >;
            // type DataFn =
            // Box<dyn Fn(Path, Node) -> Option<rhai::Dynamic> + 'static>;

            let name = new.name.clone();

            let data_sources = new
                .data_sources
                .into_iter()
                .map(|(ty, name)| {
                    if ty == std::any::TypeId::of::<u32>() {
                        let f = slot_fns
                            .data_sources_u32
                            .get(&name)
                            .unwrap()
                            .clone();

                        Box::new(
                            move |path: Path, node: Node| -> Option<rhai::Dynamic> {
                                f(path, node).map(|i| rhai::Dynamic::from_int(i as i64))
                            },
                        ) as DataFn
                    } else if ty == std::any::TypeId::of::<f32>() {
                        let f = slot_fns
                            .data_sources_f32
                            .get(&name)
                            .unwrap()
                            .clone();
                        Box::new(
                            move |path: Path, node: Node| -> Option<rhai::Dynamic> {
                                f(path, node).map(rhai::Dynamic::from_float)
                            },
                        ) as DataFn
                    } else if ty == std::any::TypeId::of::<i64>() {
                        let f = slot_fns
                            .data_sources_i64
                            .get(&name)
                            .unwrap()
                            .clone();
                        Box::new(
                            move |path: Path, node: Node| -> Option<rhai::Dynamic> {
                                f(path, node).map(rhai::Dynamic::from_int)
                            },
                        )as DataFn
                    } else if ty == std::any::TypeId::of::<rhai::Dynamic>() {
                        let f = slot_fns
                            .data_sources_dyn
                            .get(&name)
                            .unwrap()
                            .clone();
                        Box::new(
                            move |path: Path, node: Node| -> Option<rhai::Dynamic> {
                                f(path, node)
                            },
                        )as DataFn
                    } else {
                        unreachable!();
                    }
                })
                .collect::<Vec<_>>();

            let slot_fn = move |samples: &[(Node, usize)],
                                path: Path,
                                sample_ix: usize| {
                let left_ix = sample_ix.min(samples.len() - 1);
                let right_ix = (sample_ix + 1).min(samples.len() - 1);

                let (left, _offset) = samples[left_ix];
                let (right, _offset) = samples[right_ix];

                let l: u32 = left.into();
                let r: u32 = right.into();

                let node = l + (r - l) / 2;

                let args = data_sources
                    .iter()
                    .map(|ds| {
                        if let Some(val) = ds(path, node.into()) {
                            val
                        } else {
                            rhai::Dynamic::UNIT
                        }
                    })
                    .collect::<Vec<_>>();

                let result: EvalResult<i64> =
                    new.fn_ptr.call(&engine, &ast, args);

                match result {
                    Ok(v) => v as u32,
                    Err(e) => {
                        log::error!("rhai data source error: {:?}", e);
                        0
                    }
                }
            };

            slot_fns.slot_fn_u32.insert(name, Arc::new(slot_fn));
        }

        Ok(())
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
            let mut view = self.view.load();

            if view.len < slot_width {
                view.len = slot_width;
                self.view.store(view);
            }

            self.labels
                .set_label_pos(b"fps", 0, (height - 12) as u32)
                .unwrap();

            self.path_viewer.sample(waragraph, &view);
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
        compositor: &Compositor,
    ) -> Result<bool> {
        let f_ix = engine.current_frame_number();

        let frame = &mut self.frame_resources[f_ix % raving::vk::FRAME_OVERLAP];

        let [width, height] = window_resources.dims();

        let mut label_sets = Vec::new();
        let mut desc_sets = Vec::new();

        let create_label_map = |id: u64| {
            let map = self.labels.create_label_rhai_map(id).ok()?;
            Some(rhai::Dynamic::from_map(map))
        };

        label_sets.extend(["fps"].into_iter().filter_map(|name| {
            let id = self.labels.get_label_id(name.as_bytes())?;
            create_label_map(id)
        }));

        let slots = self.path_viewer.slots.read();
        for path in self.path_viewer.visible_paths(graph) {
            use rhai::Dynamic as Dyn;

            let mut desc_map = rhai::Map::default();
            if let Some(slot) = slots.get_slot_for(path) {
                if slot.path == Some(path.into()) {
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

        let slot_buf_size = self.path_viewer.width;

        let batch_builder = BatchBuilder::default();
        let fg_batch = (&self.draw_foreground)(
            batch_builder,
            desc_sets.clone(),
            width as i64,
            height as i64,
            slot_buf_size as i64,
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
                    rhai::Dynamic::from_int(width as i64),
                    rhai::Dynamic::from_int(height as i64),
                    rhai::Dynamic::from_array(label_sets),
                ],
            )
            .unwrap();

        let labels_batch_fn = builder.cast::<BatchBuilder>().build();

        let extent = vk::Extent2D { width, height };

        let out_framebuffer =
            *window_resources.indices.framebuffers.get("out").unwrap();

        let comp_batch_fn = compositor.draw(None, out_framebuffer, extent);

        let fg_batch = Box::new(
            move |dev: &Device,
                  res: &GpuResources,
                  _input: &BatchInput,
                  cmd: vk::CommandBuffer| {
                fg_batch_fn(dev, res, cmd);
                labels_batch_fn(dev, res, cmd);
                comp_batch_fn(dev, res, cmd);
            },
        ) as Box<_>;

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
                    width as i64,
                    height as i64,
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

    fn create_engine_impl(
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

pub type SlotFnName = rhai::ImmutableString;

#[derive(Clone)]
pub struct ListView<T> {
    values: Vec<T>,
    offset: usize,
    len: usize,
    max: usize,
}

impl<T> ListView<T> {
    pub fn new(values: impl IntoIterator<Item = T>) -> Self {
        let values: Vec<_> = values.into_iter().collect();
        let max = values.len();
        let offset = 0;
        let len = 1.min(max);
        Self {
            values,
            max,
            offset,
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn visible_rows<'a>(&'a self) -> impl Iterator<Item = &'a T> + 'a {
        debug_assert!(self.offset + self.len <= self.max);
        debug_assert!(self.max == self.values.len());

        let s = self.offset;
        let e = s + self.len;
        self.values[s..e].iter()
    }

    pub fn set_offset(&mut self, mut offset: usize) {
        if offset + self.len > self.max {
            offset -= (offset + self.len) - self.max;
        }

        self.offset = offset;
        debug_assert!(self.offset + self.len <= self.max);
    }

    pub fn scroll(&mut self, delta: isize) {
        let mut offset = self.offset as isize;

        let max_offset = (self.max - self.len) as isize;
        offset = (offset + delta).clamp(0, max_offset);

        self.offset = offset as usize;
        debug_assert!(self.offset + self.len <= self.max);
    }

    pub fn resize(&mut self, new_len: usize) {
        self.len = new_len.min(self.max);
        // set_offset takes care of moving the offset back for the new
        // length if needed
        self.set_offset(self.offset);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotState {
    Unknown,
    Updating,
    Contains {
        buffer_width: usize,
        view_offset: usize,
        view_len: usize,
    },
}

impl std::default::Default for SlotState {
    fn default() -> Self {
        Self::Unknown
    }
}

pub struct PathViewerNew {
    cache: GpuBufferCache<(Path, SlotFnName)>,

    slot_list: ListView<(Path, SlotFnName)>,

    slot_states: HashMap<(Path, SlotFnName), Arc<AtomicCell<SlotState>>>,

    current_view: ViewDiscrete1D,
    current_width: usize,

    need_refresh: Arc<AtomicCell<bool>>,
}

impl PathViewerNew {
    pub fn new(
        engine: &mut VkEngine,
        graph: &Arc<Waragraph>,
        current_view: ViewDiscrete1D,
        current_width: usize,
        slot_fn_name: rhai::ImmutableString,
    ) -> Result<Self> {
        let slot_count = 1024;

        let usage = vk::BufferUsageFlags::STORAGE_BUFFER;

        let elem_size = 4;
        let block_size = current_width;
        let block_capacity = slot_count;

        let cache = GpuBufferCache::new(
            engine,
            usage,
            "Path Viewer Slot Cache",
            elem_size,
            block_size,
            block_capacity,
        )?;

        let slot_list = ListView::new(
            graph
                .path_names
                .left_values()
                .map(|&path| (path, slot_fn_name.clone())),
        );

        Ok(Self {
            cache,
            slot_list,
            slot_states: HashMap::default(),

            current_view,
            current_width,

            need_refresh: Arc::new(false.into()),
        })
    }

    pub fn need_refresh(&self) -> bool {
        self.need_refresh.load()
    }

    pub fn force_refresh(&self) {
        self.need_refresh.store(true);
    }

    pub fn need_reallocation(&self) -> bool {
        let needed_len = self.slot_list.len();
        let available = self.cache.cache().block_capacity();

        let block_size = self.cache.cache().block_size();

        (needed_len > available) || (block_size != self.current_width)
    }

    // pub fn bind_rows_alloc(&mut self,
    //                        engine: &mut VkEngine,

    // reallocate cache if needed, queue slot updates, and apply ready updates
    pub fn update(
        &mut self,
        engine: &mut VkEngine,
        slot_fns: &SlotFnCache,
        samples: Arc<Vec<(Node, usize)>>,
        update_tx: &crossbeam::channel::Sender<
            UpdateReqMsg<(Path, SlotFnName)>,
        >,
    ) -> Result<()> {
        // reallocate and invalidate cache if cache block size doesn't
        // match the width, or if the current slot list view size is
        // greater than the cache block capacity
        let slot_count = self.slot_list.len();

        if slot_count > self.cache.cache().block_capacity()
            || self.current_width != self.cache.cache().block_size()
        {
            self.cache.reallocate(
                engine,
                slot_count * 2,
                self.current_width,
            )?;
            self.slot_states.clear();
        }

        // make sure all entries in the slot list are bound in the cache
        self.cache
            .bind_blocks(self.slot_list.visible_rows().cloned())?;

        // for each visible (Path, SlotFnName) in the slot list

        let need_refresh = self.need_refresh();

        for key in self.slot_list.visible_rows() {
            // get the state cell for the slot, creating and inserting
            // a new cell with the Unknown state if the current key
            // doesn't have a state
            let cell = self.slot_states.entry(key.clone()).or_default().clone();

            let need_update = match cell.load() {
                // if it's currently updating, do nothing
                SlotState::Updating => false,
                SlotState::Unknown => true,
                SlotState::Contains {
                    buffer_width,
                    view_offset,
                    view_len,
                } => {
                    // if it's up to date with the current view and
                    // width, do nothing
                    buffer_width != self.current_width
                        || view_offset != self.current_view.offset
                        || view_len != self.current_view.len
                }
            };

            // if there is no entry, it is unknown, or the view or width
            // contained do not match,

            if need_update || need_refresh {
                let (path, slot_fn_name) = key;

                let path = *path;

                let slot_fn = slot_fns.slot_fn_u32.get(slot_fn_name).ok_or(
                    anyhow!("slot renderer `{}` not found", slot_fn_name),
                )?;

                let key = (path, slot_fn_name.clone());

                let samples = samples.clone();
                let slot_fn = slot_fn.clone();
                let width = self.cache.cache().block_size();

                // let slot_state_cell =

                let current_width = self.current_width;
                let current_view = self.current_view;
                let current_state = cell.load();

                let msg = UpdateReqMsg::new(
                    key,
                    // queue up a slot update with the current parameters and
                    // samples, for this entry
                    move |key| {
                        let mut buf: Vec<u8> =
                            Vec::with_capacity(samples.len());
                        buf.extend(
                            (0..width)
                                .map(|i| slot_fn(&samples, path, i))
                                .flat_map(|val| {
                                    let bytes: [u8; 4] = bytemuck::cast(val);
                                    bytes
                                }),
                        );

                        Ok(buf)
                    },
                    // set the update signal to update the SlotState with the
                    // provided parameters
                    move || {
                        let _ = cell.compare_exchange(
                            current_state,
                            SlotState::Contains {
                                buffer_width: current_width,
                                view_offset: current_view.offset(),
                                view_len: current_view.len(),
                            },
                        );
                    },
                );

                update_tx.send(msg)?;
            }
        }

        // apply the GPU cache data messages, which also updates the
        // slot_state entries, concurrently
        self.cache.apply_data_updates(&mut engine.resources)?;

        Ok(())
    }

    // update the sublayer's vertex data (must be a slot sublayer)
    // with the currently visible slots
    // slots that are in the process of being updated are skipped
    pub fn update_slot_sublayer(
        &self,
        sublayer: &mut Sublayer,
        window_dims: [u32; 2],
        config: &ConfigMap,
    ) -> Result<()> {
        let (x0, y0, w, h, yd) = {
            let map = config.map.read();
            let get_cast =
                |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();
            let padding =
                map.get("layout.padding").unwrap().clone_cast::<i64>();
            let slot =
                map.get("layout.slot").unwrap().clone_cast::<rhai::Map>();

            let win_h = window_dims[1] as usize;
            let y = get_cast(&slot, "y") as usize;
            let slot_h = (get_cast(&slot, "h") + padding) as usize;

            let label =
                map.get("layout.label").unwrap().clone_cast::<rhai::Map>();

            let get_cast =
                |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();

            let label_x = get_cast(&label, "x");

            let name_len = get_cast(&map, "layout.max_path_name_len");

            let w = get_cast(&slot, "w");

            let slot_y = get_cast(&slot, "y");
            let slot_x =
                get_cast(&slot, "x") + label_x + padding + name_len * 8;

            let slot_w = if w < 0 {
                (window_dims[0] as i64) + w - slot_x
            } else {
                w
            };

            (
                slot_x as f32,
                slot_y as f32,
                slot_w as f32,
                slot_h as f32,
                padding as f32 + slot_h as f32,
            )
        };

        let mut vertices: Vec<[u8; 20]> = Vec::new();

        for (ix, key) in self.slot_list.visible_rows().enumerate() {
            // if the state cell somehow doesn't exist, or shows that
            // there's probably only garbage data there, simply skip
            // this row (it'll get drawn when the data's ready)
            if let Some(state) = self.slot_states.get(key) {
                if state.load() == SlotState::Unknown {
                    continue;
                }
            } else {
                continue;
            }

            // otherwise prepare the vertex data

            let range = self.cache.cache().get_range(key).unwrap();

            let mut vx = [0u8; 20];

            let x = x0;
            let y = y0 + ix as f32 * yd;

            vx[0..8].clone_from_slice([x, y].as_bytes());
            vx[8..16].clone_from_slice([w, h].as_bytes());
            vx[16..24].clone_from_slice(
                [range.start as u32, range.end as u32].as_bytes(),
            );

            vertices.push(vx);
        }

        sublayer.draw_data_mut().try_for_each(|data| {
            data.update_vertices_array(vertices.iter().copied())
        })?;

        Ok(())
    }

    /*
    pub fn prepare_buffers(
        &mut self,
        engine: &mut VkEngine,
        graph: &Arc<Waragraph>,
        samples: Arc<Vec<(Node, usize)>>,
    ) -> Result<()> {
        // reallocate the cache and backing memory if needed

        // bind the visible rows

        // update the newly bound rows (or all rows, if a refresh is forced)

        todo!();
    }

    pub fn update_slot_sublayer(
        &mut self,
        // prob. only need resources
        engine: &mut VkEngine,
        sublayer: &mut Sublayer,
    ) -> Result<()> {
        // update the sublayer's vertex data (must be a slot sublayer)
        // with the currently visible slots

        // slots that haven't been written to yet are skipped
        todo!();
    }
    */

    fn queue_slot_updates<'a>(
        &'a self,
        keys_to_update: impl Iterator<Item = &'a (Path, SlotFnName)>,
        slot_fns: &SlotFnCache,
        samples: Arc<Vec<(Node, usize)>>,
        graph: &Arc<Waragraph>,
    ) -> Result<()> {
        /*
        let update_key = self
            .config
            .map
            .read()
            .get("viz.slot_function")
            .and_then(|v| v.clone().into_immutable_string().ok())
            .unwrap_or_else(|| "unknown".into());
        */

        for (path, slot_fn_name) in keys_to_update {
            let path = *path;

            let slot_fn = slot_fns
                .slot_fn_u32
                .get(slot_fn_name)
                .ok_or(anyhow!("slot renderer `{}` not found", slot_fn_name))?;

            let key = (path, slot_fn_name.clone());

            let samples = samples.clone();
            let slot_fn = slot_fn.clone();
            let width = self.cache.cache().block_size();

            // let slot_state_cell =

            let msg = UpdateReqMsg::new(
                key,
                move |key| {
                    let mut buf: Vec<u8> = Vec::with_capacity(samples.len());
                    buf.extend(
                        (0..width)
                            .map(|i| slot_fn(&samples, path, i))
                            .flat_map(|val| {
                                let bytes: [u8; 4] = bytemuck::cast(val);
                                bytes
                            }),
                    );

                    Ok(buf)
                },
                || {
                    //
                    // update the slotstate arc here
                },
            );
        }

        Ok(())
    }
}
