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

    // macro_rules! {
    // module.
    // }

    module
}

#[export_module]
pub mod rhai_module {

    pub type Vec2 = [f32; 2];
    pub type Vec3 = [f32; 3];
    pub type Vec4 = [f32; 4];

    pub type IVec2 = [i32; 2];
    pub type IVec3 = [i32; 3];
    pub type IVec4 = [i32; 4];

    pub type UVec2 = [u32; 2];
    pub type UVec3 = [u32; 3];
    pub type UVec4 = [u32; 4];

    #[rhai_fn(pure, get = "x")]
    pub fn vec2_get_x(v: &mut Vec2) -> f32 {
        v[0]
    }

    #[rhai_fn(pure, get = "y")]
    pub fn vec2_get_y(v: &mut Vec2) -> f32 {
        v[1]
    }

    #[rhai_fn(pure, get = "x")]
    pub fn vec3_get_x(v: &mut Vec3) -> f32 {
        v[0]
    }

    #[rhai_fn(pure, get = "y")]
    pub fn vec3_get_y(v: &mut Vec3) -> f32 {
        v[1]
    }

    #[rhai_fn(pure, get = "z")]
    pub fn vec3_get_z(v: &mut Vec3) -> f32 {
        v[2]
    }

    #[rhai_fn(pure, get = "x")]
    pub fn vec4_get_x(v: &mut Vec4) -> f32 {
        v[0]
    }

    #[rhai_fn(pure, get = "y")]
    pub fn vec4_get_y(v: &mut Vec4) -> f32 {
        v[1]
    }

    #[rhai_fn(pure, get = "z")]
    pub fn vec4_get_z(v: &mut Vec4) -> f32 {
        v[2]
    }

    #[rhai_fn(pure, get = "w")]
    pub fn vec4_get_w(v: &mut Vec4) -> f32 {
        v[3]
    }
}
