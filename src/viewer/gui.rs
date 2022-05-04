use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, BufferIx, DescSetIx, FrameResources, FramebufferIx,
    GpuResources, PipelineIx, RenderPassIx, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::{FxHashMap, FxHashSet};
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};
use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufId, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use rhai::plugin::*;

pub mod tree_list;

type LabelId = u64;

#[derive(Clone)]
pub enum RectVertices {
    // RGBA {
    //     vertices: Vec<[f32; 6]>,
    // },
    Palette {
        buffer_set: DescSetIx,
        rects: Vec<([f32; 4], u32)>,
    },
    Text {
        buffer_set: DescSetIx,
        labels: Vec<([f32; 2], [u32; 2], [f32; 4])>,
    },
}

#[derive(Clone)]
pub struct GuiLayer {
    // rects: Arc<RwLock<RectVertices>>,
    name: rhai::ImmutableString,
    pub rects: RectVertices,

    // labels: FxHashMap<u64, Arc<AtomicCell<bool>>>,
    // labels: FxHashMap<rhai::ImmutableString, Arc<(u64, AtomicCell<bool>)>>,
    pub labels: FxHashMap<rhai::ImmutableString, GuiLabel>,

    vertex_buf_id: BufId,
    pub vertex_buf_ix: BufferIx,
}

#[derive(Clone)]
pub struct GuiLabel {
    pub layer: rhai::ImmutableString,
    pub label_id: u64,
    visible: Arc<AtomicCell<bool>>,
}

impl GuiLabel {
    pub fn is_visible(&self) -> bool {
        self.visible.load()
    }

    pub fn set_visibility(&self, vis: bool) {
        self.visible.store(vis);
    }
}

#[derive(Default, Clone)]
pub struct LabelMsg {
    pub layer_name: rhai::ImmutableString,
    pub label_name: rhai::ImmutableString,

    pub set_visibility: Option<bool>,
    pub set_position: Option<[u32; 2]>,
    pub set_contents: Option<rhai::ImmutableString>,
}

impl LabelMsg {
    pub fn new(layer_name: &str, label_name: &str) -> Self {
        Self {
            layer_name: layer_name.into(),
            label_name: label_name.into(),
            ..Self::default()
        }
    }

    pub fn set_visibility(&mut self, vis: bool) {
        self.set_visibility = Some(vis);
    }

    pub fn set_position(&mut self, x: u32, y: u32) {
        self.set_position = Some([x, y]);
    }

    pub fn set_contents(&mut self, contents: &str) {
        self.set_contents = Some(contents.into());
    }
}

impl GuiLayer {
    pub fn new(
        engine: &mut VkEngine,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        name: &str,
        size: usize,
        color_buf_set: DescSetIx,
    ) -> Result<Self> {
        // let name = name.into();
        let rects = RectVertices::Palette {
            buffer_set: color_buf_set,
            rects: Vec::new(),
        };

        let vertex_buf_id = buffers.allocate_buffer_with_usage(
            engine,
            &db,
            name,
            BufFmt::FVec3,
            size,
            vk::BufferUsageFlags::VERTEX_BUFFER
                | vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST,
        )?;
        let vertex_buf_ix = buffers.get_buffer_ix(vertex_buf_id).unwrap();

        Ok(Self {
            name: name.into(),

            rects,
            labels: FxHashMap::default(),

            vertex_buf_id,
            vertex_buf_ix,
        })
    }

    pub fn apply_label_msg(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        labels: &mut LabelStorage,
        msg: LabelMsg,
    ) -> Result<()> {
        if self.name != msg.layer_name {
            log::error!(
                "LabelMsg for layer `{}` being applied to layer `{}`",
                msg.layer_name,
                self.name
            );
        }

        if !self.labels.contains_key(&msg.label_name) {
            let key = self.label_name_sled_key(&msg.label_name);
            let id = labels.allocate_label(&db, engine, &key)?;
            let label = GuiLabel {
                layer: self.name.clone(),
                label_id: id,
                visible: Arc::new(true.into()),
            };
            self.labels.insert(msg.label_name.clone(), label.clone());
        }

        let label = self.labels.get(&msg.label_name).unwrap();

        if let Some(vis) = msg.set_visibility {
            label.visible.store(vis);
        }

        if let Some([x, y]) = msg.set_position {
            labels.set_pos_for_id(label.label_id, x, y)?;
        }

        if let Some(contents) = msg.set_contents {
            labels.set_text_for_id(label.label_id, contents.as_str())?;
        }

        Ok(())
    }

