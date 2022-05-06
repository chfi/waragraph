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

use super::layer::Compositor;

#[derive(Clone)]
pub struct LabelSpace {
    name: rhai::ImmutableString,

    offsets: BTreeMap<rhai::ImmutableString, (usize, usize)>,

    text: Vec<u8>,

    capacity: usize,
    used_bytes: usize,

    pub text_buffer: BufferIx,
    pub text_set: DescSetIx,
}

impl LabelSpace {
    pub fn new(
        engine: &mut VkEngine,
        name: &str,
        capacity: usize,
    ) -> Result<Self> {
        let name = format!("label-space:{}", name);

        let (text_buffer, text_set) =
            engine.with_allocators(|ctx, res, alloc| {
                let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
                let usage = vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST;

                let buffer = res.allocate_buffer(
                    ctx,
                    alloc,
                    mem_loc,
                    4,
                    capacity / 4,
                    usage,
                    Some(&name),
                )?;

                let buf_ix = res.insert_buffer(buffer);

                let desc_set =
                    crate::util::allocate_buffer_desc_set(buf_ix, res)?;

                let set_ix = res.insert_desc_set(desc_set);

                Ok((buf_ix, set_ix))
            })?;

        Ok(Self {
            name: name.into(),

            offsets: BTreeMap::default(),
            text: Vec::new(),

            capacity,
            used_bytes: 0,

            text_buffer,
            text_set,
        })
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.used_bytes = 0;
    }

    pub fn write_buffer(&self, res: &mut GpuResources) -> Option<()> {
        if self.used_bytes == 0 {
            return Some(());
        }
        let buf = &mut res[self.text_buffer];
        let slice = buf.mapped_slice_mut()?;
        slice[0..self.used_bytes].clone_from_slice(&self.text);
        Some(())
    }

    pub fn insert(&mut self, text: &str) -> Result<()> {
        self.bounds_for_insert(text)?;
        Ok(())
    }

    pub fn bounds_for(&self, text: &str) -> Option<(usize, usize)> {
        self.offsets.get(text).copied()
    }

    pub fn bounds_for_insert(&mut self, text: &str) -> Result<(usize, usize)> {
        if let Some(bounds) = self.offsets.get(text) {
            return Ok(*bounds);
        }

        let offset = self.used_bytes;
        let len = text.as_bytes().len();

        if self.used_bytes + len > self.capacity {
            anyhow::bail!("Label space out of memory");
        }

        let bounds = (offset, len);

        self.text.extend(text.as_bytes());
        self.offsets.insert(text.into(), bounds);

        self.used_bytes += len;

        Ok(bounds)
    }
}

pub struct TreeList {
    pub offset: Arc<AtomicCell<[f32; 2]>>,

    // list: Vec<rhai::Dynamic>,
    pub list: Vec<(String, usize)>,

    pub label_space: LabelSpace,

    layer_name: rhai::ImmutableString,

    sublayer_rect: rhai::ImmutableString,
    sublayer_text: rhai::ImmutableString,
    // rhai_module: Arc<rhai::Module>,
}

