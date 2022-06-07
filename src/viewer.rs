use zerocopy::FromBytes;

pub mod app;
pub mod gui;

pub mod cache;
pub mod slots;

pub mod edges;

pub mod debug;

pub use slots::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewDiscrete1D {
    pub max: usize,
    pub offset: usize,
    pub len: usize,
}

impl ViewDiscrete1D {
    pub fn view_pos_norm(&self, pos: usize) -> f64 {
        let p = pos as f64;
        let o = self.offset as f64;
        let l = self.len as f64;
        (p - o) / l
    }

    pub fn screen_x(&self, x_offset: f64, width: f64, pos: usize) -> f64 {
        let x = x_offset;
        let fact = width / (self.len as f64);
        x + (pos as f64 - self.offset as f64) * fact
    }

    pub fn as_bytes(&self) -> [u8; 24] {
        let max = self.max.to_le_bytes();
        let offset = self.offset.to_le_bytes();
        let len = self.len.to_le_bytes();

        let mut result = [0; 24];
        result[0..8].clone_from_slice(&max);
        result[8..16].clone_from_slice(&offset);
        result[16..24].clone_from_slice(&len);
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let max = bytes.get(0..8)?;
        let offset = bytes.get(8..16)?;
        let len = bytes.get(16..24)?;

        let max = usize::read_from(max)?;
        let offset = usize::read_from(offset)?;
        let len = usize::read_from(len)?;

        Some(Self { max, offset, len })
    }

    pub fn new(max: usize) -> Self {
        Self {
            max,

            offset: 0,
            len: max,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn is_valid(&self) -> bool {
        self.len > 0 && (self.offset + self.len <= self.max)
    }

    pub fn reset(&mut self) {
        self.offset = 0;
        self.len = self.max;
    }

    pub fn set(&mut self, offset: usize, len: usize) {
        assert!(len > 0);
        assert!(offset + len <= self.max);
        self.offset = offset;
        self.len = len;
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..(self.offset + self.len)
    }

    pub fn translate(&mut self, delta: isize) {
        let d = delta.abs() as usize;

        // let offset = (self.offset as isize) + delta;
        // let offset = offset.clamp(0, (self.max - self.len) as isize);
        // self.offset = offset as usize;

        if delta.is_negative() {
            if d > self.offset {
                self.offset = 0;
            } else {
                self.offset -= d;
            }
        } else if delta.is_positive() {
            self.offset += d;
            if self.offset + self.len >= self.max {
                self.offset = self.max - self.len;
            }
        }
    }

    pub fn resize(&mut self, mut new_len: usize) {
        new_len = new_len.clamp(1, self.max);

        let mid = self.offset + (self.len / 2);

        let new_hl = new_len / 2;

        self.len = new_len;
        if new_hl > mid {
            self.offset = 0;
        } else if mid + new_hl > self.max {
            self.offset = self.max - new_len;
        } else {
            self.offset = mid - new_hl;
        }
    }
}