    pub fn get_label<'a>(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        labels: &mut LabelStorage,
        name: &str,
    ) -> Result<GuiLabel> {
        {
            if let Some(label) = self.labels.get(name) {
                return Ok(label.clone());
            }
        }

        let key = self.label_name_sled_key(name);
        let id = labels.allocate_label(&db, engine, &key)?;
        let label = GuiLabel {
            layer: self.name.clone(),
            label_id: id,
            visible: Arc::new(true.into()),
        };
        self.labels.insert(name.into(), label.clone());
        Ok(label)
    }

    pub fn set_label_pos(
        &self,
        labels: &LabelStorage,
        label: &GuiLabel,
        pos: [u32; 2],
    ) -> Result<()> {
        let [x, y] = pos;
        labels.set_pos_for_id(label.label_id, x, y)?;
        Ok(())
    }

    pub fn set_label_contents(
        &self,
        labels: &LabelStorage,
        label: &GuiLabel,
        contents: &str,
    ) -> Result<()> {
        labels.set_text_for_id(label.label_id, contents)?;
        Ok(())
    }

    fn label_name_sled_key(&self, name: &str) -> String {
        format!("{}:label:{}", self.name, name)
    }

    /*

    pub fn get_label(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        labels: &mut LabelStorage,
        name: &str,
    ) -> Result<GuiLabel> {
        if let Some(_visible) = self.labels.get(name) {
            //
        } else {
            let name = format!("{}:label:{}", self.name, name);
            let id = labels.allocate_label(&db, engine, &name)?;
            todo!();
        }
    }
    */

    // pub fn get_label(&mut self, name: &str) -> GuiLabel {
    // if let Some(_vis) = self.labels.get(
    // }
    // fn label_key

    // pub fn label_id(&self, name: &str) -> u64 {
    // }
}

// pub struct GuiLayer {
//     labels: FxHashSet<LabelId>,
// }

// pub struct GuiLayer {
// }

#[derive(Clone)]
pub enum RectColor {
    // RGBA { r: f32, g: f32, b: f32, a: f32 },
    PaletteName {
        buffer_name: rhai::ImmutableString,
        ix: u32,
    },
    PaletteId {
        buffer_id: BufId,
        ix: u32,
    },
}

#[derive(Clone)]
pub enum GuiMsg {
    CreateLayer {
        name: rhai::ImmutableString,
    },
    ModifyLayer {
        name: rhai::ImmutableString,
        fn_ptr: rhai::FnPtr,
    },
}

pub struct GuiSys {
    pub config: ConfigMap,

    pub layers: Arc<RwLock<FxHashMap<rhai::ImmutableString, GuiLayer>>>,
    pub layer_order: Arc<RwLock<Vec<rhai::ImmutableString>>>,

    pub labels: LabelStorage,
    pub label_updates: sled::Subscriber,

    // pub rects: Vec<Arc<RwLock<GuiLayer>>>,
    // pub rects: Arc<RwLock<Vec<([f32; 4], RectColor)>>>,
    // pub rhai_module: Arc<rhai::Module>,

    // pub on_resize: RhaiBatchFn2<i64, i64>,

    // pub draw_labels: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    // pub draw_shapes: RhaiBatchFn4<BatchBuilder, i64, i64, rhai::Array>,
    pub pass: RenderPassIx,
    pub pipeline: PipelineIx,
    pub text_pipeline: PipelineIx,

    pub rhai_module: Arc<rhai::Module>,

    pub label_msg_tx: crossbeam::channel::Sender<LabelMsg>,
    pub label_msg_rx: crossbeam::channel::Receiver<LabelMsg>,

    msg_tx: crossbeam::channel::Sender<GuiMsg>,
    msg_rx: crossbeam::channel::Receiver<GuiMsg>,

    window_dims: Arc<AtomicCell<[u32; 2]>>,
}

impl GuiSys {
    const VX_BUF_NAME: &'static str = "waragraph:gui:vertices";

