use std::{collections::HashMap, num::NonZeroU32};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use parking_lot::RwLock;
use raving::{
    script::console::BatchBuilder,
    vk::{context::VkContext, BufferIx, GpuResources, VkEngine},
};
use rustc_hash::FxHashMap;

use sled::IVec;
use thunderdome::{Arena, Index};

use sprs::{CsMatI, CsVecI, TriMatI};
use zerocopy::{AsBytes, FromBytes};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use bstr::ByteSlice as BstrByteSlice;

use crate::{
    util::{BufFmt, BufId, BufMeta, BufferStorage, LabelStorage},
    viewer::{DataSource, SlotFnCache, ViewDiscrete1D},
};

use rhai::plugin::*;

use lazy_static::lazy_static;

use self::rhai_module::DataSourceF32;

pub fn create_rhai_module() -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(rhai_module);

    module
}

pub fn add_cache_fns(
    module: &mut rhai::Module,
    slot_fns: &Arc<RwLock<SlotFnCache>>,
) {
    let cache = slot_fns.clone();
    module.set_native_fn(
        "set_slot_color_scheme",
        move |slot_fn: rhai::ImmutableString,
              color_buffer: rhai::ImmutableString| {
            cache.write().slot_color.insert(slot_fn, color_buffer);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn("get_slot_color_scheme", move |slot_fn: &str| {
        if let Some(color) = cache.read().slot_color.get(slot_fn) {
            Ok(rhai::Dynamic::from(color.to_owned()))
        } else {
            Ok(rhai::Dynamic::FALSE)
        }
    });

    let cache = slot_fns.clone();
    module.set_native_fn(
        "register_data_source",
        move |name: rhai::ImmutableString, f: DataSource<u32>| {
            cache.write().data_sources_u32.insert(name, f);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "register_data_source",
        move |name: rhai::ImmutableString, f: DataSource<f32>| {
            cache.write().data_sources_f32.insert(name, f);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "register_data_source",
        move |name: rhai::ImmutableString, f: DataSource<rhai::Dynamic>| {
            cache.write().data_sources_dyn.insert(name, f);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "get_data_source",
        move |name: rhai::ImmutableString| {
            let cache = cache.read();
            if let Some(data) = cache.data_sources_u32.get(&name) {
                Ok(rhai::Dynamic::from(data.clone()))
            } else if let Some(data) = cache.data_sources_f32.get(&name) {
                Ok(rhai::Dynamic::from(data.clone()))
            } else if let Some(data) = cache.data_sources_dyn.get(&name) {
                Ok(rhai::Dynamic::from(data.clone()))
            } else {
                Ok(rhai::Dynamic::FALSE)
            }
        },
    );
    //
}

// pub fn add_channel_fns(module: &mut rhai::Module,

// pub fn add_channel_fns_engine(engine: &mut rhai::Engine,

#[export_module]
pub mod rhai_module {
    use rhai::plugin::RhaiResult;

    use std::sync::Arc;

    use crate::{
        console::EvalResult,
        graph::{Node, Waragraph},
        viewer::{DataSource, SlotUpdateFn},
    };

    pub type SlotFnCache = Arc<RwLock<crate::viewer::SlotFnCache>>;

    pub type ArcBytestring = Arc<Vec<u8>>;

    pub type DataSourceDyn = DataSource<rhai::Dynamic>;

    pub type DataSourceF32 = DataSource<f32>;
    pub type DataSourceU32 = DataSource<u32>;
    pub type DataSourceI32 = DataSource<i32>;
    pub type DataSourceI64 = DataSource<i64>;

    pub type SlotUpdateFnU32 = SlotUpdateFn<u32>;
    pub type SlotUpdateFnF32 = SlotUpdateFn<f32>;

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_dyn(
        d: &mut DataSource<rhai::Dynamic>,
        path: i64,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path as usize, Node::from(node as u32)) {
            v
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_f32(
        d: &mut DataSource<f32>,
        path: i64,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path as usize, Node::from(node as u32)) {
            rhai::Dynamic::from_float(v)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_u32(
        d: &mut DataSource<u32>,
        path: i64,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path as usize, Node::from(node as u32)) {
            rhai::Dynamic::from_int(v as i64)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_i64(
        d: &mut DataSource<i64>,
        path: i64,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path as usize, Node::from(node as u32)) {
            rhai::Dynamic::from_int(v)
        } else {
            rhai::Dynamic::UNIT
        }
    }
}
