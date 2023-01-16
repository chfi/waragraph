// use palette::LinSrgba;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColorSchemeId(usize);

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
