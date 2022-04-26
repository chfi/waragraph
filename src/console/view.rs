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

#[export_module]
pub mod rhai_module {
    use crate::viewer::ViewDiscrete1D;

    pub type View1D = ViewDiscrete1D;

    #[rhai_fn(get = "len", pure)]
    pub fn get_len(view: &mut View1D) -> i64 {
        view.len as i64
    }
    #[rhai_fn(get = "offset", pure)]
    pub fn get_offset(view: &mut View1D) -> i64 {
        view.offset as i64
    }

    #[rhai_fn(get = "max", pure)]
    pub fn get_max(view: &mut View1D) -> i64 {
        view.max as i64
    }

    #[rhai_fn(set = "offset")]
    pub fn set_offset(view: &mut View1D, offset: i64) {
        let offset = offset.abs() as usize;
        view.offset = offset.min(view.max - view.len);
    }
}
