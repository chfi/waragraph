use std::collections::{BTreeMap, HashMap};

use ash::vk;
use bimap::BiHashMap;
use bstr::ByteSlice;
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
    ElemSizeMismatch { actual: usize, expected: usize },
    RowSizeMismatch { actual: usize, expected: usize },
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::OutOfRows => {
                write!(f, "Buffer cache allocation error: Out of rows")
            }
            CacheError::RowSizeMismatch { actual, expected } => {
                write!(f, "Buffer cache update error: Data consisted of {} elements, but row expected {}", actual, expected)
            }
            CacheError::ElemSizeMismatch { actual, expected } => {
                write!(f, "Buffer cache update error: Element size in new buffer is {}, but cache expected {}", actual, expected)
            }
        }
    }
}

impl std::error::Error for CacheError {}

pub struct BufferCache<K>
where
    K: std::hash::Hash,
{
    usage: vk::BufferUsageFlags,

    elem_size: usize,
    row_size: usize,

    row_capacity: usize,

    buffer: BufferIx,
    desc_set: DescSetIx,
    // updates_tx: crossbeam::channel::Sender<()>,
    // updates_rx: crossbeam::channel::Receiver<()>,
    row_map: HashMap<K, Range<usize>>,
}

impl<K: std::hash::Hash> BufferCache<K> {
    pub fn new(
        engine: &mut VkEngine,
        elem_size: usize,
        row_size: usize,
        row_count: usize,
    ) -> Result<Self> {
        todo!();
    }

    pub fn clear(&mut self) {
        self.row_map.clear()
    }

    pub fn write_row(
        &mut self,
        row: usize,
        data: &[u8],
    ) -> std::result::Result<(), CacheError> {
        todo!();
    }
}
