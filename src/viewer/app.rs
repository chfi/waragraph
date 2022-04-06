use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, DescSetIx, FrameResources, GpuResources, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use winit::window::Window;

use crate::graph::Waragraph;
use crate::util::{BufFmt, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::HashMap;

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use super::{PathViewer, SlotUpdateFn};

pub struct ViewerSys {
    pub view: ViewDiscrete1D,

    pub path_viewer: PathViewer,
    pub slot_renderers: SlotRenderers,

    pub slot_renderer_cache: HashMap<String, SlotUpdateFn<u32>>,

    pub labels: LabelStorage,
    pub label_updates: sled::Subscriber,

    // buffers: BufferStorage,
    pub frame_resources: [FrameResources; 2],
    pub frame: FrameBuilder,

    pub on_resize: RhaiBatchFn2<i64, i64>,

    pub draw_labels: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    pub draw_foreground: RhaiBatchFn5<BatchBuilder, rhai::Array, i64, i64, i64>,
    pub copy_to_swapchain:
        Arc<RhaiBatchFn5<BatchBuilder, DescSetIx, rhai::Map, i64, i64>>,
}

impl ViewerSys {
    pub fn init(
        engine: &mut VkEngine,
        waragraph: &Arc<Waragraph>,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        window_resources: &mut WindowResources,
        width: u32,
        // height: u32,
    ) -> Result<Self> {
        let mut txt = LabelStorage::new(&db)?;

        let label_updates = txt.tree.watch_prefix(b"t:");

        let mut builder = FrameBuilder::from_script("paths.rhai")?;

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

        let mut slot_renderers = SlotRenderers::default();

        let graph = waragraph.clone();
        slot_renderers.register_data_source("loop_count", move |path, node| {
            let path = &graph.paths[path];
            path.get(node.into()).copied()
        });

        let graph = waragraph.clone();
        slot_renderers.register_data_source("has_node", move |path, node| {
            let path = &graph.paths[path];
            path.get(node.into()).map(|_| 1)
        });

        let mut slot_renderer_cache: HashMap<String, SlotUpdateFn<u32>> =
            HashMap::default();

        // using a Rhai function for the final step in mapping values to color indices
        let mut rhai_engine = Self::create_engine(db, buffers, &arc_module);
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
        let updater_loop_count_mean = slot_renderers
            .create_sampler_mean_with("loop_count", move |v| (&cmap)(v))
            .unwrap();

        slot_renderer_cache
            .insert("loop_count_mean".to_string(), updater_loop_count_mean);

        let cmap = color_map.clone();
        slot_renderer_cache.insert(
            "loop_count_mid".to_string(),
            slot_renderers
                .create_sampler_mid_with("loop_count", move |v| {
                    (&cmap)(v as f32)
                })
                .unwrap(),
        );

        let has_node_mid =
            slot_renderers
                .create_sampler_mid_with("has_node", |v| {
                    if v == 0 {
                        0
                    } else {
                        255
                    }
                })
                .unwrap();
        slot_renderer_cache.insert("has_node_mid".to_string(), has_node_mid);

        let slot_function = builder
            .module
            .get_var_value::<rhai::ImmutableString>("slot_function")
            .unwrap_or_else(|| "loop_count_mean".into());
        db.insert(b"slot_function", slot_function.as_bytes())?;

        //
        let view = ViewDiscrete1D::new(waragraph.total_len());

        // let slot_count = 64;
        let slot_count = 32;

        let mut path_viewer = engine.with_allocators(|ctx, res, alloc| {
            PathViewer::new(
                &db,
                ctx,
                res,
                alloc,
                width as usize,
                slot_count,
                "path_slot_",
                waragraph.paths.len(),
            )
        })?;

        path_viewer.sample(&waragraph, &view);

        for i in path_viewer.visible_indices() {
            let name = format!("path-name-{}", i);
            txt.allocate_label(&db, engine, &name)?;
        }

        path_viewer.update_labels(&waragraph, &txt)?;

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
        ]
        .into_iter()
        .for_each(|(n, g)| {
            create_gradient_buffer(engine, buffers, &db, n, g, 256)
                .expect("error creating gradient buffers");
        });

        // draw_labels
        let draw_labels = rhai::Func::<
            (BatchBuilder, i64, i64, rhai::Array),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine(db, buffers, &arc_module),
            builder.ast.clone_functions_only(),
            "draw_labels",
        );

        // main draw function
        let draw_foreground = rhai::Func::<
            (BatchBuilder, rhai::Array, i64, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine(db, buffers, &arc_module),
            builder.ast.clone_functions_only(),
            "foreground",
        );

        let copy_to_swapchain = rhai::Func::<
            (BatchBuilder, DescSetIx, rhai::Map, i64, i64),
            BatchBuilder,
        >::create_from_ast(
            Self::create_engine(db, buffers, &arc_module),
            builder.ast.clone_functions_only(),
            "copy_to_swapchain",
        );

        {
            let init = rhai::Func::<(), BatchBuilder>::create_from_ast(
                Self::create_engine(db, buffers, &arc_module),
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
                    Self::create_engine(db, buffers, &arc_module),
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

        Ok(Self {
            view,

            path_viewer,
            slot_renderers,

            slot_renderer_cache,

            labels: txt,
            label_updates,

            // buffers,
            frame_resources,
            frame: builder,

            on_resize,

            draw_labels,
            draw_foreground,
            copy_to_swapchain: Arc::new(copy_to_swapchain),
        })
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

        engine
            .with_allocators(|ctx, res, alloc| {
                res_builder.insert(
                    &mut window_resources.indices,
                    ctx,
                    res,
                    alloc,
                )?;

                self.path_viewer.resize(
                    ctx,
                    res,
                    alloc,
                    width as usize,
                    0u32,
                )?;

                Ok(())
            })
            .unwrap();

        {
            let slot_width = self.path_viewer.width;

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
        window: &Window,
        window_resources: &WindowResources,
    ) -> Result<bool> {
        let f_ix = engine.current_frame_number();

        let frame = &mut self.frame_resources[f_ix % raving::vk::FRAME_OVERLAP];

        let size = window.inner_size();

        let slot_width = self.path_viewer.width;

        let label_sets = {
            self.labels
                .label_names
                .values()
                .map(|&id| {
                    use rhai::Dynamic as Dyn;
                    let mut data = rhai::Map::default();
                    let set = self.labels.desc_set_for_id(id).unwrap().unwrap();
                    let (x, y) = self.labels.get_label_pos_id(id).unwrap();
                    data.insert("x".into(), Dyn::from_int(x as i64));
                    data.insert("y".into(), Dyn::from_int(y as i64));
                    data.insert("desc_set".into(), Dyn::from(set));
                    Dyn::from_map(data)
                })
                .collect::<Vec<_>>()
        };

        let mut desc_sets = Vec::new();
        desc_sets.extend(self.path_viewer.slots.iter().map(|slot| {
            let slot_set_ix = slot.desc_set();
            let mut map = rhai::Map::default();
            map.insert("slot".into(), rhai::Dynamic::from(slot_set_ix));
            rhai::Dynamic::from_map(map)
            // desc_sets.push(rhai::Dynamic::from_map(map));
        }));

        let batch_builder = BatchBuilder::default();
        let fg_batch = (&self.draw_foreground)(
            batch_builder,
            desc_sets.clone(),
            slot_width as i64,
            size.width as i64,
            size.height as i64,
        )
        .unwrap();
        let fg_batch_fn = fg_batch.build();

        let batch_builder = BatchBuilder::default();
        let labels_batch = (&self.draw_labels)(
            batch_builder,
            size.width as i64,
            size.height as i64,
            label_sets,
        )
        .unwrap();
        let labels_batch_fn = labels_batch.build();

        let extent = vk::Extent2D {
            width: size.width,
            height: size.height,
        };

        let fg_batch = Box::new(
            move |dev: &Device,
                  res: &GpuResources,
                  _input: &BatchInput,
                  cmd: vk::CommandBuffer| {
                fg_batch_fn(dev, res, cmd);
                labels_batch_fn(dev, res, cmd);
            },
        ) as Box<_>;

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
            Some(vec![(0, vk::PipelineStageFlags::COMPUTE_SHADER)]),
            // Some(vec![(1, vk::PipelineStageFlags::COMPUTE_SHADER)]),
        ];

        let result =
            engine.draw_from_batches(frame, &batches, deps.as_slice(), 1)?;

        Ok(result)
    }

    fn create_engine(
        db: &sled::Db,
        buffers: &BufferStorage,
        module: &Arc<rhai::Module>,
    ) -> rhai::Engine {
        let mut rhai_engine = crate::console::create_engine(db, buffers);
        rhai_engine.register_static_module("viewer", module.clone());
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

pub type RhaiBatchFn1<A> = Box<
    dyn Fn(A) -> Result<BatchBuilder, Box<rhai::EvalAltResult>> + Send + Sync,
>;
pub type RhaiBatchFn2<A, B> = Box<
    dyn Fn(A, B) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn3<A, B, C> = Box<
    dyn Fn(A, B, C) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn4<A, B, C, D> = Box<
    dyn Fn(A, B, C, D) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn5<A, B, C, D, E> = Box<
    dyn Fn(A, B, C, D, E) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
