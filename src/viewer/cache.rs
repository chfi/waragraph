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

use sled::IVec;
use zerocopy::{AsBytes, FromBytes};

use anyhow::{anyhow, Result};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheError {
    OutOfRows,
    ElemSizeMismatch,
    RowSizeMismatch { actual: usize, expected: usize },
    BufferSizeMismatch { actual: usize, expected: usize },
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::OutOfRows => {
                write!(f, "Buffer cache allocation error: Out of rows, need reallocation")
            }
            CacheError::RowSizeMismatch { actual, expected } => {
                write!(f, "Buffer cache update error: Data consisted of {} elements, but row expected {}", actual, expected)
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
    row_map: HashMap<K, usize>,

    elem_size: usize,
    row_size: usize,

    row_capacity: usize,
    used_row_count: usize,
    used_rows: Vec<bool>,
    // used_rows: roaring::RoaringBitmap,
}

impl<K: std::hash::Hash + Eq> BufferCache<K> {
    pub fn new(
        // engine: &mut VkEngine,
        elem_size: usize,
        row_size: usize,
        row_capacity: usize,
    ) -> Self {
        Self {
            row_map: HashMap::default(),

            elem_size,
            row_size,

            row_capacity,
            used_row_count: 0,
            used_rows: vec![false; row_capacity],
        }
    }

    pub fn used_rows(&self) -> usize {
        self.used_row_count
    }

    pub fn is_empty(&self) -> bool {
        self.used_row_count == 0
    }

    pub fn is_full(&self) -> bool {
        self.used_row_count >= self.row_capacity
    }

    /// Returns the size of the total cache, in bytes
    pub fn buffer_size(&self) -> usize {
        self.row_capacity * self.row_size * self.elem_size
    }

    pub fn clear(&mut self) {
        self.row_map.clear();
        self.used_rows.iter_mut().for_each(|v| *v = false);
        self.used_row_count = 0;
    }

    pub fn reallocate(&mut self, new_row_count: usize, new_width: usize) {
        self.clear();
        self.used_rows.resize(new_row_count, false);
        self.row_size = new_width;
    }

    pub fn reallocate_rows(&mut self, row_count: usize) {
        self.clear();
        self.used_rows.resize(row_count, false);
    }

    pub fn resize_rows(&mut self, new_width: usize) {
        self.clear();
        self.row_size = new_width;
    }

    pub fn is_bound<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.row_map.contains_key(k)
    }

    fn range_for_ix(&self, ix: usize) -> std::ops::Range<usize> {
        let row_size_b = self.elem_size * self.row_size;

        let start = ix * row_size_b;
        let end = start + row_size_b;

        start..end
    }

    pub fn get_range<Q: ?Sized>(&self, k: &Q) -> Option<std::ops::Range<usize>>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.row_map.get(k).map(|i| self.range_for_ix(*i))
    }

    pub fn rebind_rows(
        &mut self,
        new_keys: impl IntoIterator<Item = K>,
    ) -> std::result::Result<(), CacheError>
    where
        K: Clone + std::fmt::Debug,
    {
        let new_keys = new_keys.into_iter().collect::<HashSet<_>>();
        let old_keys = self.row_map.keys().cloned().collect::<HashSet<_>>();

        let to_remove = old_keys.difference(&new_keys);

        for key in to_remove {
            eprintln!("unbinding row {:?}", key);
            self.unbind_row(&key);
        }

        eprintln!("in rebind rows, after unbinds");
        eprintln!("#{:#?}", self);

        for key in new_keys {
            eprintln!("binding row");
            let _ = self.bind_row(key)?;
        }

        Ok(())
    }

    pub fn bind_row(
        &mut self,
        k: K,
    ) -> std::result::Result<std::ops::Range<usize>, CacheError> {
        if let Some(range) = self.get_range(&k) {
            return Ok(range);
        }

        let (row_ix, _) = self
            .used_rows
            .iter()
            .enumerate()
            .find(|(_, &v)| !v)
            .ok_or(CacheError::OutOfRows)?;

        self.used_rows[row_ix] = true;

        self.row_map.insert(k, row_ix);
        self.used_row_count += 1;

        Ok(self.range_for_ix(row_ix))
    }

    pub fn unbind_row<Q: ?Sized>(&mut self, k: &Q) -> Option<()>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        let row_ix = self.row_map.remove(k)?;
        debug_assert!(
            self.used_rows[row_ix],
            "Buffer cache: Row map entry existed but row was not in use"
        );
        self.used_rows[row_ix] = false;
        self.used_row_count -= 1;
        Some(())
    }

    // pub fn reallocate

    // fn pick_row<Q: ?Sized>(

    pub fn write_row<Q: ?Sized>(
        &mut self,
        buffer: &mut [u8],
        // buffer: &mut BufferRes,
        // res: &mut GpuResources,
        row: &Q,
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

        if elem_count != self.row_size {
            return Err(CacheError::RowSizeMismatch {
                actual: elem_count,
                expected: self.row_size,
            }
            .into());
        }

        let range = self
            .get_range(row)
            .ok_or(anyhow!("Buffer cache error: Unbound key {:?}", row))?;

        buffer[range].clone_from_slice(data);

        Ok(())
    }
}

/// TODO: only GpuToCpu memory locations currently
pub struct GpuBufferCache<K>
where
    K: std::hash::Hash + Eq,
{
    name: String,

    usage: vk::BufferUsageFlags,
    buffer: BufferIx,
    pub desc_set: DescSetIx,

    cache: BufferCache<K>,
}

