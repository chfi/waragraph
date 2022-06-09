use bimap::BiHashMap;
use crossbeam::atomic::AtomicCell;
use euclid::{point2, size2, Length, Point2D, SideOffsets2D};
use parking_lot::RwLock;
use raving::compositor::label_space::LabelSpace;
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
use crate::console::data::AnnotationSet;
use crate::console::{Console, RhaiBatchFn2, RhaiBatchFn5};
use crate::geometry::view::PangenomeView;
use crate::geometry::{LayoutElement, ListLayout, ScreenSpace};
use crate::graph::{Node, Path, Waragraph};
use crate::util::{BufFmt, BufferStorage};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, Result};

use zerocopy::AsBytes;

use super::cache::{GpuBufferCache, UpdateReqMsg};
use super::gui::layer::{label_at, path_slot};
use super::SlotFnCache;
use raving::compositor::{Compositor, Layer, SublayerAllocMsg};

pub struct ViewerSys {
    pub config: ConfigMap,
    pub props: ConfigMap,

    pub view: Arc<AtomicCell<PangenomeView>>,

    // pub path_viewer: PathViewer,
    pub slot_functions: Arc<RwLock<SlotFnCache>>,

    pub label_space: LabelSpace,

    // buffers: BufferStorage,
    pub frame_resources: [FrameResources; 2],
    pub frame: FrameBuilder,
    pub rhai_module: Arc<rhai::Module>,

    pub on_resize: RhaiBatchFn2<i64, i64>,

    pub copy_to_swapchain:
        Arc<RhaiBatchFn5<BatchBuilder, DescSetIx, rhai::Map, i64, i64>>,

    key_binds: Arc<RwLock<FxHashMap<VirtualKeyCode, rhai::FnPtr>>>,
    pub engine: rhai::Engine,

    pub slot_rhai_module: Arc<rhai::Module>,

    pub annotations:
        Arc<RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>>,

    pub path_viewer: PathViewer,
}

impl ViewerSys {
    pub fn init(
        engine: &mut VkEngine,
        compositor: &Compositor,
        waragraph: &Arc<Waragraph>,
        graph_module: &Arc<rhai::Module>,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        window_resources: &mut WindowResources,
        width: u32,
        // height: u32,
    ) -> Result<Self> {
        let mut label_space =
            LabelSpace::new(engine, "viewer-label-space", 1024 * 1024)?;

        {
            compositor.new_layer("path-slots", 1, true);
            let text_set = label_space.text_set;
            let msg = SublayerAllocMsg::new(
                "path-slots",
                "slot-labels",
                "text",
                &[text_set],
            );
            compositor.sublayer_alloc_tx.send(msg)?;

            let msg =
                SublayerAllocMsg::new("path-slots", "slots", "path-slot", &[]);
            compositor.sublayer_alloc_tx.send(msg)?;
        }

        let view = Arc::new(AtomicCell::new(PangenomeView::new(
            waragraph.total_len(),
        )));

        let mut path_viewer =
            PathViewer::new(engine, waragraph, view.load(), width as usize)?;

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

            let view_scale = path_viewer.view_scale.clone();
            let need_refresh = path_viewer.need_refresh.clone();
            module.set_native_fn("set_scale_factor", move |scale: i64| {
                let old_scale = view_scale.load();

                let new_scale = match scale {
                    1 => Ok(ScaleFactor::X1),
                    2 => Ok(ScaleFactor::X2),
                    4 => Ok(ScaleFactor::X4),
                    _ => {
                        Err("Scale factor must be one of {1, 2, 4}".to_string())
                    }
                }?;

                if old_scale != new_scale {
                    view_scale.store(new_scale);
                    need_refresh.store(true);
                }

                Ok(())
            });

            let view_scale = path_viewer.view_scale.clone();
            module.set_native_fn("get_scale_factor", move || {
                let factor = match view_scale.load() {
                    ScaleFactor::X1 => 1i64,
                    ScaleFactor::X2 => 2i64,
                    ScaleFactor::X4 => 4i64,
                };

                Ok(factor)
            });

            let need_refresh = path_viewer.need_refresh.clone();
            module.set_native_fn("force_update", move || {
                need_refresh.store(true);
                Ok(())
            });

            let row_view = path_viewer.row_view_latch.clone();
            module.set_native_fn("list_range", move || {
                let (offset, len) = row_view.load();
                let o = offset as i64;
                let l = len as i64;
                Ok(o..l)
            });

            Arc::new(module)
        };

