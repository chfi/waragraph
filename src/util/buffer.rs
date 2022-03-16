use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use sled::{
    transaction::{TransactionError, TransactionResult},
    IVec,
};
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
}

impl<N: AsRef<[u8]>> BufMeta<N> {
    pub fn insert_at(&self, store: &BufferStorage, id: u64) -> Result<()> {
        // create the sled keys for the name, fmt, cap
        // then the contents
        // then insert
        let k_id = BufferStorage::id_key(id);
        let k_fmt = BufferStorage::fmt_key(id);
        let k_cap = BufferStorage::cap_key(id);

        store.tree.transaction::<_, _, TransactionError>(|tree| {
            tree.insert(&k_id, self.name.as_ref())?;
            tree.insert(&k_fmt, self.fmt.to_bytes().as_slice())?;
            tree.insert(&k_cap, &self.capacity.to_le_bytes())?;
            Ok(())
        })?;

        Ok(())
    }
}

impl BufMeta<IVec> {
    pub fn get_stored(store: &BufferStorage, id: u64) -> Result<Self> {
        let k_id = BufferStorage::id_key(id);
        let k_fmt = BufferStorage::fmt_key(id);
        let k_cap = BufferStorage::cap_key(id);

        let name = store.tree.get(&k_id)?.ok_or(anyhow!("buffer not found"))?;

        let fmt = store
            .tree
            .get(&k_fmt)?
            .and_then(|bs| BufFmt::from_bytes(bs.as_ref()))
            .ok_or(anyhow!("fmt not found"))?;

        let capacity = store
            .tree
            .get(&k_cap)?
            .and_then(|bs| usize::read_from(bs.as_ref()))
            .ok_or(anyhow!("capacity not found"))?;

        Ok(Self {
            name,
            fmt,
            capacity,
        })
    }
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
    pub const fn to_bytes(&self) -> [u8; 3] {
        match self {
            BufFmt::UInt => *b"1u4",
            BufFmt::SInt => *b"1i4",
            BufFmt::Float => *b"1f4",
            BufFmt::UVec2 => *b"2u4",
            BufFmt::UVec4 => *b"4u4",
            BufFmt::SVec4 => *b"4i4",
            BufFmt::FVec4 => *b"4f4",
        }
    }

