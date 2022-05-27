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

// impl<K: std::hash::Hash + Eq + std::fmt::Debug> std::fmt::Debug
//     for BufferCache<K>
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         Ok(())
//     }
// }

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
        K: Clone,
    {
        let new_keys = new_keys.into_iter().collect::<HashSet<_>>();
        let old_keys = self.row_map.keys().cloned().collect::<HashSet<_>>();

        let to_remove = new_keys.difference(&old_keys);

        for key in to_remove {
            self.unbind_row(&key);
        }

        for key in new_keys {
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
    desc_set: DescSetIx,

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

        let mut buffer: Vec<u8> = vec![0u8; cache.buffer_size()];

        assert!(cache.is_empty());

        let k0 = (rhai::ImmutableString::from("A"), 0usize);
        let k1 = (rhai::ImmutableString::from("A"), 1usize);
        let k2 = (rhai::ImmutableString::from("B"), 0usize);
        let k3 = (rhai::ImmutableString::from("B"), 1usize);

        let r0 = cache.bind_row(k0)?;

        assert!(cache.used_rows() == 0);
        assert!(!cache.is_empty());
        assert!(!cache.is_full());

        eprintln!("{:?}", cache);

        /*
        let n = 8;

        let k_as = (0..n)
            .map(|i| (rhai::ImmutableString::from("A"), i))
            .collect::<Vec<_>>();
        let k_bs = (0..n)
            .map(|i| (rhai::ImmutableString::from("B"), i))
            .collect::<Vec<_>>();

        use rand::prelude::*;

        let mut rng = rand::thread_rng();

        let mut keys = k_as.iter().chain(&k_bs).cloned().collect::<Vec<_>>();
        */

        // keys.append(&mut k_bs);
        // let keys = k_as.append(&mut k)

        // let ka0 = ("AAA", 0);
        // let ka1

        //

        Ok(())
    }
}