    pub fn update_layer_buffers(&self, buffers: &BufferStorage) -> Result<()> {
        let mut vertices: Vec<[f32; 3]> = Vec::new();

        for (name, layer) in self.layers.read().iter() {
            match &layer.rects {
                RectVertices::Palette { buffer_set, rects } => {
                    vertices.clear();

                    for (rect, color) in rects.iter() {
                        let &[x, y, w, h] = rect;

                        let color = *color as f32;

                        vertices.push([x, y, color]);
                        vertices.push([x, y + h, color]);
                        vertices.push([x + w, y, color]);

                        vertices.push([x, y + h, color]);
                        vertices.push([x + w, y + h, color]);
                        vertices.push([x + w, y, color]);
                    }

                    buffers.insert_data(layer.vertex_buf_id, &vertices)?;
                }

                RectVertices::Text { buffer_set, labels } => {
                    vertices.clear();
                }
            }

            //
        }

        Ok(())
    }

    // pub fn add_label(&mut self, name: &str)

    pub fn init(
        engine: &mut VkEngine,
        db: &sled::Db,
        window_dims: &Arc<AtomicCell<[u32; 2]>>,
    ) -> Result<Self> {
        let window_dims = window_dims.clone();

        let mut config = ConfigMap::default();

        let labels = LabelStorage::new(&db)?;
        let label_updates = labels.tree.watch_prefix(b"t:");

        let (pass_ix, pipeline_ix, text_pipeline_ix) = {
            // let format = engine.swapchain_props.format.format;
            let format = vk::Format::R8G8B8A8_UNORM;

            engine.with_allocators(|ctx, res, _| {
                let pass = res.create_line_render_pass(
                    ctx,
                    format,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::GENERAL,
                )?;

                let vert = res.load_shader(
                    "shaders/rect.vert.spv",
                    vk::ShaderStageFlags::VERTEX,
                )?;
                let frag = res.load_shader(
                    "shaders/rect_flat_color.frag.spv",
                    vk::ShaderStageFlags::FRAGMENT,
                )?;

                let pass_ix = res.insert_render_pass(pass);
                let vx = res.insert_shader(vert);
                let fx = res.insert_shader(frag);

                let pass = res[pass_ix];

                let pipeline_ix =
                    res.create_graphics_pipeline_tmp(ctx, vx, fx, pass)?;

                let vert = res.load_shader(
                    "shaders/text.vert.spv",
                    vk::ShaderStageFlags::VERTEX,
                )?;
                let frag = res.load_shader(
                    "shaders/text.frag.spv",
                    vk::ShaderStageFlags::FRAGMENT,
                )?;

                let vx = res.insert_shader(vert);
                let fx = res.insert_shader(frag);

                let vert_binding_desc =
                    vk::VertexInputBindingDescription::builder()
                        .binding(0)
                        .stride(std::mem::size_of::<Self>() as u32)
                        .input_rate(vk::VertexInputRate::INSTANCE)
                        .build();

                let pos_desc = vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(0)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(0)
                    .build();

                let ix_desc = vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(1)
                    .format(vk::Format::R32G32_UINT)
                    .offset(8)
                    .build();

                let color_desc = vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(2)
                    .format(vk::Format::R32G32_UINT)
                    .offset(16)
                    .build();

                let vert_binding_descs = [vert_binding_desc];
                let vert_attr_descs = [pos_desc, ix_desc, color_desc];

                let vert_input_info =
                    vk::PipelineVertexInputStateCreateInfo::builder()
                        .vertex_binding_descriptions(&vert_binding_descs)
                        .vertex_attribute_descriptions(&vert_attr_descs);

                let text_pipeline_ix = res.create_graphics_pipeline(
                    ctx,
                    vx,
                    fx,
                    pass,
                    vert_input_info,
                )?;

                Ok((pass_ix, pipeline_ix, text_pipeline_ix))
            })?
        };

        let (label_msg_tx, label_msg_rx) = crossbeam::channel::unbounded();
        let (msg_tx, msg_rx) = crossbeam::channel::unbounded();

        let layer_order = Arc::new(RwLock::new(Vec::new()));

        let mut module: rhai::Module = rhai::exported_module!(script);

        // TODO: this implementation might lead to issues if the
        // swapchain is resized before/while the script is running,
        // if/when scripts are being run asynchronously
        let dims = window_dims.clone();
        module.set_native_fn("get_window_size", move || {
            let [width, height] = dims.load();
            let mut map = rhai::Map::default();
            map.insert("width".into(), (width as i64).into());
            map.insert("height".into(), (height as i64).into());
            Ok(map)
        });

        let layers = Arc::new(RwLock::new(FxHashMap::default()));

        let layers_ = layers.clone();

        module.set_raw_fn(
            "with_layer",
            rhai::FnNamespace::Global,
            rhai::FnAccess::Public,
            [
                TypeId::of::<rhai::ImmutableString>(),
                TypeId::of::<rhai::FnPtr>(),
            ],
            move |ctx, args| {
                let layer_name: rhai::ImmutableString =
                    args.get(0).unwrap().clone_cast();
                let fn_ptr: rhai::FnPtr = std::mem::take(args[1]).cast();

                {
                    let mut layers = layers_.write();

                    if let Some((layer_name, layer)) =
                        layers.remove_entry(&layer_name)
                    {
                        let mut layer = rhai::Dynamic::from(layer);

                        if let Err(e) =
                            fn_ptr.call_raw(&ctx, Some(&mut layer), [])
                        {
                            log::error!("GUI with_layer error: {:?}", e);
                        }

                        let layer = layer.cast::<GuiLayer>();

                        layers.insert(layer_name, layer);

                        Ok(rhai::Dynamic::TRUE)
                    } else {
                        Ok(rhai::Dynamic::FALSE)
                    }
                }
            },
        );

        let order = layer_order.clone();
        module.set_native_fn("get_layer_order", move || {
            let order = order.read();
            let result: rhai::Array = order
                .iter()
                .map(|name: &rhai::ImmutableString| name.into())
                .collect();
            Ok(result)
        });

        let order = layer_order.clone();
        module.set_native_fn(
            "set_layer_order",
            move |new_order: rhai::Array| {
                let mut order = order.write();
                order.clear();
                order.extend(new_order.into_iter().filter_map(
                    |name: rhai::Dynamic| name.into_immutable_string().ok(),
                ));
                Ok(())
            },
        );

        let order = layer_order.clone();
        module.set_native_fn("get_top_layer", move || {
            if let Some(layer) = order.read().last() {
                Ok(layer.into())
            } else {
                Ok(rhai::Dynamic::UNIT)
            }
        });

        let order = layer_order.clone();
        module.set_native_fn(
            "push_layer",
            move |layer: rhai::ImmutableString| {
                let mut layers = order.write();
                if layers.last() == Some(&layer) {
                    Ok(rhai::Dynamic::FALSE)
                } else {
                    layers.push(layer);
                    Ok(rhai::Dynamic::TRUE)
                }
            },
        );

        let order = layer_order.clone();
        module.set_native_fn("pop_layer", move || {
            let mut layers = order.write();
            match layers.pop() {
                Some(layer) => Ok(layer.into()),
                None => Ok(rhai::Dynamic::FALSE),
            }
        });

        let order = layer_order.clone();
        module.set_native_fn(
            "pop_layer",
            move |layer: rhai::ImmutableString| {
                let mut layers = order.write();
                if layers.last() == Some(&layer) {
                    layers.pop();
                    Ok(rhai::Dynamic::TRUE)
                } else {
                    Ok(rhai::Dynamic::FALSE)
                }
            },
        );

        module.set_native_fn("mk_rects", || {
            let mut rects = Vec::new();

            let int = |v: u32| rhai::Dynamic::from_int(v as i64);

            let mut push = |([x, y, w, h], c): ([u32; 4], u32)| {
                let mut map = rhai::Map::default();

                map.extend(
                    [("x", x), ("y", y), ("w", w), ("h", h), ("c", c)]
                        .into_iter()
                        .map(|(n, v)| (n.into(), int(v))),
                );
                rects.push(rhai::Dynamic::from_map(map));
            };
            push(([50, 50, 150, 150], 2));
            push(([100, 100, 100, 100], 7));
            push(([300, 300, 100, 100], 5));
            Ok(rects)
        });

        module.set_native_fn("label", |layer: &str, label: &str| {
            Ok(LabelMsg::new(layer, label))
        });

        module.set_native_fn(
            "set_visibility",
            |msg: &mut LabelMsg, vis: bool| {
                msg.set_visibility(vis);
                Ok(())
            },
        );

        module.set_native_fn(
            "set_position",
            |msg: &mut LabelMsg, x: i64, y: i64| {
                msg.set_position(x as u32, y as u32);
                Ok(())
            },
        );

        module.set_native_fn(
            "set_contents",
            |msg: &mut LabelMsg, contents: &str| {
                msg.set_contents(contents);
                Ok(())
            },
        );

        let tx = label_msg_tx.clone();
        module.set_native_fn("update_label", move |msg: LabelMsg| {
            if let Err(e) = tx.send(msg) {
                log::error!("GUI send_label_msg error: {:?}", e);
                return Ok(rhai::Dynamic::FALSE);
            }

            Ok(rhai::Dynamic::TRUE)
        });

        let rhai_module = Arc::new(module);

        Ok(Self {
            config,

            layers,
            layer_order,

            labels,
            label_updates,

            pass: pass_ix,
            pipeline: pipeline_ix,
            text_pipeline: text_pipeline_ix,

            rhai_module,

            label_msg_tx,
            label_msg_rx,

            msg_tx,
            msg_rx,

            window_dims,
        })
    }

