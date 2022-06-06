use std::collections::{BTreeMap, HashMap};

use std::ops::Add;
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

fn testin() {
    let max: Bp = Length::new(10_000);

    // let x: usize = Zero::zero();
    // let y: Bp = Zero::zero();
    // let z: Bp = zero();
    let view = View1D::new(max);
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
    // I: Copy + PartialEq + PartialOrd + NumOps + Zero,
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
