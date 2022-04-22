use std::{collections::HashMap, num::NonZeroU32};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
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
    viewer::ViewDiscrete1D,
};

use rhai::plugin::*;

use lazy_static::lazy_static;

pub fn create_rhai_module() -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(rhai_module);

    module
}

#[export_module]
pub mod rhai_module {
    use rhai::plugin::RhaiResult;

    use std::sync::Arc;

    use crate::{
        console::EvalResult,
        graph::{Node, Waragraph},
        viewer::{DataSource, SlotUpdateFn},
    };

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
