use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashMap},
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

        let total_size = self.elem_size * self.row_size * self.row_capacity;

        if buffer.len() != total_size {
            return Err(CacheError::BufferSizeMismatch {
                actual: buffer.len(),
                expected: total_size,
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
