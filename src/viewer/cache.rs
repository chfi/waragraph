use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashMap, HashSet},
};

use ash::vk;
use bimap::BiHashMap;
use gpu_allocator::vulkan::Allocator;
use parking_lot::RwLock;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use rustc_hash::FxHashMap;
use sled::IVec;
use zerocopy::{AsBytes, FromBytes};

use anyhow::{anyhow, Result};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheError {
    OutOfBlocks,
    ElemSizeMismatch,
    BlockSizeMismatch { actual: usize, expected: usize },
    BufferSizeMismatch { actual: usize, expected: usize },
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::OutOfBlocks => {
                write!(f, "Buffer cache allocation error: Out of blocks, need reallocation")
            }
            CacheError::BlockSizeMismatch { actual, expected } => {
                write!(f, "Buffer cache update error: Data consisted of {} elements, but block expected {}", actual, expected)
            }
            CacheError::ElemSizeMismatch => {
                write!(f, "Buffer cache update error: Data bytestring not evenly divisible with element size")
            }
            CacheError::BufferSizeMismatch { actual, expected } => {
                write!(f, "Buffer cache update error: Provided buffer is {} bytes, expected {}", actual, expected)
            }
        }
    }
}

impl std::error::Error for CacheError {}

#[derive(Debug, Clone)]
pub struct BufferCache<K>
where
    K: std::hash::Hash + Eq,
{
    // usage: vk::BufferUsageFlags,
    // buffer: BufferIx,
    // desc_set: DescSetIx,
    block_map: HashMap<K, usize>,

    elem_size: usize,
    block_size: usize,

    block_capacity: usize,
    used_block_count: usize,
    used_blocks: Vec<bool>,
    // used_rows: roaring::RoaringBitmap,
}

impl<K: std::hash::Hash + Eq> BufferCache<K> {
    pub fn new(
        // engine: &mut VkEngine,
        elem_size: usize,
        block_size: usize,
        block_capacity: usize,
    ) -> Self {
        Self {
            block_map: HashMap::default(),

            elem_size,
            block_size,

            block_capacity,
            used_block_count: 0,
            used_blocks: vec![false; block_capacity],
        }
    }

    pub fn block_capacity(&self) -> usize {
        self.block_capacity
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn used_blocks(&self) -> usize {
        self.used_block_count
    }

    pub fn is_empty(&self) -> bool {
        self.used_block_count == 0
    }

    pub fn is_full(&self) -> bool {
        self.used_block_count >= self.block_capacity
    }

    /// Returns the size of the total cache, in bytes
    pub fn buffer_size(&self) -> usize {
        self.block_capacity * self.block_size * self.elem_size
    }

    pub fn clear(&mut self) {
        self.block_map.clear();
        self.used_blocks.iter_mut().for_each(|v| *v = false);
        self.used_block_count = 0;
    }

    pub fn reallocate(&mut self, new_block_count: usize, new_width: usize) {
        self.clear();
        self.used_blocks.resize(new_block_count, false);
        self.block_size = new_width;
    }

    pub fn reallocate_blocks(&mut self, block_count: usize) {
        self.clear();
        self.used_blocks.resize(block_count, false);
    }

    pub fn resize_blocks(&mut self, new_width: usize) {
        self.clear();
        self.block_size = new_width;
    }

    pub fn is_bound<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.block_map.contains_key(k)
    }

    fn range_for_ix(&self, ix: usize) -> std::ops::Range<usize> {
        let block_size_b = self.elem_size * self.block_size;

        let start = ix * block_size_b;
        let end = start + block_size_b;

        start..end
    }