    pub fn draw_impl(
        layers: Arc<RwLock<FxHashMap<rhai::ImmutableString, GuiLayer>>>,
        layer_names: Vec<rhai::ImmutableString>,
        pass: RenderPassIx,
        pipeline: PipelineIx,
        text_pipeline: PipelineIx,
        framebuffer: FramebufferIx,
        // vertex_count: usize,
        extent: vk::Extent2D,
        device: &Device,
        res: &GpuResources,
        cmd: vk::CommandBuffer,
    ) {
        let pass_info = vk::RenderPassBeginInfo::builder()
            .render_pass(res[pass])
            .framebuffer(res[framebuffer])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .clear_values(&[])
            .build();

        unsafe {
            device.cmd_begin_render_pass(
                cmd,
                &pass_info,
                vk::SubpassContents::INLINE,
            );

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: extent.width as f32,
                height: extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };

            let viewports = [viewport];

            device.cmd_set_viewport(cmd, 0, &viewports);

            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            };
            let scissors = [scissor];

            device.cmd_set_scissor(cmd, 0, &scissors);

            let (pipeline, layout) = res[pipeline];

            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );

            let layers = layers.read();

            layer_names
                .iter()
                .filter_map(|name| {
                    let layer = layers.get(name)?;
                    Some((name, layer))
                })
                .for_each(|(layer_name, layer)| match &layer.rects {
                    RectVertices::Text { buffer_set, labels } => {
                        if !labels.is_empty() {
                            let vx_buf_ix = layer.vertex_buf_ix;

                            let vx_buf = res[vx_buf_ix].buffer;
                            let vxs = [vx_buf];

                            device.cmd_bind_vertex_buffers(cmd, 0, &vxs, &[12]);

                            let dims =
                                [extent.width as f32, extent.height as f32];

                            let constants = bytemuck::cast_slice(&dims);

                            let stages = vk::ShaderStageFlags::VERTEX
                                | vk::ShaderStageFlags::FRAGMENT;
                            device.cmd_push_constants(
                                cmd, layout, stages, 0, constants,
                            );

                            let descriptor_sets = [res[*buffer_set]];
                            device.cmd_bind_descriptor_sets(
                                cmd,
                                vk::PipelineBindPoint::GRAPHICS,
                                layout,
                                0,
                                &descriptor_sets,
                                &[],
                            );

                            let instance_count = labels.len();
                            let vertex_count = 6;

                            device.cmd_draw(
                                cmd,
                                vertex_count as u32,
                                instance_count as u32,
                                0,
                                0,
                            );
                        }
                    }
                    RectVertices::Palette { buffer_set, rects } => {
                        if !rects.is_empty() {
                            let vx_buf_ix = layer.vertex_buf_ix;

                            let vx_buf = res[vx_buf_ix].buffer;
                            let vxs = [vx_buf];

                            let vertex_count = rects.len() * 6;

                            device.cmd_bind_vertex_buffers(cmd, 0, &vxs, &[12]);

                            let dims =
                                [extent.width as f32, extent.height as f32];

                            let constants = bytemuck::cast_slice(&dims);

                            let stages = vk::ShaderStageFlags::VERTEX
                                | vk::ShaderStageFlags::FRAGMENT;
                            device.cmd_push_constants(
                                cmd, layout, stages, 0, constants,
                            );

                            let descriptor_sets = [res[*buffer_set]];
                            device.cmd_bind_descriptor_sets(
                                cmd,
                                vk::PipelineBindPoint::GRAPHICS,
                                layout,
                                0,
                                &descriptor_sets,
                                &[],
                            );

                            device.cmd_draw(cmd, vertex_count as u32, 1, 0, 0);
                        }
                    }
                });

            device.cmd_end_render_pass(cmd);
        }

        //
    }

    pub fn draw(
        &self,
        // layer_names: Vec<rhai::ImmutableString>,
        framebuffer: FramebufferIx,
        extent: vk::Extent2D,
        // color_buffer_set: DescSetIx,
    ) -> Box<dyn Fn(&Device, &GpuResources, vk::CommandBuffer)> {
        let pass = self.pass;
        let pipeline = self.pipeline;
        let text_pipeline = self.text_pipeline;
        // let buf_ix = self.buf_ix;
        let layers = self.layers.clone();
        let layer_names: Vec<rhai::ImmutableString> =
            self.layer_order.read().iter().cloned().collect();
        // let layer_names = layer_names;

        Box::new(move |dev, res, cmd| {
            let layers = layers.clone();
            let layer_names = layer_names.clone();
            Self::draw_impl(
                layers,
                layer_names,
                pass,
                pipeline,
                text_pipeline,
                framebuffer,
                extent,
                dev,
                res,
                cmd,
            );
        })
    }
}