impl<K: std::hash::Hash + Eq> GpuBufferCache<K> {
    pub fn new(
        engine: &mut VkEngine,
        usage: vk::BufferUsageFlags,
        name: &str,
        elem_size: usize,
        row_size: usize,
        row_capacity: usize,
    ) -> Result<Self> {
        let cache = BufferCache::new(elem_size, row_size, row_capacity);

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

        Ok(Self {
            name: name.to_string(),

            usage,
            buffer,
            desc_set,

            cache,
        })
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

    pub fn bind_rows(
        &mut self,
        new_keys: impl IntoIterator<Item = K>,
    ) -> std::result::Result<(), CacheError>
    where
        K: Clone + std::fmt::Debug,
    {
        self.cache.rebind_rows(new_keys)
    }

    pub fn reallocate(
        &mut self,
        engine: &mut VkEngine,
        new_row_count: usize,
        new_row_width: usize,
    ) -> anyhow::Result<()> {
        self.cache.reallocate(new_row_count, new_row_width);
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
        let row_size = 32;
        let row_capacity = 4;
        let mut cache: BufferCache<(rhai::ImmutableString, usize)> =
            BufferCache::new(elem_size, row_size, row_capacity);

        // let mut buffer: Vec<u8> = vec![0u8; cache.buffer_size()];

        assert!(cache.is_empty());

        let n = 4;

        let k_as = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        let k_bs = (0..n)
            .map(|i| (rhai::ImmutableString::from("B"), i))
            .collect::<Vec<_>>();

        let r0 = cache.bind_row(k_as[0].clone())?;

        assert!(cache.used_rows() == 1);
        assert!(cache.used_rows[0]);
        assert!(!cache.used_rows[1]);
        assert!(!cache.is_empty());
        assert!(!cache.is_full());

        let _r1 = cache.bind_row(k_as[1].clone())?;
        let _r2 = cache.bind_row(k_as[2].clone())?;
        let _r3 = cache.bind_row(k_as[3].clone())?;

        assert!(cache.is_full());

        assert!(cache.bind_row(k_bs[0].clone()).is_err());

        cache.unbind_row(&k_as[0]);

        let r4 = cache.bind_row(k_bs[0].clone())?;

        assert_eq!(r0, r4);

        eprintln!("{:#?}", cache);

        Ok(())
    }

    // basically tests the "list scrolling" support provided by the
    // rebind_rows() method
    #[test]
    fn test_bind_iter() -> anyhow::Result<()> {
        let elem_size = std::mem::size_of::<u32>();
        let row_size: usize = 8;
        let row_capacity = 4;
        let mut cache: BufferCache<(rhai::ImmutableString, usize)> =
            BufferCache::new(elem_size, row_size, row_capacity);

        assert!(cache.is_empty());

        let n = 4;

        let mut keys = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        keys.extend((0..n).map(|i| (rhai::ImmutableString::from("B"), i)));

        cache.rebind_rows(keys[0..4].iter().cloned())?;

        assert!(cache.is_full());

        let r0 = cache.get_range(&keys[0]).unwrap();

        cache.rebind_rows(keys[1..5].iter().cloned())?;

        let r4 = cache.get_range(&keys[4]).unwrap();

        assert_eq!(r0, r4);

        cache.rebind_rows(keys[1..5].iter().cloned())?;
        let r4 = cache.get_range(&keys[4]).unwrap();

        assert_eq!(r0, r4);

        cache.rebind_rows(keys[4..8].iter().cloned())?;

        let r4 = cache.get_range(&keys[4]).unwrap();

        assert_eq!(r0, r4);

        Ok(())
    }

    #[test]
    fn test_buffer_write() -> anyhow::Result<()> {
        let elem_size = std::mem::size_of::<u32>();
        let row_size: usize = 8;
        let row_capacity = 4;
        let mut cache: BufferCache<(rhai::ImmutableString, usize)> =
            BufferCache::new(elem_size, row_size, row_capacity);

        let mut buffer: Vec<u8> = vec![0u8; cache.buffer_size()];

        let n = 4;

        let mut keys = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        keys.extend((0..n).map(|i| (rhai::ImmutableString::from("B"), i)));

        cache.rebind_rows(keys[0..4].iter().cloned())?;

        let data: Vec<Vec<u32>> = (0..(2 * n as u32))
            .map(|i| {
                let v = i | (i << 8) | (i << 16) | (i << 24);
                vec![v; row_size]
            })
            .collect::<Vec<_>>();

        // only keys 0-3 inclusive have been bound so this must fail
        assert!(cache
            .write_row(
                &mut buffer,
                &keys[4],
                bytemuck::cast_slice(data[4].as_slice())
            )
            .is_err());

        for (key, data) in keys.iter().zip(data.iter()).take(4) {
            eprintln!("{:?} -> {:?}", key, cache.get_range(key));
            cache.write_row(&mut buffer, key, bytemuck::cast_slice(data))?;
        }

        eprintln!("{:#?}", cache);
        eprintln!("{:x?}", buffer);

        cache.rebind_rows(keys[1..5].iter().cloned())?;

        for (key, data) in keys.iter().zip(data.iter()).skip(1).take(4) {
            eprintln!("{:?} -> {:?}", key, cache.get_range(key));
            cache.write_row(&mut buffer, key, bytemuck::cast_slice(data))?;
        }

        eprintln!("{:#?}", cache);

        Ok(())
    }
}
