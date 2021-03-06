use ash::vk;
use raving::vk::{BufferIx, DescSetIx, GpuResources, VkEngine};

use rustc_hash::FxHashMap;
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

use parking_lot::{Mutex, RwLock};

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

#[derive(Debug, Clone)]
pub struct BufMeta<N: AsRef<[u8]>> {
    pub name: N,
    pub fmt: BufFmt,
    pub capacity: usize,
}

impl<N: AsRef<[u8]>> BufMeta<N> {
    pub fn insert_at(&self, store: &BufferStorage, id: BufId) -> Result<()> {
        // create the sled keys for the name, fmt, cap
        // then the contents
        // then insert
        let k_name = id.as_name_key();
        let k_fmt = id.as_fmt_key();
        let k_cap = id.as_cap_key();

        store.tree.transaction::<_, _, TransactionError>(|tree| {
            tree.insert(&k_name, self.name.as_ref())?;
            tree.insert(&k_fmt, self.fmt.to_bytes().as_slice())?;
            tree.insert(&k_cap, &self.capacity.to_le_bytes())?;
            Ok(())
        })?;

        Ok(())
    }
}

impl BufMeta<IVec> {
    pub fn get_stored(tree: &sled::Tree, id: BufId) -> Result<Self> {
        let k_name = id.as_name_key();
        let k_fmt = id.as_fmt_key();
        let k_cap = id.as_cap_key();

        let name = tree.get(&k_name)?.ok_or(anyhow!("buffer not found"))?;

        let fmt = tree
            .get(&k_fmt)?
            .and_then(|bs| BufFmt::from_bytes(bs.as_ref()))
            .ok_or(anyhow!("fmt not found"))?;

        let capacity = tree
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
    FVec2,

    FVec3,

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
            BufFmt::FVec2 => *b"2f4",
            BufFmt::FVec3 => *b"3f4",
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

            // uvec2
            b"2u4" => Some(Self::UVec2),
            // vec2
            b"2f4" => Some(Self::FVec2),

            // vec3
            b"3f4" => Some(Self::FVec3),

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
            BufFmt::FVec2 => 8,
            BufFmt::FVec3 => 12,
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

#[derive(Clone)]
pub struct BufferStorage {
    pub tree: sled::Tree,

    pub resource_indices: Arc<RwLock<FxHashMap<BufId, (BufferIx, DescSetIx)>>>,

    pub alloc_queue: Arc<Mutex<Vec<(BufId, String, BufFmt, usize)>>>,

    pub allocated_id: Arc<AtomicCell<u64>>,

    update_rx: crossbeam::channel::Receiver<BufId>,
    update_tx: crossbeam::channel::Sender<BufId>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, AsBytes, FromBytes, PartialOrd, Ord, Hash,
)]
#[repr(transparent)]
pub struct BufId(pub u64);

macro_rules! buf_id_key {
    ($fn_name:ident, $mask:literal, $len:literal) => {
        pub fn $fn_name(&self) -> [u8; $len] {
            let mut res = *$mask;
            self.write_to_suffix(&mut res[..]);
            res
        }
    };
}

impl BufId {
    // use scan_prefix to iterate through all names and IDs, i guess
    const NAME_ID_PREFIX: &'static [u8] = b"buffer_id:";

    buf_id_key!(as_name_key, b"n:01234567", 10);
    buf_id_key!(as_data_key, b"D:01234567", 10);
    buf_id_key!(as_fmt_key, b"f:01234567", 10);
    buf_id_key!(as_cap_key, b"c:01234567", 10);
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
    pub fn name_key(name: &str) -> Vec<u8> {
        let mut name_key = BufId::NAME_ID_PREFIX.to_vec();
        name_key.extend(name.as_bytes());
        name_key
    }

    pub fn new(db: &sled::Db) -> Result<Self> {
        let tree = db.open_tree("buffer_storage")?;

        let (update_tx, update_rx) = crossbeam::channel::unbounded();

        Ok(Self {
            tree,
            resource_indices: Default::default(),

            alloc_queue: Default::default(),
            allocated_id: Arc::new(0.into()),

            update_tx,
            update_rx,
        })
    }

