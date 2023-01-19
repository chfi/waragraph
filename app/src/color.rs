use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BufferUsages,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColorSchemeId(usize);

pub struct ColorStore {
    scheme_name_map: HashMap<String, ColorSchemeId>,
    color_schemes: Vec<ColorScheme>,

    scheme_buffers: HashMap<ColorSchemeId, Arc<wgpu::Buffer>>,

    mapping_buffers: BTreeMap<ColorMapping, Arc<wgpu::Buffer>>,
}

impl ColorStore {
    pub fn get_color_scheme_id(&self, name: &str) -> Option<ColorSchemeId> {
        self.scheme_name_map.get(name).copied()
    }

    pub fn get_color_scheme(&self, id: ColorSchemeId) -> &ColorScheme {
        &self.color_schemes[id.0]
    }

    pub fn init() -> Self {
        let mut result = Self {
            scheme_name_map: HashMap::default(),
            color_schemes: Vec::new(),

            scheme_buffers: HashMap::default(),
            mapping_buffers: BTreeMap::default(),
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

    pub fn get_color_mapping_gpu_buffer(
        &mut self,
        state: &raving_wgpu::State,
        mapping: ColorMapping,
    ) -> Option<Arc<wgpu::Buffer>> {
        if !self.mapping_buffers.contains_key(&mapping) {
            let usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST;
            let buf_data = mapping.into_uniform_bytes();

            let buffer =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&buf_data),
                    usage,
                });

            self.mapping_buffers.insert(mapping, Arc::new(buffer));
        }

        let buf = self.mapping_buffers.get(&mapping)?;
        Some(buf.clone())
    }

    pub fn get_color_scheme_gpu_buffer(
        &self,
        id: ColorSchemeId,
    ) -> Option<Arc<wgpu::Buffer>> {
        let buf = self.scheme_buffers.get(&id)?;
        Some(buf.clone())
    }

    pub fn upload_color_schemes_to_gpu(
        &mut self,
        state: &raving_wgpu::State,
    ) -> anyhow::Result<()> {
        let mut need_upload = Vec::new();

        for (ix, _scheme) in self.color_schemes.iter().enumerate() {
            let id = ColorSchemeId(ix);
            if !self.scheme_buffers.contains_key(&id) {
                need_upload.push(id);
            }
        }

        let mut data: Vec<u8> = Vec::new();

        let buffer_usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;

        for id in need_upload {
            data.clear();
            let scheme = self.color_schemes.get(id.0).unwrap();
            data.resize(scheme.required_buffer_size(), 0u8);
            scheme.fill_buffer(&mut data);

            let buffer =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: data.as_slice(),
                    usage: buffer_usage,
                });

            self.scheme_buffers.insert(id, Arc::new(buffer));
        }

        Ok(())
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

        self.scheme_name_map.insert(name.to_string(), id);
        self.color_schemes.push(scheme);

        id
    }
}

/// A `ColorScheme` is a sequence of colors
pub struct ColorScheme {
    pub id: ColorSchemeId,
    pub colors: Vec<[f32; 4]>,
}

impl ColorScheme {
    pub fn required_buffer_size(&self) -> usize {
        let elem_count = self.colors.len();
        let elem_size = std::mem::size_of::<[f32; 4]>();

        // the uniform itself only has a single u32 before the colors,
        // but we need to pad to get the alignment correct
        let prefix_size = std::mem::size_of::<u32>() * 4;

        prefix_size + elem_count * elem_size
    }

    fn fill_buffer(&self, buf: &mut [u8]) {
        assert!(buf.len() >= self.required_buffer_size());

        let len = self.colors.len() as u32;

        let data_start = 4 * 4;

        let data_end =
            data_start + self.colors.len() * std::mem::size_of::<[f32; 4]>();

        buf[0..data_start]
            .clone_from_slice(bytemuck::cast_slice(&[len, 0, 0, 0]));
        buf[data_start..data_end]
            .clone_from_slice(bytemuck::cast_slice(&self.colors));
    }
}

/// Defines a mapping from values (as 32-bit floats) to indices in a `ColorScheme`.
/// The range `[min_val, max_val]` in the domain will be mapped to the color index
/// range `[min_color_ix, max_color_ix]` (linear interpolation between midpoints of
/// index range, then round)
#[derive(Debug, Clone, Copy)]
pub struct ColorMapping {
    pub color_scheme: ColorSchemeId,

    pub min_color_ix: u32,
    pub max_color_ix: u32,

    pub extreme_min_color_ix: u32,
    pub extreme_max_color_ix: u32,

    pub min_val: f32,
    pub max_val: f32,
}

impl ColorMapping {
    pub fn into_uniform_bytes(self) -> [u8; 24] {
        let mut out = [0u8; 24];

        out[0..(4 * 4)].clone_from_slice(bytemuck::cast_slice(&[
            self.min_color_ix,
            self.max_color_ix,
            self.extreme_min_color_ix,
            self.extreme_max_color_ix,
        ]));
        out[(4 * 4)..(6 * 4)].clone_from_slice(bytemuck::cast_slice(&[
            self.min_val,
            self.max_val,
        ]));

        out
    }

    pub fn new(
        color_scheme: ColorSchemeId,
        color_range: std::ops::RangeInclusive<u32>,
        val_range: std::ops::RangeInclusive<f32>,
        extreme_min_color_ix: u32,
        extreme_max_color_ix: u32,
    ) -> Self {
        let (min_color_ix, max_color_ix) = color_range.into_inner();
        let (min_val, max_val) = val_range.into_inner();

        Self {
            color_scheme,

            min_color_ix,
            max_color_ix,

            extreme_min_color_ix,
            extreme_max_color_ix,

            min_val,
            max_val,
        }
    }
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

impl PartialOrd for ColorMapping {
    // messy but uses the total ordering on the f32 values
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let t1_u = (
            &self.color_scheme,
            &self.min_color_ix,
            &self.max_color_ix,
            &self.extreme_min_color_ix,
            &self.extreme_max_color_ix,
        );
        let t1_f = (&self.min_val, &self.max_val);

        let t2_u = (
            &other.color_scheme,
            &other.min_color_ix,
            &other.max_color_ix,
            &other.extreme_min_color_ix,
            &other.extreme_max_color_ix,
        );

        let t2_f = (&other.min_val, &other.max_val);

        let u = t1_u.cmp(&t2_u);

        let f = (t1_f.0.total_cmp(&t2_f.0)).cmp(&t1_f.1.total_cmp(&t2_f.1));

        Some(u.cmp(&f))
    }
}

impl Ord for ColorMapping {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(&other).unwrap()
    }
}

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
