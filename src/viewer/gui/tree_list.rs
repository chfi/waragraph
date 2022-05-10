use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::compositor::label_space::LabelSpace;
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

use raving::compositor::Compositor;

pub struct TreeList {
    pub offset: Arc<AtomicCell<[f32; 2]>>,

    pub list: Vec<rhai::Dynamic>,
    // pub list: Vec<(String, usize)>,
    pub label_space: LabelSpace,

    layer_name: rhai::ImmutableString,

    sublayer_rect: rhai::ImmutableString,
    sublayer_text: rhai::ImmutableString,
    // rhai_module: Arc<rhai::Module>,
}

#[derive(Debug, Default, Clone)]
pub struct Breadcrumbs {
    stack: smallvec::SmallVec<[u16; 11]>,
}

impl Breadcrumbs {
    pub fn pop_back(&self) -> Option<(u16, Self)> {
        if self.is_empty() {
            return None;
        }

        match self.stack.split_at(1) {
            (&[v], rest) => Some((v, Self { stack: rest.into() })),
            _ => unreachable!(),
        }
    }

    pub fn index_dyn_ref(&self, val: &rhai::Dynamic) -> Option<rhai::Dynamic> {
        if self.is_empty() {
            return Some(val.clone());
        }

        if val.type_name() != "array" {
            return None;
        }

        let array = val.read_lock::<rhai::Array>()?;

        match self.pop_back() {
            None => None,
            Some((ix, crumbs)) => {
                let val = array.get(ix as usize)?;
                crumbs.index_dyn_ref(val)
            }
        }
    }

    pub fn index_dyn(&self, val: rhai::Dynamic) -> Option<rhai::Dynamic> {
        if self.is_empty() {
            return Some(val);
        }

        if val.type_name() != "array" {
            return None;
        }

        let mut array = val.cast::<rhai::Array>();

        let last_level = self.len();

        for (level, ix) in self.stack.iter().enumerate() {
            let ix = *ix as usize;

            let val = (ix < array.len()).then(|| array.swap_remove(ix))?;

            if level + 1 == last_level {
                return Some(val);
            }

            if val.type_name() != "array" {
                return None;
            }

            array = val.cast();
        }

        None
    }

    pub fn append(&self, v: u16) -> Self {
        let mut stack = self.stack.clone();
        stack.push(v);
        Self { stack }
    }

    pub fn push(&mut self, v: u16) {
        self.stack.push(v)
    }

    pub fn pop(&mut self) -> Option<u16> {
        self.stack.pop()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl TreeList {
    fn label_vertices(
        offset: [f32; 2],
        index: usize,
        indent: f32,
        text_bounds: (usize, usize),
    ) -> [u8; 32] {
        let [x0, y0] = offset;

        let (s, l) = text_bounds;
        let color = [0.0f32, 0.0, 0.0, 1.0];

        let x = x0 + 1.0 + indent;
        let y = y0 + 1.0 + (10.0 * index as f32);

        let mut out = [0u8; 8 + 8 + 16];
        out[0..8].clone_from_slice([x, y].as_bytes());
        out[8..16].clone_from_slice([s as u32, l as u32].as_bytes());
        out[16..32].clone_from_slice(color.as_bytes());

        out
    }

    fn rect_vertices(&self, index: usize, indent: f32, width: f32) -> [u8; 32] {
        let [x0, y0] = self.offset.load();

        let color = if index % 2 == 0 {
            [0.85f32, 0.85, 0.85, 1.0]
        } else {
            [0.75f32, 0.75, 0.75, 1.0]
        };

        let h = 10.0;

        let x = x0;
        let y = y0 + h * index as f32;

        let mut out = [0u8; 32];
        out[0..8].clone_from_slice([x, y].as_bytes());
        out[8..16].clone_from_slice([width, h].as_bytes());
        out[16..32].clone_from_slice(color.as_bytes());
        out
    }

    pub fn update_layer(
        &mut self,
        compositor: &mut Compositor,
        mouse_pos: [f32; 2],
    ) -> Result<()> {
        let offset = self.offset.load();
        let [x0, y0] = self.offset.load();

        let layer_name = self.layer_name.clone();

        compositor.with_layer(&layer_name, |layer| {
            let mut max_label_len = 0;

            let mut total_width = 0.0;

            let mut max_depth = 0;
            let mut max_sum = 0;

            let mut row_stack: Vec<(Breadcrumbs, rhai::Dynamic)> = Vec::new();

            for (i, val) in self.list.iter().enumerate() {
                let mut crumbs = Breadcrumbs::default();
                crumbs.push(i as u16);
                row_stack.push((crumbs, val.clone()));
            }

            let mut rows = Vec::new();

            while let Some((crumbs, val)) = row_stack.pop() {
                max_depth = max_depth.max(crumbs.len());

                match val.type_name() {
                    "bool" => {
                        let v = val.as_bool().unwrap();
                        let bounds = self
                            .label_space
                            .bounds_for_insert(&v.to_string())
                            .unwrap();
                        rows.push((crumbs.clone(), bounds));
                    }
                    "i64" => {
                        let int = val.as_int().unwrap();
                        let bounds = self
                            .label_space
                            .bounds_for_insert(&int.to_string())
                            .unwrap();
                        rows.push((crumbs.clone(), bounds));
                    }
                    "f32" => {
                        let float = val.as_float().unwrap();
                        let bounds = self
                            .label_space
                            .bounds_for_insert(&float.to_string())
                            .unwrap();
                        rows.push((crumbs.clone(), bounds));
                    }
                    "string" => {
                        let text = val.clone_cast::<rhai::ImmutableString>();

                        max_label_len = max_label_len.max(text.len());

                        let bounds =
                            self.label_space.bounds_for_insert(&text).unwrap();

                        rows.push((crumbs.clone(), bounds));
                    }
                    "array" => {
                        let array = val.clone_cast::<rhai::Array>();

                        let mut i = 0u16;

                        for val in array {
                            let crumbs = crumbs.append(i);
                            row_stack.push((crumbs, val));
                            i += 1;
                        }
                    }
                    "map" => {
                        let map = val.clone_cast::<rhai::Map>();

                        let mut i = 0u16;

                        for (key, val) in map {
                            if val.type_name() == "string" {}
                        }
                    }
                    _ => {
                        //
                    }
                }
            }

            rows.reverse();

            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_text)
            {
                sublayer.update_vertices_array(rows.iter().enumerate().map(
                    |(i, (crumbs, bounds))| {
                        let depth = 10.0 * crumbs.len() as f32;
                        Self::label_vertices(offset, i, depth, *bounds)
                    },
                ))?;
            }

            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_rect)
            {
                let w = 4.0
                    + (10.0 * max_depth as f32)
                    + 8.0 * max_label_len as f32;
                let h = 4.0 + 8.0 * max_sum as f32;

                let mut bg = [0u8; 8 + 8 + 16];
                bg[0..8].clone_from_slice([x0, y0].as_bytes());
                bg[8..16].clone_from_slice([w, h].as_bytes());
                bg[16..32]
                    .clone_from_slice([0.85f32, 0.85, 0.85, 1.0].as_bytes());

                sublayer.update_vertices_array_range(0..1, [bg])?;

                sublayer.update_vertices_array(Some(bg).into_iter().chain(
                    rows.iter().enumerate().map(|(i, (crumbs, _bounds))| {
                        let depth = 10.0 * (crumbs.len() - 1) as f32;
                        self.rect_vertices(i, depth, w)
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