    // pub fn from_bytes(fmt: [u8; 3]) -> Option<Self> {
    pub fn from_bytes(fmt: &[u8]) -> Option<Self> {
        match fmt {
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

    pub buffers: Vec<BufferIx>,
    pub desc_sets: Vec<DescSetIx>,
}

macro_rules! key_fn {
    // ($fn_name:ident, $init:expr, $offset:literal, $out_len:literal) => {
    ($fn_name:ident, $out:ty, $init:expr, $offset:literal) => {
        pub const fn $fn_name(id: u64) -> $out {
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
    const ID_NAME_MASK: [u8; 10] = *b"n:01234567";
    // use scan_prefix to iterate through all names and IDs, i guess
    const NAME_ID_PREFIX: &'static [u8] = b"buffer_id:";

    const BUF_DATA_MASK: [u8; 10] = *b"d:01234567";
    // used to store e.g. "[u8;2]"; basically a simple schema
    const BUF_FMT_MASK: [u8; 10] = *b"f:01234567";
    const BUF_CAP_MASK: [u8; 10] = *b"c:01234567";

    // const BUF_IX_MASK: [u8; 10] = *b"B:01234567";
    // const SET_IX_MASK: [u8; 10] = *b"S:01234567";

    const VEC_ID_MASK: [u8; 10] = *b"v:01234567";

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

    key_fn!(id_key, [u8; 10], Self::ID_NAME_MASK, 2);
    key_fn!(data_key, [u8; 10], Self::BUF_DATA_MASK, 2);
    key_fn!(fmt_key, [u8; 10], Self::BUF_FMT_MASK, 2);
    key_fn!(cap_key, [u8; 10], Self::BUF_CAP_MASK, 2);

    key_fn!(vec_id_key, [u8; 10], Self::VEC_ID_MASK, 2);

    // key_fn!(buf_ix_key, [u8; 10], Self::BUF_IX_MASK, 2);
    // key_fn!(set_ix_key, [u8; 10], Self::SET_IX_MASK, 2);
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

    /*
    pub fn allocate_buffer_and_fill(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        name: &str,
        fmt: BufFmt,
        capacity: usize,

    ) -> Result<u64> {
    }
    */

    pub fn fill_slice_from<T: Copy + FromBytes>(
        fmt: BufFmt,
        capacity: usize,
        src: &[T],
        dst: &mut [u8],
    ) -> Option<()> {
        let elem_size = fmt.size();
        let align_prefix = elem_size;

        let len = src.len().min(capacity);

        let dst_data = {
            let len = len as u32;
            let (prefix, data) = dst.split_at_mut(elem_size);

            let prefix_n = align_prefix / std::mem::size_of::<u32>();
            log::warn!("prefix_n: {}", prefix_n);
            for i in 0..prefix_n {
                let s = i * 4;
                let e = s + 4;
                prefix[s..e].clone_from_slice(&len.to_le_bytes());
            }

            let slice: &mut [T] = fmt.as_slice_mut(data)?;
            slice
        };

        for (s, d) in std::iter::zip(src, dst_data) {
            *d = *s;
        }

        Some(())
    }

    pub fn fill_buffer<T: Copy + FromBytes>(
        &self,
        res: &mut GpuResources,
        id: u64,
    ) -> Option<()> {
        // let buf_key = Self::buf_ix_key(id);
        let k_vec = Self::vec_id_key(id);
        let vec_ix = self.tree.get(k_vec).ok()??;
        let vec_ix = usize::read_from(vec_ix.as_ref())?;

        let buf_ix = self.buffers[vec_ix];

        let buf = &mut res[buf_ix];

        let dst = buf.alloc.mapped_slice_mut()?;

        self.fill_slice_from_data::<T>(id, dst)?;

        Some(())
    }

    pub fn fill_slice_from_data<T: Copy + FromBytes>(
        &self,
        id: u64,
        dst: &mut [u8],
    ) -> Option<()> {
        let meta = BufMeta::get_stored(self, id).ok()?;

        let elem_size = meta.fmt.size();
        let align_prefix = elem_size;

        let src = {
            let k_dat = Self::data_key(id);
            let raw = self.tree.get(k_dat).ok()??;
            let src: &[T] = meta.fmt.as_slice(&raw)?;
            src
        };

        let len = src.len().min(meta.capacity);

        let dst_data = {
            let (prefix, data) = dst.split_at_mut(elem_size);

            for i in 0..(align_prefix / std::mem::size_of::<u32>()) {
                let s = i * 4;
                let e = s + 4;
                prefix[s..e].clone_from_slice(&len.to_le_bytes());
            }

            let slice: &mut [T] = meta.fmt.as_slice_mut(data)?;
            slice
        };

        for (s, d) in std::iter::zip(src, dst_data) {
            *d = *s;
        }

        Some(())
    }

    pub fn insert_data<T: Copy + AsBytes>(
        &self,
        id: u64,
        src: &[T],
    ) -> Result<()> {
        // 1. get the buffer metadata from sled
        let meta = BufMeta::get_stored(self, id)?;

        // 2. make sure the format matches T
        if meta.fmt.size() != std::mem::size_of::<T>() {
            bail!("src type size doesn't match buffer metadata");
        }

        // 3. limit the length of src based on capacity, if needed
        // 4. cast src to a bytestring
        let value = src
            .iter()
            .take(meta.capacity)
            .flat_map(|s| s.as_bytes())
            .copied()
            .collect::<Vec<_>>();

        // 5. insert bytestring at the data key
        let key = Self::data_key(id);
        self.tree.insert(key, value)?;

        Ok(())
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

        if let Some(slice) = engine.resources[buf].mapped_slice_mut() {
            slice.fill(0);
        }

        let ix = self.buffers.len();

        let k_vec = Self::vec_id_key(id);

        self.tree.insert(k_vec, &ix.to_le_bytes())?;

        self.buffers.push(buf);
        self.desc_sets.push(set);

        let mut name_key = Self::NAME_ID_PREFIX.to_vec();
        name_key.extend(name.as_bytes());

        let id_u8 = id.to_le_bytes();

        // "buffer_id:{name}" -> id
        self.tree.insert(name_key, &id_u8)?;
        // id -> name
        let k_id = Self::id_key(id);
        self.tree.insert(k_id, name.as_bytes())?;

        // id -> fmt, id -> cap
        let k_fmt = Self::fmt_key(id);
        let k_cap = Self::cap_key(id);

        self.tree.insert(k_fmt, &fmt.to_bytes())?;
        self.tree.insert(k_cap, &capacity.to_le_bytes())?;

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use rand::prelude::*;

    use super::*;

    #[test]
    fn fill_with_fmt() {
        let mut rng = rand::thread_rng();

        let fmt = BufFmt::UInt;
        let capacity = 64;

        let mut buf = vec![0u8; capacity * fmt.size()];

        let len = 40;

        let mut src_uints: Vec<u32> = (0u32..len).collect();
        src_uints.shuffle(&mut rng);

        let _print_as_u32s = |s: &str, bytes: &[u8]| {
            if let Some(slice) = fmt.as_slice::<u32>(bytes) {
                let len = slice[0];
                eprintln!("{} len {} - {:?}", s, len, &slice[1..]);
            }
        };

        BufferStorage::fill_slice_from::<u32>(fmt, 40, &src_uints, &mut buf);

        let (dst_len, dst_data) = {
            let slice = fmt.as_slice::<u32>(&buf).unwrap();
            let len = slice[0] as usize;
            (len, &slice[1..len + 1])
        };

        assert_eq!(dst_len, src_uints.len());
        assert_eq!(dst_data, &src_uints);
    }

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
