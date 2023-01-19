use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColorSchemeId(usize);

pub struct ColorStore {
    scheme_name_map: HashMap<String, ColorSchemeId>,
    color_schemes: Vec<ColorScheme>,

    scheme_buffers: HashMap<ColorSchemeId, wgpu::Buffer>,
    mapping_buffers: HashMap<String, wgpu::Buffer>,
}

impl ColorStore {
    pub fn init() -> Self {
        let mut result = Self {
            scheme_name_map: HashMap::default(),
            color_schemes: Vec::new(),

            scheme_buffers: HashMap::default(),
            mapping_buffers: HashMap::default(),
        };

        let rgba = |r: u8, g: u8, b: u8| {
            let max = u8::MAX as f32;
            [r as f32 / max, g as f32 / max, b as f32 / max, 1.0]
        };

        let spectral = [
            rgba(255, 255, 255),
            rgba(196, 196, 196),
            rgba(128, 128, 128),
            rgba(158, 1, 66),
            rgba(213, 62, 79),
            rgba(244, 109, 67),
            rgba(253, 174, 97),
            rgba(254, 224, 139),
            rgba(255, 255, 191),
            rgba(230, 245, 152),
            rgba(171, 221, 164),
            rgba(102, 194, 165),
            rgba(50, 136, 189),
            rgba(94, 79, 162),
        ];
        result.add_color_scheme("spectral", spectral);

        let black_red = [rgba(255, 255, 255), rgba(0, 0, 0), rgba(255, 0, 0)];

        result.add_color_scheme("black_red", black_red);

        result
    }

    pub fn add_color_scheme(
        &mut self,
        name: &str,
        colors: impl IntoIterator<Item = [f32; 4]>,
    ) -> ColorSchemeId {
        let id = ColorSchemeId(self.color_schemes.len());

        let scheme = ColorScheme {
            id,
            colors: colors.into_iter().collect(),
        };

        id
    }
}

/// A `ColorScheme` is a sequence of colors
pub struct ColorScheme {
    id: ColorSchemeId,
    colors: Vec<[f32; 4]>,
}

/// Defines a mapping from values (as 32-bit floats) to indices in a `ColorScheme`.
/// The range `[min_val, max_val]` in the domain will be mapped to the color index
/// range `[min_color_ix, max_color_ix]` (linear interpolation between midpoints of
/// index range, then round)
#[derive(Debug, Clone, Copy)]
pub struct ColorMapping {
    color_scheme: ColorSchemeId,

    min_color_ix: u32,
    max_color_ix: u32,

    min_val: f32,
    max_val: f32,

    extreme_min_color_ix: u32,
    extreme_max_color_ix: u32,
}

impl PartialEq for ColorMapping {
    fn eq(&self, other: &Self) -> bool {
        self.color_scheme == other.color_scheme
            && self.min_color_ix == other.min_color_ix
            && self.max_color_ix == other.max_color_ix
            && self.min_val == other.min_val
            && self.max_val == other.max_val
            && self.extreme_min_color_ix == other.min_color_ix
            && self.extreme_max_color_ix == other.max_color_ix
    }
}

impl Eq for ColorMapping {}

impl ColorMapping {
    pub fn get_ix(&self, val: f32) -> u32 {
        if val < self.min_val {
            self.extreme_min_color_ix
        } else if val > self.max_val {
            self.extreme_max_color_ix
        } else {
            use ultraviolet::interp::Lerp;
            let t = (val - self.min_val) / (self.max_val - self.min_val);
            let min = (self.min_color_ix as f32) + 0.5;
            let max = (self.max_color_ix as f32) + 0.5;
            min.lerp(max, t).round() as u32
        }
    }
}
