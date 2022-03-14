use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use sled::transaction::{TransactionError, TransactionResult};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use thunderdome::Index;

use crossbeam::atomic::AtomicCell;

use bstr::ByteSlice;

use zerocopy::{AsBytes, FromBytes};

use parking_lot::Mutex;

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

#[derive(Clone)]
pub struct BufMeta<N: AsRef<[u8]>> {
    name: N,
    fmt: BufFmt,
    capacity: usize,
    buffer_ix: BufferIx,
    storage_set_ix: DescSetIx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufFmt {
    UInt,
    SInt,
    Float,

    UVec2,

    UVec4,
    SVec4,
    FVec4,
}

macro_rules! as_fns {
    // ($val:expr, $refn:ident, $res:ty) => {
    ($val:expr, $refn:ident, $mutn:ident, $res:ty) => {
        pub fn $refn(&self, bytes: &[u8]) -> Option<&[$res]> {
            if *self != $val || !self.is_compatible(bytes) {
                return None;
            }

            let slice = unsafe {
                let ptr = bytes.as_ptr();
                let data = ptr.cast() as *const $res;
                let len = bytes.len() / self.size();
                std::slice::from_raw_parts(data, len)
            };

            Some(slice)
        }

        pub fn $mutn(&self, bytes: &mut [u8]) -> Option<&mut [$res]> {
            if *self != $val || !self.is_compatible(bytes) {
                return None;
            }

            let slice = unsafe {
                let ptr = bytes.as_mut_ptr();
                let data = ptr.cast() as *mut $res;
                let len = bytes.len() / self.size();
                std::slice::from_raw_parts_mut(data, len)
            };

            Some(slice)
        }
    };
}

impl BufFmt {
    pub fn from_fmt(fmt: [u8; 3]) -> Option<Self> {
        match &fmt[..] {
            /*
            // 1-4 bytes as u32
            b"1u1" => Some(Self::UInt),
            b"2u1" => Some(Self::UInt),
            b"4u1" => Some(Self::UInt),
            b"8u1" => Some(Self::UInt),
            */
            // float
            b"1f4" => Some(Self::Float),
            // uint
            b"1u4" => Some(Self::UInt),
            // int
            b"1i4" => Some(Self::SInt),

            // uvec4
            b"2u4" => Some(Self::UVec2),

            // vec4
            b"4f4" => Some(Self::FVec4),
            // uvec4
            b"4u4" => Some(Self::UVec4),
            // ivec4
            b"4i4" => Some(Self::SVec4),
            _ => None,
        }
    }

    pub const fn size(&self) -> usize {
        match self {
            BufFmt::UInt => 4,
            BufFmt::SInt => 4,
            BufFmt::Float => 4,
            BufFmt::UVec2 => 8,
            BufFmt::UVec4 => 16,
            BufFmt::SVec4 => 16,
            BufFmt::FVec4 => 16,
        }
    }

    pub fn as_slice<T: Copy + FromBytes>(&self, bytes: &[u8]) -> Option<&[T]> {
        if bytes.len() % self.size() != 0 {
            return None;
        }

        let slice = unsafe {
            let ptr = bytes.as_ptr();
            let data = ptr.cast() as *const T;
            let len = bytes.len() / self.size();
            std::slice::from_raw_parts(data, len)
        };

        Some(slice)
    }

    pub fn as_slice_mut<T: Copy + FromBytes>(
        &self,
        bytes: &mut [u8],
    ) -> Option<&mut [T]> {
        if bytes.len() % self.size() != 0 {
            return None;
        }

        let slice = unsafe {
            let ptr = bytes.as_mut_ptr();
            let data = ptr.cast() as *mut T;
            let len = bytes.len() / self.size();
            std::slice::from_raw_parts_mut(data, len)
        };

        Some(slice)
    }

    pub fn map<T, F>(&self, bytes: &mut [u8], mut f: F) -> Option<&mut [T]>
    where
        T: Copy + FromBytes,
        F: FnMut(&mut T),
    {
        let slice_mut = self.as_slice_mut::<T>(bytes)?;

        for v in slice_mut.iter_mut() {
            f(v);
        }

        Some(slice_mut)
    }

    pub fn map_assign<T, F>(
        &self,
        bytes: &mut [u8],
        mut f: F,
    ) -> Option<&mut [T]>
    where
        T: Copy + FromBytes,
        F: FnMut(T) -> T,
    {
        let slice_mut = self.as_slice_mut::<T>(bytes)?;

        for v in slice_mut.iter_mut() {
            *v = f(*v);
        }

        Some(slice_mut)
    }

    // TODO these should be put in a macro or handled using const generics
    as_fns!(BufFmt::UInt, as_uint_ref, as_uint_mut, u32);
    as_fns!(BufFmt::UInt, as_uvec2_ref, as_uvec2_mut, [u32; 2]);
    as_fns!(BufFmt::UInt, as_uvec4_ref, as_uvec4_mut, [u32; 4]);

    pub fn is_compatible(&self, bytes: &[u8]) -> bool {
        let good_len = bytes.len() % self.size() == 0;

        good_len
    }

    /*

    pub fn is_compatible_vec<T, const N: usize>(&self, bytes: &[u8]) -> bool
    where
        T: Copy,
    {
        if bytes.len() % std::mem::size_of::<T>() == 0 {
            let inner_len = bytes.len() / std::mem::size_of::<T>();
            return inner_len % N == 0;
        }
        true
    }
    */
}

// pub struct BufFmt {
//     fmt: [u8;3],
// }

pub struct BufferStorage {
    pub tree: sled::Tree,

