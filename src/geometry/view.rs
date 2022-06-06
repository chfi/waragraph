use std::collections::{BTreeMap, HashMap};

use std::ops::{Add, Sub};
use std::sync::Arc;

use parking_lot::RwLock;

use rhai::plugin::*;

use anyhow::{anyhow, bail, Result};

use crossbeam::atomic::AtomicCell;

use euclid::*;

use num_traits::{
    one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive,
};

pub struct PangenomeSpace;
pub type Bp = Length<usize, PangenomeSpace>;

pub struct ScreenSpace;
pub type ScreenLen = Length<f32, ScreenSpace>;
pub type ScreenPoint = Point2D<f32, ScreenSpace>;

pub type PixelsLen = Length<usize, ScreenSpace>;

pub type PangenomeScreenScale<T> = Scale<T, PangenomeSpace, ScreenSpace>;

pub type PangenomeView = View1D<Bp>;

impl View1D<Bp> {
    pub fn screen_scale(&self, pixel_len: f64) -> PangenomeScreenScale<f64> {
        let f = pixel_len / self.len.0 as f64;
        Scale::new(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct View1D<I>
where
    I: Copy + PartialEq + PartialOrd + Add<Output = I> + euclid::num::Zero,
{
    max: I,

    offset: I,
    len: I,
}

impl<I> View1D<I>
where
    I: Copy + PartialEq + PartialOrd + Add<Output = I> + euclid::num::Zero,
{
    pub fn new(max: I) -> Self {
        Self {
            max,
            offset: I::zero(),
            len: max,
        }
    }

    pub fn offset(&self) -> I {
        self.offset
    }

    pub fn len(&self) -> I {
        self.len
    }

    pub fn max(&self) -> I {
        self.max
    }

    pub fn is_valid(&self) -> bool {
        self.len > I::zero() && (self.offset + self.len <= self.max)
    }

    pub fn reset(&mut self) {
        self.offset = I::zero();
        self.len = self.max;
    }

    pub fn set(&mut self, offset: I, len: I) {
        assert!(len > I::zero());
        assert!(offset + len <= self.max);
        self.offset = offset;
        self.len = len;
    }
}

//
impl<I> View1D<I>
where
    I: Copy
        + PartialEq
        + PartialOrd
        + Add<Output = I>
        + Sub<Output = I>
        + euclid::num::Zero,
{
    pub fn set_offset(&self, new_offset: I) -> Self {
        let mut new = *self;

        if new_offset + self.len >= self.max {
            new.offset = self.max - self.len;
        } else {
            new.offset = new_offset;
        }

        new
    }

    pub fn shift_right(&self, delta: I) -> Self {
        let mut new = *self;

        if delta + self.offset + self.len >= self.max {
            new.offset = self.max - self.len;
        } else {
            new.offset = self.offset + delta;
        }

        new
    }

    pub fn shift_left(&self, delta: I) -> Self {
        let mut new = *self;

        if delta >= self.offset {
            new.offset = I::zero();
        } else {
            new.offset = self.offset - delta;
        }

        new
    }

    /// Returns a new `View1D<I>` with the same offset but a new length.
    pub fn resize_from_left(&self, new_len: I) -> Self {
        if self.offset + new_len >= self.max {
            Self {
                len: self.max - self.offset,
                ..*self
            }
        } else {
            Self {
                len: new_len,
                ..*self
            }
        }
    }

    pub fn resize_around<X>(&self, p: I, new_len: I) -> Self
    where
        I: std::ops::Div<Output = X> + std::ops::Mul<X>,
        X: ToPrimitive,
    {
        if new_len > self.len {
            // "zooming out"
            let fact = new_len / self.len;

            // let f_a = p / self.len;
            // let f_b = p / new_len;

            todo!();
        } else if new_len < self.len {
            // "zooming in"
            let fact = self.len / new_len;

            todo!();
        } else {
            *self
        }
    }

    /*
    /// Returns a new `View1D<I>` by resizing this view while keeping the right-hand side fixed.
    pub fn resize_from_right(&self, new_len: I) -> Self {
    }
    */
}

/*
impl<I> View1D<I>
where
    I: Copy
        + PartialEq
        + PartialOrd
        + Add<Output = I>
        + Sub<Output = I>
        + euclid::num::Zero,
{
    pub fn set_offset(&self, new_offset: I) -> Self {
        let mut new = *self;

        if new_offset + self.len >= self.max {
            new.offset = self.max - self.len;
        } else {
            new.offset = new_offset;
        }

        new
    }

    pub fn shift_right(&self, delta: I) -> Self {
        let mut new = *self;

        if delta + self.offset + self.len >= self.max {
            new.offset = self.max - self.len;
        } else {
            new.offset = self.offset + delta;
        }

        new
    }

    pub fn shift_left(&self, delta: I) -> Self {
        let mut new = *self;

        if delta >= self.offset {
            new.offset = I::zero();
        } else {
            new.offset = self.offset - delta;
        }

        new
    }
}
*/

/*
pub type PgView = View1D_<usize, PangenomeSpace>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct View1D_<I, U>
where
    I: Copy + PartialEq + PartialOrd + NumOps + Zero,
    // I: NumOps + One + Zero,
{
    offset: Length<I, U>,
    len: Length<I, U>,

    max: Length<I, U>,
}

impl<I, U> View1D_<I, U>
where
    I: Copy + PartialEq + PartialOrd + NumOps + Zero,
{
    pub fn new(max: I) -> Self {
        let max = Length::new(max);

        Self {
            max,
            offset: Length::new(I::zero()),
            len: max,
        }
    }

    pub fn offset(&self) -> Length<I, U> {
        self.offset
    }

    pub fn len(&self) -> Length<I, U> {
        self.len
    }

    pub fn max(&self) -> Length<I, U> {
        self.max
    }

    pub fn is_valid(&self) -> bool {
        self.len > Length::new(zero()) && (self.offset + self.len <= self.max)
    }

    pub fn reset(&mut self) {
        self.offset = Length::new(zero());
        self.len = self.max;
    }

    pub fn set(&mut self, offset: I, len: I) {
        assert!(len > I::zero());

        let offset = Length::new(offset);
        let len = Length::new(len);

        assert!(offset + len <= self.max);

        self.offset = offset;
        self.len = len;
    }
}
*/