        let mut builder =
            FrameBuilder::from_script_with("paths.rhai", |engine| {
                crate::console::register_buffer_storage(db, buffers, engine);
                crate::console::append_to_engine(db, engine);

                engine.register_static_module("slot", slot_module.clone());
                engine.register_static_module("graph", graph_module.clone());

                engine.register_fn("bind_key", bind_key_closure.clone());
            })?;

        let config = builder.module.get_var_value::<ConfigMap>("cfg").unwrap();
        let props = builder.module.get_var_value::<ConfigMap>("props").unwrap();

        {
            let view_ = view.clone();
            builder
                .module
                .set_native_fn("get_view", move || Ok(view_.load()));

            let view_ = view.clone();
            let need_refresh = path_viewer.need_refresh.clone();
            builder.module.set_native_fn(
                "set_view",
                move |new: PangenomeView| {
                    if new != view_.load() {
                        need_refresh.store(true);
                    }
                    view_.store(new);
                    Ok(())
                },
            );

            let need_refresh = path_viewer.need_refresh.clone();
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

                    if let Some(view) = view.try_cast::<PangenomeView>() {
                        if old_view != view {
                            need_refresh.store(true);
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

        log::debug!("Config: {:?}", config);

        let arc_module = Arc::new(builder.module.clone());

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
                |_path, v| {
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

        ////

        path_viewer.sample(
            waragraph,
            path_viewer.view_scale.load(),
            &view.load(),
        );

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

        let engine =
            Self::create_engine_impl(db, buffers, &arc_module, &slot_module);

        Ok(Self {
            config,
            props,

            view,

            // path_viewer,
            slot_functions: slot_fns,
            annotations,

            label_space,

            // buffers,
            frame_resources,
            frame: builder,
            rhai_module: arc_module,

            on_resize,

            copy_to_swapchain: Arc::new(copy_to_swapchain),

            key_binds,
            engine,

            slot_rhai_module: slot_module,

            path_viewer,
        })
    }

    pub fn handle_input(
        &mut self,
        console: &Console,
        event: &winit::event::WindowEvent<'_>,
    ) {
        match event {
            winit::event::WindowEvent::KeyboardInput { input, .. } => {
                if let Some(kc) = input.virtual_keycode {
                    use VirtualKeyCode as VK;

                    let mut view = self.view.load();

                    let pre_len = view.len();
                    let len = view.len().0;
                    let view_mid = view.offset() + view.len() / 2;

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
                            view = view.shift_left(len / 10);
                            update = true;
                        } else if matches!(kc, VK::Right) {
                            view = view.shift_right(len / 10);
                            update = true;
                        } else if matches!(kc, VK::Up) {
                            if view.len().0 > self.path_viewer.current_width {
                                view =
                                    view.resize_mid((len - len / 9) as usize);
                            }

                            if view.len().0 <= self.path_viewer.current_width {
                                view.set(
                                    view.offset().0,
                                    self.path_viewer.current_width,
                                );
                            }

                            // view.len =
                            //     view.len.max(self.path_viewer.current_width);

                            update = true;
                        } else if matches!(kc, VK::Down) {
                            view = view.resize_mid((len + len / 10) as usize);

                            // view = view.resize_around(
                            //     view_mid.0,
                            //     (len + len / 10) as usize,
                            // );
                            update = true;
                        } else if matches!(kc, VK::Escape) {
                            view.reset();
                            update = true;
                        } else if matches!(kc, VK::PageUp) {
                            self.path_viewer.slot_list.scroll(-1);
                            update = true;
                        } else if matches!(kc, VK::PageDown) {
                            self.path_viewer.slot_list.scroll(1);
                            update = true;
                        }
                    }

                    self.view.store(view);

                    self.path_viewer.need_refresh.fetch_or(update);
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

        let slot_h = (get_cast(&slot, "h") + padding * 2) as usize;

        let bottom_pad = get_cast(&map, "layout.list_bottom_pad") as usize;

        let count = (win_h - y - bottom_pad) / slot_h;

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

                Ok(())
            })
            .unwrap();

        {
            let slot_width = self.path_viewer.current_width;
            let mut view = self.view.load();

            if view.len().0 < slot_width {
                view.len().0 = slot_width;
                self.view.store(view);
            }

            self.path_viewer.sample(
                waragraph,
                self.path_viewer.view_scale.load(),
                &view,
            );
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

        let extent = vk::Extent2D { width, height };

        let out_framebuffer =
            *window_resources.indices.framebuffers.get("out").unwrap();

        let comp_batch_fn =
            compositor.draw(Some([0.9, 0.9, 0.9]), out_framebuffer, extent);

        let fg_batch = Box::new(
            move |dev: &Device,
                  res: &GpuResources,
                  _input: &BatchInput,
                  cmd: vk::CommandBuffer| {
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
        // let len = 1.min(max);
        let len = 16.min(max);
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

pub type SlotFnVar = usize;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScaleFactor {
    X1,
    X2,
    X4,
}

impl ScaleFactor {
    pub fn scale(&self, v: usize) -> usize {
        match self {
            ScaleFactor::X1 => v,
            ScaleFactor::X2 => v / 2,
            ScaleFactor::X4 => v / 4,
        }
    }
}

impl std::default::Default for ScaleFactor {
    fn default() -> Self {
        Self::X1
    }
}

pub struct PathViewer {
    pub list_layout: Arc<AtomicCell<ListLayout>>,

    pub cache: GpuBufferCache<(Path, SlotFnName)>,

    slot_fn_vars: BiHashMap<SlotFnVar, SlotFnName>,

    pub slot_list: ListView<(Path, SlotFnVar)>,

    slot_states: HashMap<(Path, SlotFnName), Arc<AtomicCell<SlotState>>>,

    current_view: PangenomeView,
    pub current_width: usize,

    pub need_refresh: Arc<AtomicCell<bool>>,

    row_view_latch: Arc<AtomicCell<(usize, usize)>>,
    // row_offset: Arc<AtomicCell<usize>>,
    pub view_scale: Arc<AtomicCell<ScaleFactor>>,

    pub sample_buf: Arc<Vec<(Node, usize)>>,
    new_samples: AtomicCell<bool>,
}

impl PathViewer {
    pub fn new(
        engine: &mut VkEngine,
        graph: &Arc<Waragraph>,
        current_view: PangenomeView,
        current_width: usize,
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

        let mut slot_fn_vars: BiHashMap<SlotFnVar, SlotFnName> =
            BiHashMap::new();

        slot_fn_vars.insert(0, "depth_mean".into());
        // slot_fn_vars.insert(1, "path_primary".into());

        let slot_list = ListView::new(
            graph.path_names.left_values().map(|&path| (path, 0)),
        );

        let row_view = (slot_list.offset, slot_list.len);

        let list_layout = ListLayout {
            origin: point2(0.0, 0.0),
            size: size2(256.0, 256.0),
            side_offsets: Some(SideOffsets2D::new(36.0, 10.0, 100.0, 14.0)),
            slot_height: Length::new(18.0),
        };

        Ok(Self {
            list_layout: Arc::new(list_layout.into()),

            cache,

            slot_fn_vars,

            slot_list,
            slot_states: HashMap::default(),

            current_view,
            current_width,

            need_refresh: Arc::new(false.into()),

            row_view_latch: Arc::new(row_view.into()),

            view_scale: Arc::new(ScaleFactor::X1.into()),

            sample_buf: Arc::new(Vec::new()),

            new_samples: false.into(),
        })
    }

    pub fn sample(
        &mut self,
        graph: &Waragraph,
        scale_factor: ScaleFactor,
        view: &PangenomeView,
    ) {
        let nsamples = scale_factor.scale(self.current_width);

        let sample_buf = Arc::make_mut(&mut self.sample_buf);

        if nsamples > 0 {
            graph.sample_node_lengths(nsamples, view, sample_buf);
            self.new_samples.store(true);
        }
    }

    pub fn has_new_samples(&self) -> bool {
        self.new_samples.load()
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
        win_dims: [u32; 2],
        graph: &Arc<Waragraph>,
        label_space: &LabelSpace,
        slot_fns: &SlotFnCache,
        config: &ConfigMap,
        buffer_width: usize,
        view: PangenomeView,
        row_count: usize,
    ) -> Result<()> {
        let [win_width, win_height] = win_dims;

        let mut layout = self.list_layout.load();
        self.list_layout.store(layout);

        // get the active slot functions from the rhai config object
        {
            let map = config.map.read();
            let primary = map
                .get("viz.slot_function")
                .unwrap()
                .clone_cast::<rhai::ImmutableString>();
            // let secondary = map
            //     .get("viz.secondary")
            //     .unwrap()
            //     .clone_cast::<rhai::ImmutableString>();

            if Some(&primary) != self.slot_fn_vars.get_by_left(&0) {
                self.slot_fn_vars.insert(0, primary);
                self.slot_states.clear();
            }

            let size = size2(win_width as f32, win_height as f32);
            let layout = ListLayout::from_config_map(config, size).unwrap();
            self.list_layout.store(layout);
        };

        let _ = label_space.write_buffer(&mut engine.resources);

        let buffer_width = match self.view_scale.load() {
            ScaleFactor::X1 => buffer_width,
            ScaleFactor::X2 => buffer_width / 2,
            ScaleFactor::X4 => buffer_width / 4,
        };

        if self.current_width != buffer_width || self.current_view != view {
            self.need_refresh.store(true);
        }

        self.current_width = buffer_width;
        self.current_view = view;
        self.slot_list.resize(row_count);

        // reallocate and invalidate cache if cache block size doesn't
        // match the width, or if the current slot list view size is
        // greater than the cache block capacity
        let slot_count = self.slot_list.len();

        self.row_view_latch
            .store((self.slot_list.offset, self.slot_list.len));

        if slot_count > self.cache.cache().block_capacity()
            || self.current_width != self.cache.cache().block_size()
        {
            let block_cap = self.cache.cache().block_capacity();
            let new_slot_count = if slot_count > block_cap {
                slot_count * 2
            } else {
                block_cap
            };

            self.cache.reallocate(
                engine,
                new_slot_count,
                self.current_width,
            )?;
            self.slot_states.clear();
        }

        // make sure all entries in the slot list are bound in the cache
        let result = self.cache.bind_blocks(self.slot_list.visible_rows().map(
            |(path, var)| {
                let slot_fn = self.slot_fn_vars.get_by_left(var).unwrap();
                (*path, slot_fn.clone())
            },
        ));

        if let Err(e) = result {
            log::error!("Cache error: {:#?}", e);
            Err(e)?;
        }

        // for each visible (Path, SlotFnName) in the slot list
        let need_refresh = self.need_refresh();

        if need_refresh {
            self.sample(graph, self.view_scale.load(), &view);
        }

        for (path, var) in self.slot_list.visible_rows() {
            let slot_fn = self.slot_fn_vars.get_by_left(var).unwrap();
            let key = (*path, slot_fn.clone());

            // get the state cell for the slot, creating and inserting
            // a new cell with the Unknown state if the current key
            // doesn't have a state
            let cell = self.slot_states.entry(key.clone()).or_default().clone();

            let need_update = match cell.load() {
                // let need_update = match cell.load() {
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
                    // need_refresh
                    // || buffer_width != self.current_width
                    buffer_width != self.current_width
                        || view_offset != self.current_view.offset().0
                        || view_len != self.current_view.len().0
                }
            };

            // if there is no entry, it is unknown, or the view or width
            // contained do not match,

            if need_update {
                let (path, slot_fn_name) = key;

                let slot_fn = slot_fns.slot_fn_u32.get(&slot_fn_name).ok_or(
                    anyhow!("slot renderer `{}` not found", slot_fn_name),
                )?;

                let key = (path, slot_fn_name.clone());

                let samples = self.sample_buf.clone();
                let slot_fn = slot_fn.clone();
                let width = self.cache.cache().block_size();

                let current_width = self.current_width;
                let current_view = self.current_view;

                cell.store(SlotState::Updating);

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
                        let cur = cell.load();

                        if cur == SlotState::Unknown
                            || cur == SlotState::Updating
                        {
                            cell.store(SlotState::Contains {
                                buffer_width: current_width,
                                view_offset: current_view.offset().0,
                                view_len: current_view.len().0,
                            });
                        }

                        // cell.store(SlotState::Contains {
                        //     buffer_width: current_width,
                        //     view_offset: current_view.offset().0,
                        //     view_len: current_view.len().0,
                        // });
                    },
                );

                self.cache.update_request_tx.send(msg)?;
            }
        }

        self.need_refresh.store(false);
        self.new_samples.store(false);

        // apply the GPU cache data messages, which also updates the
        // slot_state entries, concurrently
        self.cache.apply_data_updates(&mut engine.resources)?;

        Ok(())
    }

    // update the sublayer's vertex data (must be a slot sublayer)
    // with the currently visible slots
    // slots that are in the process of being updated are skipped
    //
    pub fn update_slot_sublayer(
        &self,
        graph: &Arc<Waragraph>,
        label_space: &mut LabelSpace,
        layer: &mut Layer,
        config: &ConfigMap,
        slot_fns: &SlotFnCache,
        buffer_storage: &BufferStorage,
    ) -> Result<()> {
        let (slot_partition_x, name_len) = {
            let map = config.map.read();
            let padding =
                map.get("layout.padding").unwrap().clone_cast::<i64>();

            let get_cast =
                |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();

            let name_len = get_cast(&map, "layout.max_path_name_len");

            let slot_x = padding + name_len * 8;

            (slot_x as f32, name_len as usize)
        };

        let slot_ver_offsets =
            SideOffsets2D::<f32, ScreenSpace>::new(1.0f32, 0.0, 1.0, 0.0);
        let label_offsets = SideOffsets2D::new(2.0, 0.0, 0.0, 0.0);

        let mut vertices: Vec<[u8; 24]> = Vec::new();

        let layout = self.list_layout.load();

        for (_ix, rect, (path, var)) in
            layout.apply_to_rows(self.slot_list.visible_rows())
        {
            let slot_fn = self.slot_fn_vars.get_by_left(var).unwrap();
            let key = (*path, slot_fn.clone());

            // if the state cell somehow doesn't exist, or shows that
            // there's probably only garbage data there, simply skip
            // this row (it'll get drawn when the data's ready)
            //
            // TODO: it still renders garbage when resizing
            if let Some(state) = self.slot_states.get(&key) {
                let state = state.load();
                match state {
                    SlotState::Unknown => {
                        // log::warn!("Unknown, skipping slot {}", ix);
                        continue;
                    }
                    _ => (),
                }
            } else {
                continue;
            }

            // otherwise prepare the vertex data
            let range = if let Some(range) = self.cache.cache().get_range(&key)
            {
                range
            } else {
                continue;
            };

            let [_left, right] = rect.split_hor(slot_partition_x);

            let right = right.inner_rect(slot_ver_offsets);

            vertices.push(path_slot(
                right,
                range.start,
                range.end - range.start,
            ));
        }

        let data_set = self.cache.desc_set();

        let color_buffer_set = {
            let slot_fn = self.slot_fn_vars.get_by_left(&0).unwrap();
            let name = slot_fns.slot_color.get(slot_fn).unwrap();
            let buf_id = buffer_storage.get_id(name.as_str()).unwrap();
            let set = buffer_storage.get_desc_set_ix(buf_id).unwrap();
            set
        };

        if let Some(sublayer) = layer.get_sublayer_mut("slots") {
            sublayer.draw_data_mut().try_for_each(|data| {
                data.update_sets([data_set, color_buffer_set]);
                data.update_vertices_array(vertices.iter().copied())
            })?;
        }

        // TODO handle path name labels
        if let Some(sublayer) = layer.get_sublayer_mut("slot-labels") {
            let mut vertices: Vec<[u8; 32]> = Vec::new();

            // insert path names into the label space
            for (_ix, rect, (path, _var)) in
                layout.apply_to_rows(self.slot_list.visible_rows())
            {
                // for (ix, (path, _)) in self.slot_list.visible_rows().enumerate() {
                let path_name = graph.path_name(*path).unwrap();

                let text = if path_name.len() < name_len {
                    format!("{}", path_name)
                } else {
                    let prefix = &path_name[..name_len - 3];
                    format!("{}...", prefix)
                };

                let bounds = label_space.bounds_for_insert(&text)?;

                let [left, _right] = rect.split_hor(slot_partition_x);
                let left = left.inner_rect(label_offsets);

                vertices.push(label_at(
                    left.origin,
                    bounds,
                    rgb::RGBA::new(0.0, 0.0, 0.0, 1.0),
                ));
            }

            // upload vertices
            sublayer.draw_data_mut().try_for_each(|data| {
                data.update_vertices_array(vertices.iter().copied())
            })?;
        }

        Ok(())
    }
}
