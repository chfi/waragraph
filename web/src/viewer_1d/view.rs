use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct View1D {
    range: std::ops::Range<f64>,

    max: f64,
}

#[wasm_bindgen]
impl View1D {
    pub fn new(start: f64, end: f64, max: u64) -> Self {
        let max = max as f64;
        let s = start.clamp(0.0, max);
        let e = end.clamp(s, max);

        Self { range: s..e, max }
    }

    pub fn new_full(max: u64) -> Self {
        let max = max as f64;
        let view_range = 0f64..max;

        Self {
            range: view_range,
            max,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn start(&self) -> f64 {
        self.range.start
    }

    #[wasm_bindgen(getter)]
    pub fn end(&self) -> f64 {
        self.range.end
    }

    #[wasm_bindgen(getter)]
    pub fn len(&self) -> f64 {
        self.range.end - self.range.start
    }

    #[wasm_bindgen(getter)]
    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn set(&mut self, start: f64, end: f64) {
        self.range = start..end;
        self.make_valid();
    }

    pub fn make_valid(&mut self) {
        let len = self.len();

        if self.range.start < 0.0 {
            self.range.start = 0.0;
            self.range.end = len;
        }

        if self.range.end > self.max() {
            self.range.end = self.max();
        }

        // let max_offset = self.max.checked_sub(len).unwrap_or_default();
        let max_offset = (self.max - len).max(0.0);
        if self.start() > max_offset {
            self.range.start = max_offset;
        }
    }

    pub fn translate(&mut self, delta: f64) {
        let len = self.len();

        self.range.start += delta;
        self.range.end += delta;

        self.make_valid();
    }

    pub fn translate_norm(&mut self, fdelta: f64) {
        self.translate(fdelta * self.len());
    }

    pub fn zoom_with_focus(&mut self, t: f64, s: f64) {
        let l0 = self.range.start;
        let r0 = self.range.end;

        let v = r0 - l0;

        let x = l0 + t * v;

        let p_l = t;
        let p_r = 1.0 - t;

        let mut v_ = v * s;

        // just so things don't implode
        if v_ < 1.0 {
            v_ = 1.0;
        }

        let x_l = p_l * v_;
        let x_r = p_r * v_;

        let l1 = x - x_l;
        let r1 = x + x_r;

        let max = self.max;

        let l = l1.min(r1).clamp(0.0, max);
        let r = r1.max(l1).clamp(0.0, max);

        self.range = l..r;
    }
}

impl View1D {
    pub fn range_bp(&self) -> std::ops::Range<u64> {
        let start = self.range.start.floor() as u64;
        let end = self.range.end.ceil() as u64;
        start..end
    }
}
