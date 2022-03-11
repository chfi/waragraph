use std::{collections::HashMap, num::NonZeroU32};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{context::VkContext, BufferIx, GpuResources, VkEngine};
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

use crate::viewer::ViewDiscrete1D;

pub fn create_engine(db: &sled::Db) -> rhai::Engine {
    //
    let mut engine = rhai::Engine::new();

    engine.register_type_with_name::<IVec>("IVec");

    engine.register_fn("write_u64", |v: &mut IVec, offset: i64, val: i64| {
        let val = val as u64;
        let o = offset as usize;
        if o + 8 <= v.len() {
            v[o..o + 8].clone_from_slice(&val.to_le_bytes());
        }
    });

    engine.register_result_fn(
        "subslice",
        |v: &mut IVec, offset: i64, len: i64| {
            let o = offset as usize;
            let l = len as usize;

            if o >= v.len() || o + l > v.len() {
                return Err("offset out of bounds".into());
            }

            Ok(v.subslice(o, l))
        },
    );

    engine.register_result_fn("as_u64", |v: &mut IVec| {
        u64::read_from(v.as_ref()).ok_or("bytestring is not u64".into())
    });

    engine.register_result_fn("as_u32", |v: &mut IVec| {
        u32::read_from(v.as_ref()).ok_or("bytestring is not u32".into())
    });

    let db_ = db.clone();
    engine.register_fn("get", move |k: &str| {
        let k = k.as_bytes();
        let v = db_.get(k).unwrap().unwrap();
        v
    });

    let db_ = db.clone();
    engine.register_fn("set", move |k: &str, v: IVec| {
        // let k = k.as_bytes();
        db_.insert(k, v).unwrap();
        // let v = db_.get(k).unwrap().unwrap();
        // v
    });

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