    pub fn get_range<Q: ?Sized>(&self, k: &Q) -> Option<std::ops::Range<usize>>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.block_map.get(k).map(|i| self.range_for_ix(*i))
    }

    pub fn rebind_blocks(
        &mut self,
        new_keys: impl IntoIterator<Item = K>,
    ) -> std::result::Result<Vec<K>, CacheError>
    where
        K: Clone + std::fmt::Debug,
    {
        let new_keys = new_keys.into_iter().collect::<HashSet<_>>();
        let old_keys = self.block_map.keys().cloned().collect::<HashSet<_>>();

        let to_remove = old_keys.difference(&new_keys);

        for key in to_remove {
            self.unbind_block(&key);
        }

        let mut newly_inserted = Vec::new();
        for key in new_keys {
            if self.bind_block(key.clone())? {
                newly_inserted.push(key);
            }
        }

        Ok(newly_inserted)
    }

    /// returns `Ok(false)` if the key was already bound, `Ok(true)`
    /// if the key was freshly bound (and thus the backing buffer
    /// needs to be updated)
    pub fn bind_block(
        &mut self,
        k: K,
    ) -> std::result::Result<bool, CacheError> {
        if let Some(range) = self.get_range(&k) {
            return Ok(false);
        }

        let (block_ix, _) = self
            .used_blocks
            .iter()
            .enumerate()
            .find(|(_, &v)| !v)
            .ok_or(CacheError::OutOfBlocks)?;

        self.used_blocks[block_ix] = true;

        self.block_map.insert(k, block_ix);
        self.used_block_count += 1;

        Ok(true)
    }

    pub fn unbind_block<Q: ?Sized>(&mut self, k: &Q) -> Option<()>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        let block_ix = self.block_map.remove(k)?;
        debug_assert!(
            self.used_blocks[block_ix],
            "Buffer cache: Block map entry existed but block was not in use"
        );
        self.used_blocks[block_ix] = false;
        self.used_block_count -= 1;
        Some(())
    }

    // pub fn reallocate

    // fn pick_row<Q: ?Sized>(

    pub fn write_block<Q: ?Sized>(
        &mut self,
        buffer: &mut [u8],
        // buffer: &mut BufferRes,
        // res: &mut GpuResources,
        block: &Q,
        data: &[u8],
        // ) -> std::result::Result<(), CacheError>
    ) -> Result<()>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq + std::fmt::Debug,
    {
        let elem_count = data.len() / self.elem_size;

        let expected_size = self.buffer_size();

        if buffer.len() != self.buffer_size() {
            return Err(CacheError::BufferSizeMismatch {
                actual: buffer.len(),
                expected: expected_size,
            }
            .into());
        }

        if data.len() % self.elem_size != 0 {
            return Err(CacheError::ElemSizeMismatch.into());
        }

        if elem_count != self.block_size {
            return Err(CacheError::BlockSizeMismatch {
                actual: elem_count,
                expected: self.block_size,
            }
            .into());
        }

        let range = self
            .get_range(block)
            .ok_or(anyhow!("Buffer cache error: Unbound key {:?}", block))?;

        buffer[range].clone_from_slice(data);

        Ok(())
    }
}

// pub struct UpdateReqMsg<K, T>
pub struct UpdateReqMsg<K>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
    // T: std::hash::Hash + Eq + Send + Sync + 'static,
    // T: Eq + Send + Sync + 'static,
{
    key: K,
    // payload: T,
    create_payload: Box<
        dyn FnOnce(K) -> anyhow::Result<DataMsg<K>> + Send + Sync + 'static,
    >,
}

impl<K> UpdateReqMsg<K>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
{
    pub fn new<F, G>(key: K, f: F, signal: G) -> Self
    where
        F: FnOnce(&K) -> anyhow::Result<Vec<u8>> + Send + Sync + 'static,
        G: FnOnce() + Send + Sync + 'static,
    {
        let create_payload = Box::new(|key| {
            let data = f(&key)?;
            Ok(DataMsg {
                key,
                data,
                and_then: Some(Box::new(signal)),
            })
        });

        Self {
            key,
            create_payload,
        }
    }
}

pub struct DataMsg<K>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
{
    key: K,
    data: Vec<u8>,
    and_then: Option<Box<dyn FnOnce() + Send + Sync + 'static>>,
}

// #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub enum BlockState {
//     Unknown,
//     UpToDate,
// }

/// TODO: only GpuToCpu memory locations currently
pub struct GpuBufferCache<K>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
{
    name: String,

    usage: vk::BufferUsageFlags,
    buffer: BufferIx,
    pub desc_set: DescSetIx,

    cache: BufferCache<K>,

    // block_state_map: FxHashMap<u64, Arc<AtomicCell<BlockState>>>,
    // block_state_map: FxHashMap<K, Arc<AtomicCell<BlockState>>>,
    pub update_request_tx: crossbeam::channel::Sender<UpdateReqMsg<K>>,
    update_request_rx: crossbeam::channel::Receiver<UpdateReqMsg<K>>,

    pub data_msg_tx: crossbeam::channel::Sender<DataMsg<K>>,
    data_msg_rx: crossbeam::channel::Receiver<DataMsg<K>>,
}

