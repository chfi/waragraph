use bimap::BiHashMap;
use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::compositor::label_space::LabelSpace;
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
use crate::geometry::view::{ScreenPoint, ScreenRect};
use crate::graph::{Node, Path, Waragraph};
use crate::util::{BufFmt, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use raving::compositor::{Compositor, Layer, Sublayer, SublayerAllocMsg};

pub enum Shape {
    Rect(ScreenRect),
    Line {
        p0: ScreenPoint,
        p1: ScreenPoint,
    },
    Label {
        p: ScreenPoint,
        text: rhai::ImmutableString,
    },
}

pub struct Style {
    color: [f32; 4],
}

struct DebugLayer {
    // rects: Vec<Rect>,
    // lines: Vec<Line>,
    // labels: Vec<Label>,
}

pub struct DebugLayers {
    pub name_prefix: String,
    // pub layers: Vec<rhai::ImmutableString>,
    pub layer_names: FxHashMap<usize, rhai::ImmutableString>,

    label_space: LabelSpace,

    next_layer_id: usize,
    // pub rect_name: rhai::ImmutableString,
    // pub text_name: rhai::ImmutableString,
    // pub line_name: rhai::ImmutableString,
    // pub rect_layers: Vec<(usize, Vec<
}

impl DebugLayers {
    const RECT_SUBLAYER: &'static str = "RECT";
    const TEXT_SUBLAYER: &'static str = "TEXT";
    const LINE_SUBLAYER: &'static str = "LINE";

    pub fn new(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        name: &str,
        depth: usize,
    ) -> Result<Self> {
        let name_prefix = format!("DEBUG_LAYER<{}>", name);

        let label_space = LabelSpace::new(
            engine,
            &format!("{}_labels", name_prefix),
            1024 * 1024,
        )?;

        let mut res = Self {
            label_space,
            name_prefix,
            layer_names: FxHashMap::default(),
            next_layer_id: 0,
        };

        res.create_layer(compositor, depth)?;

        Ok(res)
    }

    pub fn update(&mut self, engine: &mut VkEngine) -> Result<()> {
        // TODO this label space method can't really fail, should the signature
        let _ = self.label_space.write_buffer(&mut engine.resources);

        Ok(())
    }

    pub fn create_layer(
        &mut self,
        compositor: &mut Compositor,
        depth: usize,
    ) -> Result<rhai::ImmutableString> {
        let layer_id = self.next_layer_id;

        let layer_name = rhai::ImmutableString::from(format!(
            "{}_{}",
            self.name_prefix, layer_id
        ));

        compositor.new_layer(&layer_name, depth, true);

        self.layer_names.insert(layer_id, layer_name.clone());
        self.next_layer_id += 1;

        let text_set = self.label_space.text_set;
        let msg = SublayerAllocMsg::new(
            layer_name.as_str(),
            Self::TEXT_SUBLAYER,
            "text",
            &[text_set],
        );
        compositor.sublayer_alloc_tx.send(msg)?;

        let msg = SublayerAllocMsg::new(
            layer_name.as_str(),
            Self::RECT_SUBLAYER,
            "rect-rgb",
            &[],
        );
        compositor.sublayer_alloc_tx.send(msg)?;

        let msg = SublayerAllocMsg::new(
            layer_name.as_str(),
            Self::LINE_SUBLAYER,
            "line-rgb",
            &[],
        );
        compositor.sublayer_alloc_tx.send(msg)?;

        Ok(layer_name)
    }
}
