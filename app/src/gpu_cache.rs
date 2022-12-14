use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use anyhow::{anyhow, Result};

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

    fn write_block<Q: ?Sized>(
        &mut self,
        buffer: &mut [u8],
        block: &Q,
        data: &[u8],
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

    usage: wgpu::BufferUsages,
    buffer: wgpu::Buffer,
    // bind_group: wgpu::BindGroup,
    cache: BufferCache<K>,
}

impl<K> GpuBufferCache<K>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
{
    pub fn new(
        device: &wgpu::Device,
        usage: wgpu::BufferUsages,
        name: &str,
        elem_size: usize,
        block_size: usize,
        block_capacity: usize,
    ) -> Result<Self> {
        let cache: BufferCache<K> =
            BufferCache::new(elem_size, block_size, block_capacity);
        let capacity = cache.buffer_size();

        let usage = usage | wgpu::BufferUsages::COPY_DST;

        let label = format!("GPU Cache: {name}");
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label.as_str()),
            size: capacity as u64,
            usage,
            mapped_at_creation: false,
        });

        Ok(Self {
            name: name.to_string(),
            usage,
            buffer,
            cache,
        })
    }
}