impl TreeList {
    pub fn update_layer(&mut self, compositor: &mut Compositor) -> Result<()> {
        let [x0, y0] = self.offset.load();

        compositor.with_layer(&self.layer_name, |layer| {
            let mut max_label_len = 0;

            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_text)
            {
                sublayer.update_vertices_array(
                    self.list.iter().enumerate().map(|(i, (text, v))| {
                        max_label_len = max_label_len.max(text.len());

                        let (s, l) =
                            self.label_space.bounds_for_insert(text).unwrap();

                        let h = 10.0;

                        let x = x0;
                        let y = y0 + h * i as f32;

                        let color = [0.0f32, 0.0, 0.0, 1.0];

                        let mut out = [0u8; 8 + 8 + 16];
                        out[0..8].clone_from_slice([x, y].as_bytes());
                        out[8..16]
                            .clone_from_slice([s as u32, l as u32].as_bytes());
                        out[16..32].clone_from_slice(color.as_bytes());
                        out
                    }),
                )?;
            }

            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_rect)
            {
                let w = 4.0 + 8.0 * max_label_len as f32;
                let h = 4.0 + 8.0 * self.list.len() as f32;

                let mut bg = [0u8; 8 + 8 + 16];
                bg[0..8].clone_from_slice([x0, y0].as_bytes());
                bg[8..16].clone_from_slice([w, h].as_bytes());
                bg[16..32]
                    .clone_from_slice([0.85f32, 0.85, 0.85, 1.0].as_bytes());

                sublayer.update_vertices_array_range(0..1, [bg])?;

                sublayer.update_vertices_array(Some(bg).into_iter().chain(
                    self.list.iter().enumerate().map(|(i, (s, v))| {
                        let color = if i % 2 == 0 {
                            [0.85f32, 0.85, 0.85, 1.0]
                        } else {
                            [0.75f32, 0.75, 0.75, 1.0]
                        };

                        let h = 10.0;

                        let x = x0;
                        let y = y0 + h * i as f32;

                        let mut out = [0u8; 32];
                        out[0..8].clone_from_slice([x, y].as_bytes());
                        out[8..16].clone_from_slice([w, h].as_bytes());
                        out[16..32].clone_from_slice(color.as_bytes());
                        out
                    }),
                ))?;
            }

            Ok(())
        })?;

        Ok(())
    }

    pub fn new(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        x: f32,
        y: f32,
    ) -> Result<Self> {
        let label_space =
            LabelSpace::new(engine, "tree-list-labels", 4 * 1024 * 1024)?;

        let layer_name = "tree-list-layer";
        let rect_name = "tree-list:rect";
        let text_name = "tree-list:text";

        let offset = Arc::new(AtomicCell::new([x, y]));

        compositor.new_layer(layer_name, 1, true);

        compositor.with_layer(layer_name, |layer| {
            Compositor::push_sublayer(
                &compositor.sublayer_defs,
                engine,
                layer,
                "rect-rgb",
                rect_name,
                None,
            )?;

            Compositor::push_sublayer(
                &compositor.sublayer_defs,
                engine,
                layer,
                "text",
                text_name,
                [label_space.text_set],
            )?;

            Ok(())
        });

        Ok(Self {
            offset,

            list: Vec::new(),
            label_space,

            layer_name: layer_name.into(),

            sublayer_rect: rect_name.into(),
            sublayer_text: text_name.into(),
        })
    }
}

#[export_module]
pub mod rhai_module {
    use parking_lot::RwLock;

    use crate::console::EvalResult;

    pub type LabelSpace = Arc<RwLock<super::LabelSpace>>;

    /*
    #[rhai_fn(global)]
    pub fn bounds_for(
        labels: &mut LabelSpace,
        text: rhai::ImmutableString,
    ) -> EvalResult<std::ops::Range<i64>> {
        // let mut space = labels.write();
        todo!();
    }
    */

    #[rhai_fn(global, return_raw)]
    pub fn label_rects(
        label_space: &mut LabelSpace,
        labels: rhai::Array,
    ) -> EvalResult<Vec<[u8; 4 * 8]>> {
        let mut space = label_space.write();

        let mut result = Vec::with_capacity(labels.len());

        let get_f32 = |map: &rhai::Map, k: &str| -> EvalResult<f32> {
            map.get(k).and_then(|v| v.as_float().ok()).ok_or_else(|| {
                format!("map key `{}` must be a float", k).into()
            })
        };

        for label in labels {
            let mut map = label
                .try_cast::<rhai::Map>()
                .ok_or("array elements must be maps")?;

            let x = get_f32(&map, "x")?;
            let y = get_f32(&map, "y")?;

            let color = [
                get_f32(&map, "r")?,
                get_f32(&map, "g")?,
                get_f32(&map, "b")?,
                get_f32(&map, "a")?,
            ];

            let text = map
                .remove("contents")
                .and_then(|v| v.into_string().ok())
                .ok_or("`contents` key must be a string")?;

            let (s, l) = space.bounds_for_insert(&text).unwrap();

            let mut vertex = [0u8; 4 * 8];
            vertex[0..8].clone_from_slice([x, y].as_bytes());
            vertex[8..16].clone_from_slice([s as u32, l as u32].as_bytes());
            vertex[16..32].clone_from_slice(color.as_bytes());
            result.push(vertex);
        }

        Ok(result)
    }

    #[rhai_fn(global, return_raw)]
    pub fn batch_upload_labels(
        labels: &mut LabelSpace,
        texts: rhai::Array,
    ) -> EvalResult<()> {
        let mut space = labels.write();

        for text in texts {
            let text = text.into_immutable_string()?;
            if let Err(e) = space.insert(&text) {
                return Err(format!("LabelSpace batch error: {:?}", e).into());
            }
        }

        Ok(())
    }
}
