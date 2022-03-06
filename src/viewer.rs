use std::{collections::HashMap, num::NonZeroU32};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, BufferIx, BufferRes, GpuResources, VkEngine,
};
use rustc_hash::FxHashMap;
use thunderdome::{Arena, Index};

use sprs::{CsMat, CsMatI, CsVec, CsVecI, CsVecView, TriMat, TriMatI};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

pub struct PathViewSlot {
    capacity: usize,
    width: usize,

    buffer: BufferRes,

    name: Option<String>,
}

impl PathViewSlot {
    pub fn new<F>(
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        width: usize,
        name: Option<&str>,
        mut fill: F,
    ) -> Result<Self>
    where
        F: FnMut(usize) -> u32,
    {
        let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
        // let usage = vk::BufferUsageFlags::STORAGE_BUFFER
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        let mut buffer =
            res.allocate_buffer(ctx, alloc, mem_loc, 4, width, usage, name)?;

        let slice = buffer
            .mapped_slice_mut()
            .ok_or(anyhow!("Path slot buffer could not be memory mapped"))?;

        let data = (0..width)
            .flat_map(|i| fill(i).to_ne_bytes())
            .collect::<Vec<u8>>();

        slice.clone_from_slice(&data);

        let name = name.map(String::from);

        Ok(Self {
            capacity: width,
            width,

            buffer,
            name,
        })
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn width(&self) -> usize {
        self.width
    }
}