    pub fn get_id(&self, name: &str) -> Option<BufId> {
        let name_key = BufferStorage::name_key(name);
        self.tree
            .get(&name_key)
            .ok()
            .flatten()
            .and_then(|id| BufId::read_from(id.as_ref()))
    }

    pub fn fill_buffer(&self, res: &mut GpuResources, id: BufId) -> Option<()> {
        let (buf_ix, _) = *self.resource_indices.read().get(&id)?;

        let buf = &mut res[buf_ix];

        let dst = buf.alloc.mapped_slice_mut()?;

        let meta = BufMeta::get_stored(&self.tree, id).ok()?;
        log::trace!("filling buffer {}", meta.name.as_bstr());

        match meta.fmt {
            BufFmt::UInt => self.fill_slice_from_data::<u32>(id, dst),
            BufFmt::SInt => self.fill_slice_from_data::<i32>(id, dst),
            BufFmt::Float => self.fill_slice_from_data::<f32>(id, dst),
            BufFmt::UVec2 => self.fill_slice_from_data::<[u32; 2]>(id, dst),
            BufFmt::FVec2 => self.fill_slice_from_data::<[f32; 2]>(id, dst),
            BufFmt::FVec3 => self.fill_slice_from_data::<[f32; 3]>(id, dst),
            BufFmt::UVec4 => self.fill_slice_from_data::<[u32; 4]>(id, dst),
            BufFmt::SVec4 => self.fill_slice_from_data::<[i32; 4]>(id, dst),
            BufFmt::FVec4 => self.fill_slice_from_data::<[f32; 4]>(id, dst),
        }?;

        Some(())
    }

    pub fn fill_buffer_impl<T: Copy + FromBytes>(
        &self,
        res: &mut GpuResources,
        id: BufId,
    ) -> Option<()> {
        let (buf_ix, _) = *self.resource_indices.read().get(&id)?;

        let buf = &mut res[buf_ix];

        let dst = buf.alloc.mapped_slice_mut()?;

        self.fill_slice_from_data::<T>(id, dst)?;

        Some(())
    }

