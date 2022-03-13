use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use sled::transaction::{TransactionError, TransactionResult};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use thunderdome::Index;

use crossbeam::atomic::AtomicCell;

use bstr::ByteSlice;

use zerocopy::{AsBytes, FromBytes};

use parking_lot::Mutex;

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

pub struct BufferStorage {
    pub tree: sled::Tree,

    buffers: Vec<BufferIx>,
    desc_sets: Vec<DescSetIx>,
}

impl BufferStorage {
    const BUF_IX_MASK: [u8; 10] = *b"B:01234567";
    const SET_IX_MASK: [u8; 10] = *b"S:01234567";

    const BUF_DATA_MASK: [u8; 10] = *b"d:01234567";
    // used to store e.g. "[u8;2]"; basically a simple schema
    const BUF_FMT_MASK: [u8; 10] = *b"f:01234567";

    pub fn new(db: &sled::Db) -> Result<Self> {
        let tree = db.open_tree("buffer_storage")?;

        let buffers = Vec::new();
        let desc_sets = Vec::new();

        Ok(Self {
            tree,
            buffers,
            desc_sets,
        })
    }

    // pub fn allocate_buffer(&mut self, name: &str,
}
