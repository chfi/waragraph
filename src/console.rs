use std::{collections::HashMap, num::NonZeroU32};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{context::VkContext, BufferIx, GpuResources, VkEngine};
use rustc_hash::FxHashMap;

use thunderdome::{Arena, Index};

use sprs::{CsMatI, CsVecI, TriMatI};
use zerocopy::AsBytes;

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use crate::viewer::ViewDiscrete1D;

pub fn create_engine(db: &sled::Db) -> rhai::Engine {
    //
    let mut engine = rhai::Engine::new();

    let db_ = db.clone();
    engine.register_fn("view", move || {
        let raw = db_.get(b"view").unwrap().unwrap();
        ViewDiscrete1D::from_bytes(&raw)
    });

    let db_ = db.clone();
    engine.register_fn("set_view_offset", move |new: i64| {
        let offset = new.abs() as usize;
        let raw = db_.get(b"view").unwrap().unwrap();
        let mut view = ViewDiscrete1D::from_bytes(&raw).unwrap();
        view.offset = offset.clamp(0, view.max() - view.len());
        log::warn!("new view offset: {}", view.offset);
        let bytes = view.as_bytes();
        db_.update_and_fetch(b"view", |_| Some(&bytes)).unwrap();
    });

    let db_ = db.clone();
    engine.register_fn("view_offset", move || {
        let raw = db_.get(b"view").unwrap().unwrap();
        let view = ViewDiscrete1D::from_bytes(&raw).unwrap();
        view.offset() as i64
    });
    // let tree =

    engine
}

pub fn eval<T: Clone + Send + Sync + 'static>(
    db: &sled::Db,
    script: &str,
) -> Result<T> {
    let engine = create_engine(db);
    match engine.eval(script) {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow!("eval err: {:?}", err)),
    }
}