impl<K> GpuBufferCache<K>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
{
    // returns a closure that can be used in a loop by a worker thread
    // to consume the update requests
    //
    // the closure blocks until an update request is received
    pub fn data_msg_worker(
        &self,
    ) -> Box<dyn Fn() -> anyhow::Result<()> + Send + Sync + 'static> {
        let in_rx = self.update_request_rx.clone();
        let out_tx = self.data_msg_tx.clone();

        Box::new(move || {
            let msg = in_rx.recv()?;
            let data = (msg.create_payload)(msg.key)?;
            out_tx.send(data)?;
            Ok(())
        })
    }

    pub fn new(
        engine: &mut VkEngine,
        usage: vk::BufferUsageFlags,
        name: &str,
        elem_size: usize,
        block_size: usize,
        block_capacity: usize,
    ) -> Result<Self> {
        let cache = BufferCache::new(elem_size, block_size, block_capacity);

        let capacity = cache.buffer_size();

        let (buffer, desc_set) =
            engine.with_allocators(|ctx, res, alloc| {
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

                let desc_set =
                    crate::util::allocate_buffer_desc_set(buf_ix, res)?;

                let set_ix = res.insert_desc_set(desc_set);

                Ok((buf_ix, set_ix))
            })?;

        let (update_request_tx, update_request_rx) =
            crossbeam::channel::unbounded();

        let (data_msg_tx, data_msg_rx) = crossbeam::channel::unbounded();

        Ok(Self {
            name: name.to_string(),

            usage,
            buffer,
            desc_set,

            cache,

            data_msg_tx,
            data_msg_rx,

            update_request_tx,
            update_request_rx,
        })
    }

    pub fn apply_data_updates(
        &mut self,
        res: &mut GpuResources,
    ) -> anyhow::Result<()>
    where
        K: std::fmt::Debug,
    {
        let buffer = &mut res[self.buffer];

        let slice = buffer
            .mapped_slice_mut()
            .expect("GPU cache buffer must be host-accessible");

        while let Ok(msg) = self.data_msg_rx.try_recv() {
            let range = self
                .cache
                .get_range(&msg.key)
                .ok_or(anyhow!("GPU cache error: unbound key {:?}", msg.key))?;

            if range.len() == msg.data.len() {
                slice[range].clone_from_slice(&msg.data);
                if let Some(signal) = msg.and_then {
                    signal();
                }
            } else {
                log::debug!("received data of incorrect width, ignoring");
            }
        }

        Ok(())
    }

    pub fn cache(&self) -> &BufferCache<K> {
        &self.cache
    }

    pub fn buffer(&self) -> BufferIx {
        self.buffer
    }

    pub fn desc_set(&self) -> DescSetIx {
        self.desc_set
    }

    pub fn bind_blocks(
        &mut self,
        new_keys: impl IntoIterator<Item = K>,
    ) -> std::result::Result<(), CacheError>
    where
        K: Clone + std::fmt::Debug,
    {
        let new_keys = self.cache.rebind_blocks(new_keys)?;
        // for key in new_keys {
        //     self.block_state_map
        //         .insert(key, Arc::new(BlockState::Unknown.into()));
        // }
        Ok(())
    }

    /// keys that haven't been bound are ignored
    pub fn block_ranges<'a>(
        &'a self,
        keys: impl IntoIterator<Item = K> + 'a,
    ) -> impl Iterator<Item = (K, std::ops::Range<usize>)> + 'a {
        keys.into_iter().filter_map(|k| {
            let range = self.cache.get_range(&k)?;
            Some((k, range))
        })
    }

    pub fn reallocate(
        &mut self,
        engine: &mut VkEngine,
        new_block_count: usize,
        new_block_size: usize,
    ) -> anyhow::Result<()> {
        self.cache.reallocate(new_block_count, new_block_size);
        let capacity = self.cache.buffer_size();

        engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;

            let buffer = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                self.cache.elem_size,
                capacity,
                self.usage,
                Some(&self.name),
            )?;

            res.insert_buffer_at(self.buffer, buffer)
                .into_iter()
                .try_for_each(|buf| res.free_buffer(ctx, alloc, buf))?;

            let desc_set =
                crate::util::allocate_buffer_desc_set(self.buffer, res)?;
            let _ = res.insert_desc_set_at(self.desc_set, desc_set);

            Ok(())
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_bind_unbind() -> anyhow::Result<()> {
        let elem_size = std::mem::size_of::<u32>();
        let block_size = 32;
        let block_capacity = 4;
        let mut cache: BufferCache<(rhai::ImmutableString, usize)> =
            BufferCache::new(elem_size, block_size, block_capacity);

        // let mut buffer: Vec<u8> = vec![0u8; cache.buffer_size()];

        assert!(cache.is_empty());

        let n = 4;

        let k_as = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        let k_bs = (0..n)
            .map(|i| (rhai::ImmutableString::from("B"), i))
            .collect::<Vec<_>>();

        let r0 = cache.bind_block(k_as[0].clone())?;

        assert!(cache.used_blocks() == 1);
        assert!(cache.used_blocks[0]);
        assert!(!cache.used_blocks[1]);
        assert!(!cache.is_empty());
        assert!(!cache.is_full());

        let _r1 = cache.bind_block(k_as[1].clone())?;
        let _r2 = cache.bind_block(k_as[2].clone())?;
        let _r3 = cache.bind_block(k_as[3].clone())?;

        assert!(cache.is_full());

        assert!(cache.bind_block(k_bs[0].clone()).is_err());

        cache.unbind_block(&k_as[0]);

        let r4 = cache.bind_block(k_bs[0].clone())?;

        assert_eq!(r0, r4);

        eprintln!("{:#?}", cache);

        Ok(())
    }

    // basically tests the "list scrolling" support provided by the
    // rebind_blocks() method
    #[test]
    fn test_bind_iter() -> anyhow::Result<()> {
        let elem_size = std::mem::size_of::<u32>();
        let block_size: usize = 8;
        let block_capacity = 4;
        let mut cache: BufferCache<(rhai::ImmutableString, usize)> =
            BufferCache::new(elem_size, block_size, block_capacity);

        assert!(cache.is_empty());

        let n = 4;

        let mut keys = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        keys.extend((0..n).map(|i| (rhai::ImmutableString::from("B"), i)));

        cache.rebind_blocks(keys[0..4].iter().cloned())?;

        assert!(cache.is_full());

        let r0 = cache.get_range(&keys[0]).unwrap();

        cache.rebind_blocks(keys[1..5].iter().cloned())?;

        let r4 = cache.get_range(&keys[4]).unwrap();

        assert_eq!(r0, r4);

        cache.rebind_blocks(keys[1..5].iter().cloned())?;
        let r4 = cache.get_range(&keys[4]).unwrap();

        assert_eq!(r0, r4);

        cache.rebind_blocks(keys[4..8].iter().cloned())?;

        let r4 = cache.get_range(&keys[4]).unwrap();

        assert_eq!(r0, r4);

        Ok(())
    }

    #[test]
    fn test_buffer_write() -> anyhow::Result<()> {
        let elem_size = std::mem::size_of::<u32>();
        let block_size: usize = 8;
        let block_capacity = 4;
        let mut cache: BufferCache<(rhai::ImmutableString, usize)> =
            BufferCache::new(elem_size, block_size, block_capacity);

        let mut buffer: Vec<u8> = vec![0u8; cache.buffer_size()];

        let n = 4;

        let mut keys = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        keys.extend((0..n).map(|i| (rhai::ImmutableString::from("B"), i)));

        cache.rebind_blocks(keys[0..4].iter().cloned())?;

        let data: Vec<Vec<u32>> = (0..(2 * n as u32))
            .map(|i| {
                let v = i | (i << 8) | (i << 16) | (i << 24);
                vec![v; block_size]
            })
            .collect::<Vec<_>>();

        // only keys 0-3 inclusive have been bound so this must fail
        assert!(cache
            .write_block(
                &mut buffer,
                &keys[4],
                bytemuck::cast_slice(data[4].as_slice())
            )
            .is_err());

        for (key, data) in keys.iter().zip(data.iter()).take(4) {
            eprintln!("{:?} -> {:?}", key, cache.get_range(key));
            cache.write_block(&mut buffer, key, bytemuck::cast_slice(data))?;
        }

        eprintln!("{:#?}", cache);
        eprintln!("{:x?}", buffer);

        cache.rebind_blocks(keys[1..5].iter().cloned())?;

        for (key, data) in keys.iter().zip(data.iter()).skip(1).take(4) {
            eprintln!("{:?} -> {:?}", key, cache.get_range(key));
            cache.write_block(&mut buffer, key, bytemuck::cast_slice(data))?;
        }

        eprintln!("{:#?}", cache);

        Ok(())
    }
}
