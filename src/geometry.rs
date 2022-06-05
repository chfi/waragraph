use bstr::ByteSlice;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use parking_lot::RwLock;

use rhai::plugin::*;

use rhai::ImmutableString;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use crossbeam::atomic::AtomicCell;

use euclid::*;

// pub enum LayoutInput {
//     ScalarInt(i32),
//     ScalarUInt(u32),
//     ScalarFloat(f32),
// }

pub struct Pixels {}

#[derive(Clone, Copy)]
pub struct ListLayout {
    pub origin: Point2D<f32, Pixels>,
    pub size: Size2D<f32, Pixels>,
    pub side_offsets: Option<SideOffsets2D<f32, Pixels>>,

    pub slot_height: Length<f32, Pixels>,
}

impl ListLayout {
    /// Returns the rectangle that will contain the list slots (i.e.
    /// with `side_offsets` taken into account)
    pub fn inner_rect(&self) -> Rect<f32, Pixels> {
        todo!();
    }

    /// Returns the rectangle for the slot at the given index. If `ix`
    /// is pointing to a slot beyond the available height, `None` is
    /// returned.
    pub fn slot_rect(&self, ix: usize) -> Option<Rect<f32, Pixels>> {
        todo!();
    }

    // the output can then be mapped to vertices
    pub fn apply_to_rows<'a, T: 'a>(
        &self,
        rows: impl Iterator<Item = &'a T> + 'a,
    ) -> impl Iterator<Item = (usize, Rect<f32, Pixels>, &'a T)> + 'a {
        todo!();
    }
}

// impl ListLayout {
// }

// pub trait Layout {
//     fn apply<F>(&self, f: F) -> ()
//     where
//         F: FnMut();
// }

#[export_module]
pub mod rhai_module {
    //

    pub type Point2DF = euclid::Point2D<f32, euclid::UnknownUnit>;
    pub type Point2DI = euclid::Point2D<i64, euclid::UnknownUnit>;

    pub type Vec2DF = euclid::Vector2D<f32, euclid::UnknownUnit>;
    pub type Vec2DI = euclid::Vector2D<i64, euclid::UnknownUnit>;
}