    pub fn fill_slice_from_data<T: Copy + FromBytes>(
        &self,
        id: BufId,
        dst: &mut [u8],
    ) -> Option<()> {
        let meta = BufMeta::get_stored(&self.tree, id).ok()?;

        let elem_size = meta.fmt.size();
        let align_prefix = elem_size;

        let src = {
            let k_dat = id.as_data_key();
            let raw = self.tree.get(k_dat).ok()??;
            let src: &[T] = meta.fmt.as_slice(&raw)?;
            src
        };

        let len = src.len().min(meta.capacity);

        let dst_data = {
            let len = len as u32;

            log::trace!(
                "writing len {} for buffer {}",
                len,
                meta.name.as_bstr()
            );
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

    pub fn insert_data_from<T, F>(
        &self,
        id: BufId,
        len: usize,
        f: F,
    ) -> Result<()>
    where
        T: Copy + AsBytes,
        F: FnMut(usize) -> T,
    {
        let meta = BufMeta::get_stored(&self.tree, id)?;

        if meta.fmt.size() != std::mem::size_of::<T>() {
            bail!(
                "src type size {} doesn't match buffer metadata size {}",
                std::mem::size_of::<T>(),
                meta.fmt.size(),
            );
        }

        let len = len.min(meta.capacity);

        let src = (0..len).map(f).collect::<Vec<_>>();

        self.insert_data(id, &src)
    }

    pub fn insert_data<T: Copy + AsBytes>(
        &self,
        id: BufId,
        src: &[T],
    ) -> Result<()> {
        // 1. get the buffer metadata from sled
        log::trace!("src.len(): {}", src.len());
        let meta = BufMeta::get_stored(&self.tree, id)?;
        // log::warn!("src: {:?}", src);

        // dbg!(&meta);
        log::trace!(
            "meta.name {}\nmeta.capacity {}\nmeta.fmt {:?}\nmeta.fmt.size() {}",
            meta.name.as_bstr(),
            meta.capacity,
            meta.fmt,
            meta.fmt.size()
        );
        // 2. make sure the format matches T
        if meta.fmt.size() != std::mem::size_of::<T>() {
            bail!(
                "src type size {} doesn't match buffer metadata size {}",
                std::mem::size_of::<T>(),
                meta.fmt.size(),
            );
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
        let key = id.as_data_key();

        self.tree
            .update_and_fetch(key, |_| Some(value.as_slice()))?;

        self.update_tx.send(id)?;

        Ok(())
    }

    pub fn fill_updated_buffers(&self, res: &mut GpuResources) -> Result<()> {
        while let Ok(id) = self.update_rx.try_recv() {
            // is the problem here
            self.fill_buffer(res, id);
        }

        Ok(())
    }

    pub fn get_desc_set_ix(&self, id: BufId) -> Option<DescSetIx> {
        let (_, set_ix) = *self.resource_indices.read().get(&id)?;
        Some(set_ix)
    }

    pub fn get_buffer_ix(&self, id: BufId) -> Option<BufferIx> {
        let (buf_ix, _) = *self.resource_indices.read().get(&id)?;
        Some(buf_ix)
    }

    pub fn queue_allocate_buffer(
        &self,
        db: &sled::Db,
        name: &str,
        fmt: BufFmt,
        capacity: usize,
    ) -> Result<BufId> {
        let id = db.generate_id()?;
        let id = BufId(id);

        let params = (id, name.to_string(), fmt, capacity);

        self.initialize_buffer_metadata(id, name, fmt, capacity)?;
        self.alloc_queue.lock().push(params);

        Ok(id)
    }

    pub fn allocate_queued(&mut self, engine: &mut VkEngine) -> Result<usize> {
        let queue = {
            let mut old_queue = self.alloc_queue.lock();

            if old_queue.is_empty() {
                return Ok(0);
            }
            log::trace!("allocating {} queued buffers", old_queue.len());

            let mut queue = Vec::new();
            std::mem::swap(&mut queue, &mut old_queue);
            queue
        };

        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        let mut count = 0;
        let mut max_id = self.allocated_id.load();
        for (id, name, fmt, cap) in queue {
            self.allocate_buffer_impl(engine, id, &name, fmt, cap, usage)?;
            let id = id.0;
            max_id = id.max(max_id);
            count += 1;
        }
        self.allocated_id.store(max_id);

        Ok(count)
    }

    fn initialize_buffer_metadata(
        &self,
        id: BufId,
        name: &str,
        fmt: BufFmt,
        capacity: usize,
    ) -> Result<BufId> {
        //

        let id_u8 = id.0.to_le_bytes();

        // name -> id
        let name_key = Self::name_key(name);
        self.tree.insert(name_key, &id_u8)?;

        // metadata (id -> name, fmt, capacity)
        let k_name = id.as_name_key();
        self.tree.insert(k_name, name)?;

        self.tree.insert(id.as_fmt_key(), &fmt.to_bytes())?;
        self.tree.insert(id.as_cap_key(), &capacity.to_le_bytes())?;

        Ok(id)
    }

    pub fn allocate_buffer_impl(
        &mut self,
        engine: &mut VkEngine,
        id: BufId,
        name: &str,
        fmt: BufFmt,
        capacity: usize,
        usage: vk::BufferUsageFlags,
    ) -> Result<BufId> {
        let elem_size = fmt.size();

        let (buf, set) = engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;

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

        self.resource_indices.write().insert(id, (buf, set));

        Ok(id)
    }

    pub fn allocate_buffer(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        name: &str,
        fmt: BufFmt,
        capacity: usize,
    ) -> Result<BufId> {
        let id = db.generate_id()?;
        let id = BufId(id);

        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        self.initialize_buffer_metadata(id, name, fmt, capacity)?;
        self.allocate_buffer_impl(engine, id, name, fmt, capacity, usage)
    }

    pub fn allocate_buffer_with_usage(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        name: &str,
        fmt: BufFmt,
        capacity: usize,
        usage: vk::BufferUsageFlags,
    ) -> Result<BufId> {
        let id = db.generate_id()?;
        let id = BufId(id);

        self.initialize_buffer_metadata(id, name, fmt, capacity)?;
        self.allocate_buffer_impl(engine, id, name, fmt, capacity, usage)
    }
}