    buffers: Vec<BufferIx>,
    desc_sets: Vec<DescSetIx>,
}

macro_rules! key_fn {
    // ($fn_name:ident, $init:expr, $offset:literal, $out_len:literal) => {
    ($fn_name:ident, $out:ty, $init:expr, $offset:literal) => {
        const fn $fn_name(id: u64) -> $out {
            let src = id.to_le_bytes();
            let mut key = $init;

            let mut i = 0;
            while i < 8 {
                let s = src[0];
                key[$offset + i] = s;
                i += 1;
            }
            key
        }
    };
}

impl BufferStorage {
    const ID_NAME_MASK: [u8; 10] = *b"I:01234567";
    // use scan_prefix to iterate through all names and IDs, i guess
    const NAME_ID_PREFIX: &'static [u8] = b"buffer_id:";

    const BUF_DATA_MASK: [u8; 10] = *b"d:01234567";
    // used to store e.g. "[u8;2]"; basically a simple schema
    const BUF_FMT_MASK: [u8; 10] = *b"f:01234567";
    const BUF_CAP_MASK: [u8; 10] = *b"c:01234567";

    const BUF_IX_MASK: [u8; 10] = *b"B:01234567";
    const SET_IX_MASK: [u8; 10] = *b"S:01234567";

    const VEC_ID_MASK: [u8; 10] = *b"i:01234567";

    /*
    const fn buf_ix_key(id: u64) -> [u8; 10] {
        let src = id.to_le_bytes();
        let mut key = Self::BUF_IX_MASK;

        let mut i = 0;
        while i < 8 {
            let s = src[0];
            key[2 + i] = s;
            i += 1;
        }
        key
    }
    */

    key_fn!(buf_ix_key, [u8; 10], Self::BUF_IX_MASK, 2);
    key_fn!(set_ix_key, [u8; 10], Self::SET_IX_MASK, 2);

    key_fn!(data_key, [u8; 10], Self::BUF_DATA_MASK, 2);
    key_fn!(fmt_key, [u8; 10], Self::BUF_FMT_MASK, 2);

    key_fn!(vec_id_key, [u8; 10], Self::VEC_ID_MASK, 2);

    /*
      each name gets mapped to a u64 sled id
    */

    pub fn new(db: &sled::Db) -> Result<Self> {
        let tree = db.open_tree("buffer_storage")?;

        let buffers = Vec::new();
        let desc_sets = Vec::new();

        Ok(Self {
            tree,
            buffers,
            desc_sets,
        })
    }

    pub fn allocate_buffer(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        name: &str,
        fmt: BufFmt,
        capacity: usize,
    ) -> Result<u64> {
        let elem_size = fmt.size();

        let id = db.generate_id()?;

        let (buf, set) = engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
            let usage = vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST;

            let buffer = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                elem_size,
                capacity,
                usage,
                Some(name),
            )?;

            let buf_ix = res.insert_buffer(buffer);

            let desc_set = crate::util::allocate_buffer_desc_set(buf_ix, res)?;

            let set_ix = res.insert_desc_set(desc_set);

            Ok((buf_ix, set_ix))
        })?;

        let ix = self.buffers.len();

        self.buffers.push(buf);
        self.desc_sets.push(set);

        let mut name_key = Self::NAME_ID_PREFIX.to_vec();
        name_key.extend(name.as_bytes());

        let id_u8 = id.to_le_bytes();

        self.tree.insert(name_key, &id_u8)?;

        // self.tree
        //     .insert(Self::buf_ix_key(id), &buf.0.to_bits().as_le_bytes())?;

        todo!();
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn fmt_compatibility() {
        use zerocopy::{AsBytes, FromBytes};

        let z_u1 = 0u8;
        let z_u2 = 0u16;
        let z_u4 = 0u32;
        let z_u8 = 0u64;

        let z_f4 = 0f32;
        let z_f8 = 0f64;

        let z_u1_1 = [0u8; 1];
        let z_u1_2 = [0u8; 4];
        let z_u1_4 = [0u8; 4];

        let z_u4_1 = [0u32; 1];
        let z_u4_2 = [0u8; 4];
        let z_u4_4 = [0u32; 4];

        let z_u4_10 = [0u32; 10];
        let z_u4_12 = [0u32; 12];

        // let i_u32_4 = [0u32, 1u32, 2u32,

        let f_uint = BufFmt::UInt;
        let f_uvec2 = BufFmt::UVec2;
        let f_uvec4 = BufFmt::UVec4;

        let f_1u4 = BufFmt::UInt;
        let f_1i4 = BufFmt::SInt;
        let f_1f4 = BufFmt::Float;

        let f_4f4 = BufFmt::FVec4;
        let f_4u4 = BufFmt::UVec4;

        // let c_bytes =
        assert!(f_uint.is_compatible(z_u4_1.as_bytes()));

        assert!(f_uint.is_compatible(z_u4_4.as_bytes()));

        assert!(f_uvec2.is_compatible(z_u4_4.as_bytes()));

        assert!(f_uvec2.is_compatible(z_u4_10.as_bytes()));

        assert!(f_uvec2.is_compatible(z_u4_12.as_bytes()));

        assert!(!f_uvec4.is_compatible(z_u4_10.as_bytes()));

        assert!(f_uvec4.is_compatible(z_u4_12.as_bytes()));

        // casts

        assert!(f_uint.as_uint_ref(z_u4_10.as_bytes()).is_some());

        // assert!(f_1u4.is_compatible(z_u4_1.as_slice()));

        // assert_eq!(2 + 2, 4);
    }
}
