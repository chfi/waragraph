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
    RowSizeMismatch { actual: usize, expected: usize },
    ElemSizeMismatch,
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::OutOfRows => {
                write!(f, "Buffer cache allocation error: Unbound row")
            }
            CacheError::RowSizeMismatch { actual, expected } => {
                write!(f, "Buffer cache update error: Data consisted of {} elements, but row expected {}", actual, expected)
            }
            CacheError::ElemSizeMismatch => {
                write!(f, "Buffer cache update error: Data bytestring not evenly divisible with element size")
            } // CacheError::ElemSizeMismatch { actual, expected } => {
              //     write!(f, "Buffer cache update error: Data does not consist of  {}, but cache expected {}", actual, expected)
              // }
        }
    }
}

impl std::error::Error for CacheError {}

pub struct BufferCache<K>
where
    K: std::hash::Hash + Eq,
{
    usage: vk::BufferUsageFlags,

    elem_size: usize,
    row_size: usize,

    row_capacity: usize,

    buffer: BufferIx,
    desc_set: DescSetIx,
    // updates_tx: crossbeam::channel::Sender<()>,
    // updates_rx: crossbeam::channel::Receiver<()>,
    // row_map: HashMap<K, Range<usize>>,
    row_map: HashMap<K, usize>,

    used_rows: Vec<bool>,
    // used_rows: roaring::RoaringBitmap,
}

impl<K: std::hash::Hash + Eq> BufferCache<K> {
    pub fn new(
        engine: &mut VkEngine,
        elem_size: usize,
        row_size: usize,
        row_count: usize,
    ) -> Result<Self> {
        todo!();
    }

    pub fn clear(&mut self) {
        self.row_map.clear();
        self.used_rows.iter_mut().for_each(|v| *v = false);
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

        Ok(self.range_for_ix(row_ix))
    }

    pub fn unbind_row<Q: ?Sized>(&mut self, k: &Q) -> Option<()>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        let row_ix = self.row_map.remove(k)?;
        self.used_rows[row_ix] = false;
        Some(())
    }

    // pub fn reallocate

    // fn pick_row<Q: ?Sized>(

    pub fn write_row<Q: ?Sized>(
        &mut self,
        res: &mut GpuResources,
        row: &Q,
        data: &[u8],
    ) -> std::result::Result<(), CacheError>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        if data.len() % self.elem_size != 0 {
            return Err(CacheError::ElemSizeMismatch);
        }

        let elem_count = data.len() / self.elem_size;

        if elem_count != self.row_size {
            return Err(CacheError::RowSizeMismatch {
                actual: elem_count,
                expected: self.row_size,
            });
        }

        let range = if let Some(range) = self.row_map.get(row) {
            range
        } else {
            todo!();
        };

        let buffer = &mut res[self.buffer]
            .mapped_slice_mut()
            .expect("BufferCache can't be bound, impossible!");

        todo!();
    }
}
