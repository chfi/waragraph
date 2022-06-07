use raving::compositor::label_space::LabelSpace;
use raving::vk::VkEngine;

use rustc_hash::FxHashMap;

use crate::geometry::{ScreenPoint, ScreenRect};

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

use raving::compositor::{Compositor, SublayerAllocMsg};

use euclid::*;

use super::gui::layer::{label_at, line_width_rgba, rect_rgba};

#[derive(Debug, Clone)]
pub enum Shape {
    Rect(ScreenRect),
    Line {
        p0: ScreenPoint,
        p1: ScreenPoint,
        width: f32,
    },
    Label {
        p: ScreenPoint,
        text: rhai::ImmutableString,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    stroke: rgb::RGBA<f32>,
    fill: Option<rgb::RGBA<f32>>,
}

impl Shape {
    pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Self {
        Shape::Rect(Rect::new(Point2D::new(x, y), Size2D::new(w, h)))
    }

    pub fn line(x0: f32, y0: f32, x1: f32, y1: f32, width: f32) -> Self {
        let p0 = Point2D::new(x0, y0);
        let p1 = Point2D::new(x1, y1);
        Shape::Line { p0, p1, width }
    }

    pub fn label(x: f32, y: f32, text: &str) -> Self {
        let p = Point2D::new(x, y);
        Shape::Label {
            p,
            text: text.into(),
        }
    }
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

    pub fn fill_layer(
        &mut self,
        compositor: &mut Compositor,
        layer: usize,
        shapes: impl IntoIterator<Item = (Shape, rgb::RGBA<f32>)>,
    ) -> Result<()> {
        let layer_name = self.layer_names.get(&layer).unwrap();

        let mut rects: Vec<[u8; 32]> = Vec::new();
        let mut lines: Vec<[u8; 40]> = Vec::new();
        let mut texts: Vec<[u8; 32]> = Vec::new();

        for (shape, color) in shapes {
            match shape {
                Shape::Rect(rect) => rects.push(rect_rgba(rect, color)),
                Shape::Line { p0, p1, width } => {
                    lines.push(line_width_rgba(p0, p1, width, width, color))
                }
                Shape::Label { p, text } => {
                    let bounds =
                        self.label_space.bounds_for_insert(text.as_str())?;
                    texts.push(label_at(p, bounds, color));
                }
            }
        }

        compositor.with_layer(layer_name, |layer| {
            if let Some(data) = layer
                .get_sublayer_mut(Self::RECT_SUBLAYER)
                .and_then(|sl| sl.draw_data_mut().next())
            {
                data.update_vertices_array(rects)?;
            }

            if let Some(data) = layer
                .get_sublayer_mut(Self::LINE_SUBLAYER)
                .and_then(|sl| sl.draw_data_mut().next())
            {
                data.update_vertices_array(lines)?;
            }

            if let Some(data) = layer
                .get_sublayer_mut(Self::TEXT_SUBLAYER)
                .and_then(|sl| sl.draw_data_mut().next())
            {
                data.update_vertices_array(texts)?;
            }

            Ok(())
        })?;

        Ok(())
    }

    pub fn create_layer(
        &mut self,
        compositor: &mut Compositor,
        depth: usize,
    ) -> Result<usize> {
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

        let text_set = self.label_space.text_set;
        let msg = SublayerAllocMsg::new(
            layer_name.as_str(),
            Self::TEXT_SUBLAYER,
            "text",
            &[text_set],
        );
        compositor.sublayer_alloc_tx.send(msg)?;

        Ok(layer_id)
    }
}