#[export_module]
pub mod script {

    use crate::console::EvalResult;

    use super::*;

    use parking_lot::RwLock;
    use rustc_hash::{FxHashMap, FxHashSet};
    use std::sync::Arc;

    pub type GuiLayers =
        Arc<RwLock<FxHashMap<rhai::ImmutableString, GuiLayer>>>;

    pub type GuiLayer = super::GuiLayer;

    pub type GuiLabel = super::GuiLabel;

    pub type LabelMsg = super::LabelMsg;

    #[rhai_fn(name = "set_visibility", global, set = "visibility")]
    pub fn msg_set_visibility(msg: &mut LabelMsg, vis: bool) {
        msg.set_visibility(vis);
    }

    #[rhai_fn(name = "set_position", global, set = "position")]
    pub fn msg_set_position(msg: &mut LabelMsg, pos: rhai::Map) {
        let get = |k: &str| {
            pos.get(k)
                .and_then(|v| {
                    v.as_int().ok().or(v.as_float().ok().map(|v| v as i64))
                })
                .unwrap_or_default()
        };

        let x = get("x");
        let y = get("y");

        msg.set_position(x as u32, y as u32);
    }

    #[rhai_fn(name = "set_contents", global, set = "contents")]
    pub fn msg_set_contents(
        msg: &mut LabelMsg,
        contents: rhai::ImmutableString,
    ) {
        msg.set_contents = Some(contents);
    }

