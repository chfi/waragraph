use raving::compositor::label_space::LabelSpace;
use raving::vk::VkEngine;

use rustc_hash::FxHashMap;

use crate::geometry::{ScreenPoint, ScreenRect};

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

use raving::compositor::{Compositor, SublayerAllocMsg};

use euclid::*;

use super::layer::{label_at, line_width_rgba, rect_rgba};

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

impl From<ScreenRect> for Shape {
    fn from(r: ScreenRect) -> Self {
        Shape::Rect(r)
    }
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

#[derive(Debug, Clone, Copy)]
pub struct Style {
    stroke: Option<rgb::RGBA<f32>>,
    fill: Option<rgb::RGBA<f32>>,
}

impl Style {
    pub fn stroke(color: rgb::RGBA<f32>) -> Self {
        Style {
            stroke: Some(color),
            fill: None,
        }
    }

    pub fn fill(color: rgb::RGBA<f32>) -> Self {
        Style {
            stroke: None,
            fill: Some(color),
        }
    }

    pub fn stroke_fill(stroke: rgb::RGBA<f32>, fill: rgb::RGBA<f32>) -> Self {
        Style {
            stroke: Some(stroke),
            fill: Some(fill),
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
        shapes: impl IntoIterator<Item = (Shape, Style)>,
    ) -> Result<()> {
        let layer_name = self.layer_names.get(&layer).unwrap();

        let mut rects: Vec<[u8; 32]> = Vec::new();
        let mut lines: Vec<[u8; 40]> = Vec::new();
        let mut texts: Vec<[u8; 32]> = Vec::new();

        for (shape, style) in shapes {
            match shape {
                Shape::Rect(rect) => {
                    if let Some(stroke) = style.stroke {
                        let w = rect.size.width;
                        let h = rect.size.height;

                        let p0 = rect.origin;
                        let p1 = p0 + vec2(w, 0.0);
                        let p2 = p0 + vec2(w, h);
                        let p3 = p0 + vec2(0.0, h);

                        for (a, b) in [(p0, p1), (p1, p2), (p2, p3), (p3, p0)] {
                            lines.push(line_width_rgba(a, b, 0.5, 0.5, stroke))
                        }
                    }

                    if let Some(fill) = style.fill {
                        rects.push(rect_rgba(rect, fill))
                    }
                }
                Shape::Line { p0, p1, width } => {
                    if let Some(stroke) = style.stroke {
                        lines
                            .push(line_width_rgba(p0, p1, width, width, stroke))
                    }
                }
                Shape::Label { p, text } => {
                    if let Some(stroke) = style.stroke {
                        let bounds = self
                            .label_space
                            .bounds_for_insert(text.as_str())?;
                        texts.push(label_at(p, bounds, stroke));
                    }

                    if let Some(fill) = style.fill {
                        let h = 8f32;
                        let w = (text.len() * 8) as f32;

                        rects.push(rect_rgba(rect(p.x, p.y, w, h), fill));
                    }
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
