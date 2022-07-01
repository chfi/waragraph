pub mod debug;
pub mod layer;
pub mod tree_list;

use crate::{
    geometry::{
        ScreenLength, ScreenPoint, ScreenRect, ScreenSideOffsets, ScreenSize,
    },
    text::TextCache,
};

use anyhow::Result;
use euclid::Rect;
use raving::{compositor::Compositor, vk::VkEngine};

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

pub struct Window {
    offset: ScreenPoint,

    bg_color: rgb::RGBA<f32>,
    border_color: rgb::RGBA<f32>,
    border_width: ScreenLength,
}
