pub mod debug;
pub mod layer;
pub mod tree_list;

use std::borrow::Cow;

use crate::{
    geometry::{
        ScreenLength, ScreenPoint, ScreenRect, ScreenSideOffsets, ScreenSize,
    },
    gui::layer::{line_width_rgba2, rect_rgba},
    text::TextCache,
};

use anyhow::Result;
use euclid::Rect;
use glyph_brush::{GlyphCruncher, OwnedSection, Text};
use nalgebra::{Point2, Vector2};
use raving::{
    compositor::{Compositor, SublayerAllocMsg},
    vk::VkEngine,
};

// pub struct WindowId

#[derive(Default, Clone, Copy)]
pub struct AreaBounds {
    width: Option<f32>,
    height: Option<f32>,
}

impl AreaBounds {
    pub fn max_width(&self) -> Option<f32> {
        self.width
    }

    pub fn max_height(&self) -> Option<f32> {
        self.height
    }

    pub fn from_max(width: f32, height: f32) -> Self {
        assert!(
            width > 0.0 && height > 0.0,
            "AreaBounds max bounds must be positive, were ({}, {})",
            width,
            height
        );
        Self {
            // `None` is already used to represent no bounds, i.e.
            // infinity, so better to ensure `Some(bound)` is always
            // finite in the constructors
            width: (width != std::f32::INFINITY).then(|| width),
            height: (height != std::f32::INFINITY).then(|| height),
        }
    }

    pub fn from_max_width(width: f32) -> Self {
        assert!(
            width > 0.0,
            "AreaBounds max width must be positive, was {}",
            width
        );

        Self {
            width: (width != std::f32::INFINITY).then(|| width),
            height: None,
        }
    }

    pub fn from_max_height(height: f32) -> Self {
        assert!(
            height > 0.0,
            "AreaBounds max height must be positive, was {}",
            height
        );

        Self {
            width: None,
            height: (height != std::f32::INFINITY).then(|| height),
        }
    }
}

#[derive(Clone)]
pub struct Area {
    bounds: AreaBounds,

    width: f32,
    height: f32,
}

impl Area {
    pub fn new(bounds: AreaBounds) -> Self {
        Self {
            bounds,

            width: 0.0,
            height: 0.0,
        }
    }

    pub fn bounds(&self) -> AreaBounds {
        self.bounds
    }

    pub fn size(&self) -> ScreenSize {
        euclid::size2(self.width, self.height)
    }

    pub fn rect(&self, origin: ScreenPoint) -> ScreenRect {
        ScreenRect::new(origin, self.size())
    }

    // returns a rectangle as an origin (top left) + vector, in the local `Area`
    pub fn allocate_rect(
        &mut self,
        top_left: Point2<f32>,
        width: f32,
        height: f32,
    ) -> Result<(Point2<f32>, Vector2<f32>)> {
        //
        todo!();
    }
}

#[derive(Clone)]
pub enum Drawable {
    Text {
        section: OwnedSection,
    },
    // only unfilled for now
    Polygon {
        points: Vec<ScreenPoint>,
        color: rgb::RGBA<f32>,
        width: ScreenLength,
    },
    Rect {
        border: Option<(rgb::RGBA<f32>, f32)>,
        fill: Option<rgb::RGBA<f32>>,
        rect: ScreenRect,
    },
    /*
    RawVertices {
        sublayer_name: rhai::ImmutableString,
        vertices: Vec<u8>,
        bounding_box: ScreenRect,
    },
    */
}

pub struct Window {
    id: u64,
    layer_name: rhai::ImmutableString,

    offset: ScreenPoint,
    size: ScreenSize,

    bg_color: rgb::RGBA<f32>,
    border_color: rgb::RGBA<f32>,
    border_width: f32,

    drawables: Vec<Drawable>,
}

impl Window {
    const RECT_SUBLAYER: &'static str = "rects";
    const LINE_SUBLAYER: &'static str = "lines";
    const GLYPH_SUBLAYER: &'static str = "glyphs";

    pub fn new(
        compositor: &mut Compositor,
        id: u64,
        layer_name: &str,
        layer_depth: usize,
        offset: ScreenPoint,
    ) -> Result<Self> {
        // add layer
        compositor.new_layer(layer_name, layer_depth, true);

        // allocate sublayers
        for (name, def) in [
            (Self::RECT_SUBLAYER, "rect-rgb"),
            (Self::LINE_SUBLAYER, "line-rgb-2"),
            (Self::GLYPH_SUBLAYER, "glyph"),
        ] {
            let msg = SublayerAllocMsg::new(layer_name, name, def, &[]);
            compositor.sublayer_alloc_tx.send(msg)?;
        }

        let bg_color = rgb::RGBA::new(0.7, 0.7, 0.7, 1.0);
        let border_color = rgb::RGBA::new(0.0, 0.0, 0.0, 1.0);

        let mut res = Self {
            id,
            layer_name: layer_name.into(),

            offset,
            size: ScreenSize::new(100.0, 100.0),

            bg_color,
            border_color,
            border_width: 1.0,

            drawables: Vec::new(),
        };

        res.drawables.push(Drawable::Rect {
            border: None,
            fill: Some(rgb::RGBA::new(0.6, 0.1, 0.6, 1.0)),
            rect: euclid::rect(offset.x + 40.0, offset.y + 40.0, 40.0, 40.0),
        });

        res.drawables.push(Drawable::Text {
            section: OwnedSection::default()
                .add_text(
                    &Text::new("whoa hello").with_color([0.9, 0.9, 0.9, 1.0]),
                )
                .with_screen_position((120.0, 120.0)),
        });

        Ok(res)
    }

