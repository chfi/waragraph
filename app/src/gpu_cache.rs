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
        let cache: BufferCache<K> = BufferCache::new(elem_size, block_size, block_capacity);
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


/*
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

*/