use raving::compositor::label_space::LabelSpace;
use raving::vk::VkEngine;

use rustc_hash::FxHashMap;

use crate::geometry::{ScreenPoint, ScreenRect};

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

use raving::compositor::{Compositor, SublayerAllocMsg};

#[derive(Debug, Clone)]
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

// pub struct DebugLayer {
//     id: usize,
//     shapes: Vec<Shape>,
// }

pub struct DebugLayers {
    pub name_prefix: String,
    // pub layers: Vec<rhai::ImmutableString>,
    pub layer_names: FxHashMap<usize, rhai::ImmutableString>,
    // pub layers: HashMap<rhai::ImmutableString, DebugLayer>,
    label_space: LabelSpace,

    next_layer_id: usize,
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
            // layers: HashMap::default(),
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

    // pub fn update_layer_from

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

        // self.layers.insert(
        //     layer_name.clone(),
        //     DebugLayer {
        //         id: layer_id,
        //         shapes: Vec::new(),
        //     },
        // );

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