    fn layer_name(&self) -> &rhai::ImmutableString {
        &self.layer_name
    }

    pub fn update_layer(
        &self,
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        text_cache: &mut TextCache,
    ) -> Result<()> {
        // initialize sublayer vertex arrays

        let mut rect_buf: Vec<[u8; 32]> = Vec::new();
        let mut line_buf: Vec<[u8; 56]> = Vec::new();
        // let mut glyph_buf: Vec<[u8; 48]> = Vec::new();

        // map drawables to vertices and distribute to vertex arrays

        // window rectangle background
        {
            let bg_rect =
                ScreenRect::new(self.offset, euclid::size2(300.0, 300.0));
            let brd_rect = bg_rect.inflate(1.0, 1.0);

            rect_buf.push(rect_rgba(brd_rect, self.border_color));
            rect_buf.push(rect_rgba(bg_rect, self.bg_color));
        }

        for obj in &self.drawables {
            match obj {
                Drawable::Text { section } => {
                    // queue the glyph

                    // let section: Cow<OwnedSection> = section.into();

                    if let Some(rect) = text_cache.brush.glyph_bounds(section) {
                        //

                        text_cache.queue(section);
                    }
                }
                Drawable::Polygon {
                    points,
                    color,
                    width,
                } => {
                    if points.len() < 2 {
                        continue;
                    }

                    let mut points = points.iter().copied();

                    let mut p0 = points.next().unwrap();

                    for p1 in points {
                        let vx = line_width_rgba2(
                            p0, p1, width.0, width.0, *color, *color,
                        );
                        line_buf.push(vx);
                        p0 = p1;
                    }
                }
                Drawable::Rect { border, fill, rect } => {
                    if let Some((color, width)) = border {
                        let brd_rect = rect.inflate(*width, *width);
                        rect_buf.push(rect_rgba(brd_rect, *color));
                    }

                    if let Some(color) = fill {
                        rect_buf.push(rect_rgba(*rect, *color));
                    }
                }
            }
        }

        text_cache.process_queued(engine, compositor)?;

        // update sublayers with vertex array data
        compositor.with_layer(self.layer_name(), |layer| {
            if let Some(sublayer_data) = layer
                .get_sublayer_mut(Self::RECT_SUBLAYER)
                .and_then(|sub| sub.draw_data_mut().next())
            {
                log::warn!("updating rect sublayer");
                sublayer_data.update_vertices_array(rect_buf)?;
            }

            if let Some(sublayer_data) = layer
                .get_sublayer_mut(Self::LINE_SUBLAYER)
                .and_then(|sub| sub.draw_data_mut().next())
            {
                log::warn!("updating line sublayer");
                sublayer_data.update_vertices_array(line_buf)?;
            }

            if let Some(sublayer_data) = layer
                .get_sublayer_mut(Self::GLYPH_SUBLAYER)
                .and_then(|sub| sub.draw_data_mut().next())
            {
                log::warn!("updating glyph sublayer");
                text_cache.update_sublayer(sublayer_data)?;
            }

            Ok(())
        })?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct WindowGeometry {
    pub origin: ScreenPoint,
    pub size: ScreenSize,
    pub side_offsets: ScreenSideOffsets,
}

impl WindowGeometry {
    pub fn available_height(&self) -> ScreenLength {
        //
        todo!();
    }

    /// Returns the rectangle that will contain the list slots (i.e.
    /// with `side_offsets` taken into account)
    pub fn inner_rect(&self) -> ScreenRect {
        let rect = Rect::new(self.origin, self.size);
        rect.inner_rect(self.side_offsets)
    }

    /// Returns the rectangle that includes the area removed by the
    /// `side_offsets`
    pub fn rect(&self) -> ScreenRect {
        Rect::new(self.origin, self.size)
    }
}

pub struct WindowManager {
    text_cache: TextCache,

    // TODO: starting out with just one window since the text cache
    // will need some changes to easily support multiple, possibly
    // overlapping, window rendering
    active_window: Option<()>,
}

impl WindowManager {
    pub fn init(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
    ) -> Result<Self> {
        //
        todo!();
    }
}
