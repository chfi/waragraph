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

    // uniform: BufferIx,
    buffer: BufferIx,

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
        let mut buffer = Self::allocate_buffer(ctx, res, alloc, width, name)?;

        let slice = buffer
            .mapped_slice_mut()
            .ok_or(anyhow!("Path slot buffer could not be memory mapped"))?;

        let data = (0..width)
            .flat_map(|i| fill(i).to_ne_bytes())
            .collect::<Vec<u8>>();

        slice.clone_from_slice(&data);

        let name = name.map(String::from);

        let buffer = res.insert_buffer(buffer);

        Ok(Self {
            capacity: width,
            width,

            buffer,
            name,
        })
    }

    pub fn resize(
        &mut self,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        new_width: usize,
        fill: u32,
    ) -> Result<()> {
        if new_width <= self.capacity {
            self.width = new_width;
            return Ok(());
        }

        let buffer_ix = self.buffer;

        let mut new_data = {
            let mut new_data = Vec::with_capacity(new_width);
            let slice = res[buffer_ix].mapped_slice_mut().ok_or(anyhow!(
                "Path slot buffer could not be memory mapped"
            ))?;
            new_data.clone_from_slice(slice);
            new_data
        };

        let fb = fill.to_ne_bytes();
        for _ in 0..(new_width - self.width) {
            new_data.extend_from_slice(&fb);
        }

        let name = self.name.as_deref();

        let mut new_buffer =
            Self::allocate_buffer(ctx, res, alloc, new_width, name)?;

        let slice = new_buffer
            .mapped_slice_mut()
            .ok_or(anyhow!("Path slot buffer could not be memory mapped"))?;

        slice.clone_from_slice(&new_data);

        if let Some(old_buffer) = res.insert_buffer_at(buffer_ix, new_buffer) {
            res.free_buffer(ctx, alloc, old_buffer)?;
        }

        self.width = new_width;

        Ok(())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn width(&self) -> usize {
        self.width
    }

    fn allocate_buffer(
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        width: usize,
        name: Option<&str>,
    ) -> Result<BufferRes> {
        let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        res.allocate_buffer(ctx, alloc, mem_loc, 4, width, usage, name)
    }
}
