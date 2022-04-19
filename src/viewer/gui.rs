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
}

#[derive(Clone)]
pub struct GuiLayer {
    // rects: Arc<RwLock<RectVertices>>,
    name: rhai::ImmutableString,
    pub rects: RectVertices,

    // labels: FxHashMap<u64, Arc<AtomicCell<bool>>>,
    // labels: FxHashMap<rhai::ImmutableString, Arc<(u64, AtomicCell<bool>)>>,
    labels: FxHashMap<rhai::ImmutableString, GuiLabel>,

    vertex_buf_id: BufId,
    pub vertex_buf_ix: BufferIx,
}

#[derive(Clone)]
pub struct GuiLabel {
    layer: rhai::ImmutableString,
    label_id: u64,
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

    pub rhai_module: Arc<rhai::Module>,

    pub label_msg_tx: crossbeam::channel::Sender<LabelMsg>,
    pub label_msg_rx: crossbeam::channel::Receiver<LabelMsg>,

    msg_tx: crossbeam::channel::Sender<GuiMsg>,
    msg_rx: crossbeam::channel::Receiver<GuiMsg>,
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
            }

            //
        }

        Ok(())
    }

    // pub fn add_label(&mut self, name: &str)

    pub fn init(
        engine: &mut VkEngine,
        db: &sled::Db,
        buffers: &mut BufferStorage,
        width: u32,
        height: u32,
        // height: u32,
    ) -> Result<Self> {
        let mut config = ConfigMap::default();

        let mut labels = LabelStorage::new(&db)?;
        let label_updates = labels.tree.watch_prefix(b"t:");

        let (pass_ix, pipeline_ix) = {
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

                Ok((pass_ix, pipeline_ix))
            })?
        };

        let (label_msg_tx, label_msg_rx) = crossbeam::channel::unbounded();
        let (msg_tx, msg_rx) = crossbeam::channel::unbounded();

        // let mut module = rhai::Module::new();

        let mut module: rhai::Module = rhai::exported_module!(script);

        module.set_native_fn("label_msg", |layer: &str, label: &str| {
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
        module.set_native_fn("send_label_msg", move |msg: LabelMsg| {
            if let Err(e) = tx.send(msg) {
                log::error!("GUI send_label_msg error: {:?}", e);
                return Ok(rhai::Dynamic::FALSE);
            }

            Ok(rhai::Dynamic::TRUE)
        });

        let rhai_module = Arc::new(module);

        Ok(Self {
            config,

            layers: Arc::new(RwLock::new(FxHashMap::default())),

            labels,
            label_updates,

            pass: pass_ix,
            pipeline: pipeline_ix,

            rhai_module,

            label_msg_tx,
            label_msg_rx,

            msg_tx,
            msg_rx,
        })
    }

    pub fn draw_impl(
        layers: Arc<RwLock<FxHashMap<rhai::ImmutableString, GuiLayer>>>,
        layer_names: Vec<rhai::ImmutableString>,
        pass: RenderPassIx,
        pipeline: PipelineIx,
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
                    RectVertices::Palette { buffer_set, rects } => {
                        let vx_buf_ix = layer.vertex_buf_ix;

                        let vx_buf = res[vx_buf_ix].buffer;
                        let vxs = [vx_buf];

                        let vertex_count = rects.len() * 6;

                        device.cmd_bind_vertex_buffers(cmd, 0, &vxs, &[12]);

                        let dims = [extent.width as f32, extent.height as f32];

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
                });

            device.cmd_end_render_pass(cmd);
        }

        //
    }

    pub fn draw(
        &self,
        layer_names: Vec<rhai::ImmutableString>,
        framebuffer: FramebufferIx,
        extent: vk::Extent2D,
        // color_buffer_set: DescSetIx,
    ) -> Box<dyn Fn(&Device, &GpuResources, vk::CommandBuffer)> {
        let pass = self.pass;
        let pipeline = self.pipeline;
        // let buf_ix = self.buf_ix;
        let layers = self.layers.clone();
        let layer_names = layer_names;

        Box::new(move |dev, res, cmd| {
            let layers = layers.clone();
            let layer_names = layer_names.clone();
            Self::draw_impl(
                layers,
                layer_names,
                pass,
                pipeline,
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

    pub fn is_visible(label: &mut GuiLabel) -> bool {
        label.is_visible()
    }

    pub fn set_visibility(label: &mut GuiLabel, vis: bool) {
        label.set_visibility(vis);
    }

    pub fn set_rects(layer: &mut GuiLayer, new_rects: rhai::Array) {
        match &mut layer.rects {
            RectVertices::Palette { rects, .. } => {
                rects.clear();
                rects.extend(new_rects.into_iter().filter_map(|rect| {
                    let rect = rect.try_cast::<rhai::Map>()?;

                    let get_cast = |k: &str| {
                        let field = rect.get(k)?;
                        field.as_int().ok()
                    };

                    let x = get_cast("x")?;
                    let y = get_cast("y")?;
                    let w = get_cast("w")?;
                    let h = get_cast("h")?;
                    let c = get_cast("c")?;

                    Some(([x as f32, y as f32, w as f32, h as f32], c as u32))
                }));
            }
        }
    }

    #[rhai_fn(return_raw)]
    pub fn with_layer(
        ctx: NativeCallContext,
        layers: &mut GuiLayers,
        layer_name: &str,
        f: rhai::FnPtr,
    ) -> EvalResult<rhai::Dynamic> {
        let mut layers = layers.write();

        if let Some((layer_name, layer)) = layers.remove_entry(layer_name) {
            let mut layer = rhai::Dynamic::from(layer);

            if let Err(e) = f.call_raw(&ctx, Some(&mut layer), []) {
                log::error!("GUI with_layer error: {:?}", e);
            }

            let layer = layer.cast::<GuiLayer>();

            layers.insert(layer_name, layer);

            Ok(rhai::Dynamic::TRUE)
        } else {
            Ok(rhai::Dynamic::FALSE)
        }
    }
}
