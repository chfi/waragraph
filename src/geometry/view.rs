use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use parking_lot::RwLock;

use rhai::plugin::*;

use anyhow::{anyhow, bail, Result};

use crossbeam::atomic::AtomicCell;

use euclid::*;

use num_traits::{
    one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive, Zero,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct View1D<I>
where
    I: Copy + PartialEq + PartialOrd + NumOps + One + Zero,
    // I: NumOps + One + Zero,
{
    max: I,

    offset: I,
    len: I,
}

impl<I> View1D<I>
where
    I: Copy + PartialEq + PartialOrd + NumOps + One + Zero,
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
