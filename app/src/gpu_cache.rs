use anyhow::{anyhow, Result};

/*
slot/row-based GPU cache:
- one GPU buffer per cache
- buffer alloc. parameterized over row count & row size (available)
- reallocate to increase, use less of buffer when bin count decreases
- sample data in parallel, collect results and update entire buffer at once
 */

/*
    The buffer will always be compatible with this layout:

layout (set = 0, binding = 0) readonly buffer DataBuf {
  uint total_size;
  uint row_size;
  float values[];
} data;

    where the `row_size` and `total_size` are given in elements,
    so that `values` always contains `total_size` elements divided
    into blocks of `row_size`.
 */

pub struct GpuRowCacheState {
    elem_size: usize,

    used_columns: usize,
    used_rows: usize,

    column_capacity: usize,
    row_capacity: usize,
}


impl GpuRowCacheState {

    pub const METADATA_SIZE: usize = std::mem::size_of::<[u32; 4]>();

    pub fn new<T>(row_capacity: usize, column_capacity: usize) -> Self
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let elem_size = std::mem::size_of::<T>();
        Self {
            elem_size,

            used_columns: 0,
            used_rows: 0,

            column_capacity,
            row_capacity,
        }
    }

    /// The size needed for the full allocation.
    pub fn buffer_size_bytes(&self) -> usize {
        Self::METADATA_SIZE + self.capacity_bytes()
    }

    /// The maximum size of the data, in bytes, before a reallocation is needed.
    pub fn capacity_bytes(&self) -> usize {
        self.elem_size * self.column_capacity * self.row_capacity
    }

    /// The used size of the data, in bytes.
    pub fn used_bytes(&self) -> usize {
        self.elem_size * self.used_columns * self.used_rows
    }
}

/*

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
*/
