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

    // module.set_iter(type_id, func)
    module.set_native_fn("add", |a: &mut [f32; 2], b: [f32; 2]| {
        Ok([a[0] + b[0], a[1] + b[1]])
    });
    // module.set_iterator::<[f32; 2]>();

    macro_rules! vec_math_fns {
        ($ty:ty) => {
            module.set_native_fn("add", |a: &mut [$ty; 2], b: [$ty; 2]| {
                Ok([a[0] + b[0], a[1] + b[1]])
            });
            module.set_native_fn("add", |a: &mut [$ty; 3], b: [$ty; 3]| {
                Ok([a[0] + b[0], a[1] + b[1], a[2] + b[2]])
            });
            module.set_native_fn("add", |a: &mut [$ty; 4], b: [$ty; 4]| {
                Ok([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]])
            });

            module.set_native_fn("sub", |a: &mut [$ty; 2], b: [$ty; 2]| {
                Ok([a[0] - b[0], a[1] - b[1]])
            });
            module.set_native_fn("sub", |a: &mut [$ty; 3], b: [$ty; 3]| {
                Ok([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
            });
            module.set_native_fn("sub", |a: &mut [$ty; 4], b: [$ty; 4]| {
                Ok([a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]])
            });
        };
    }

    module.set_native_fn("vec4", |x: f32, y: f32, z: f32, w: f32| {
        Ok([x, y, z, w])
    });

    macro_rules! vec_get_set {
        ($arr:ty, $ty:ty, [$(($name:literal, $ix:literal)),*]) => {
            $(
                module.set_getter_fn($name, |v: &mut $arr| Ok(v[$ix]));
                module.set_setter_fn($name, |v: &mut $arr, x: $ty| {
                    v[$ix] = x;
                    Ok(())
                });
            )*
        };
    }

    macro_rules! impl_get_set {
        ($ty:ty) => {
            vec_get_set!([$ty; 2], $ty, [("x", 0), ("y", 1)]);
            vec_get_set!([$ty; 3], $ty, [("x", 0), ("y", 1), ("z", 2)]);
            vec_get_set!(
                [$ty; 4],
                $ty,
                [("x", 0), ("y", 1), ("z", 2), ("w", 3)]
            );
        };
    }

    impl_get_set!(f32);
    impl_get_set!(u32);
    impl_get_set!(i32);

    vec_math_fns!(f32);
    vec_math_fns!(u32);
    vec_math_fns!(i32);

    module.set_getter_fn("x", |v: &mut [f32; 2]| Ok(v[0]));

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
}