    pub fn is_visible(label: &mut GuiLabel) -> bool {
        label.is_visible()
    }

    pub fn set_visibility(label: &mut GuiLabel, vis: bool) {
        label.set_visibility(vis);
    }

    #[rhai_fn(global)]
    pub fn set_rects(layer: &mut GuiLayer, new_rects: rhai::Array) {
        match &mut layer.rects {
            RectVertices::Palette { rects, .. } => {
                rects.clear();
                rects.extend(new_rects.into_iter().filter_map(|rect| {
                    let rect = rect.try_cast::<rhai::Map>()?;

                    let get_cast = |k: &str| {
                        let field = rect.get(k)?;

                        field
                            .as_int()
                            .ok()
                            .or(field.as_float().ok().map(|v| v as i64))
                    };

                    let x = get_cast("x")?;
                    let y = get_cast("y")?;
                    let w = get_cast("w")?;
                    let h = get_cast("h")?;
                    let c = get_cast("c")?;

                    Some(([x as f32, y as f32, w as f32, h as f32], c as u32))
                }));
            }
            RectVertices::Text { labels, .. } => {
                labels.clear();

                labels.extend(new_rects.into_iter().filter_map(|label| {
                    let label = label.try_cast::<rhai::Map>()?;

                    let get_cast_i = |k: &str| {
                        let field = label.get(k)?;
                        field.as_int().ok()
                    };

                    let get_cast_f = |k: &str| {
                        let field = label.get(k)?;
                        field.as_float().ok()
                    };

                    let x = get_cast_f("x")?;
                    let y = get_cast_f("y")?;
                    let txt_o = get_cast_i("text_offset")?;
                    let txt_l = get_cast_i("text_len")?;
                    let r = get_cast_f("r")?;
                    let g = get_cast_f("g")?;
                    let b = get_cast_f("b")?;

                    let pos = [x, y];
                    let txt = [txt_o as u32, txt_l as u32];
                    let color = [r, g, b, 1.0];

                    Some((pos, txt, color))
                }));
            }
        }
    }
}
