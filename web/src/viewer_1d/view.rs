use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct View1D {
    range: std::ops::Range<f64>,

    max: f64,
}

#[wasm_bindgen]
impl View1D {
    pub fn new(max: u64) -> Self {
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

    pub fn make_valid(&mut self) {
        let len = self.len();

        if self.range.end > self.max() {
            self.range.end = self.max();
        }

        // let max_offset = self.max.checked_sub(len).unwrap_or_default();
        let max_offset = (self.max - len).max(0.0);
        if self.start() > max_offset {
            self.range.start = max_offset;
        }
    }
}

impl View1D {
    pub fn range_bp(&self) -> std::ops::Range<u64> {
        let start = self.range.start.floor() as u64;
        let end = self.range.end.ceil() as u64;
        start..end
    }
}
