use std::collections::{BTreeMap, HashMap};

use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

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
    desc_set: DescSetIx,

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

        let desc_set = Self::allocate_desc_set(buffer, ctx, res, alloc)?;
        let desc_set = res.insert_desc_set(desc_set);

        Ok(Self {
            capacity: width,
            width,

            buffer,
            desc_set,

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
        log::warn!(
            "old_width: {}\tcapacity: {}\tnew_width: {}",
            self.width,
            self.capacity,
            new_width
        );
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
            new_data.extend_from_slice(slice);
            new_data
        };

        let fb = fill.to_ne_bytes();
        log::warn!("diff: {}", new_width - self.width);
        for _ in 0..(new_width - self.width) {
            new_data.extend_from_slice(&fb);
        }

        let name = self.name.as_deref();

        let mut new_buffer =
            Self::allocate_buffer(ctx, res, alloc, new_width, name)?;

        let slice = new_buffer
            .mapped_slice_mut()
            .ok_or(anyhow!("Path slot buffer could not be memory mapped"))?;

        slice[..new_width].clone_from_slice(&new_data[..new_width]);

        if let Some(old_buffer) = res.insert_buffer_at(buffer_ix, new_buffer) {
            res.free_buffer(ctx, alloc, old_buffer)?;
        }

        self.width = new_width;
        self.capacity = new_width;

        let desc_set = Self::allocate_desc_set(self.buffer, ctx, res, alloc)?;
        let _ = res.insert_desc_set_at(self.desc_set, desc_set);

        Ok(())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn buffer(&self) -> BufferIx {
        self.buffer
    }

    pub fn desc_set(&self) -> DescSetIx {
        self.desc_set
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

    fn allocate_desc_set(
        // &self,
        buffer: BufferIx,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
    ) -> Result<vk::DescriptorSet> {
        // TODO also do uniforms if/when i add them, or keep them in a
        // separate set
        let layout_info = {
            let mut info = DescriptorLayoutInfo::default();

            let binding = vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .stage_flags(vk::ShaderStageFlags::COMPUTE) // TODO should also be graphics, probably
                .build();

            info.bindings.push(binding);
            info
        };

        let set_info = {
            let info = DescriptorInfo {
                ty: rspirv_reflect::DescriptorType::STORAGE_BUFFER,
                binding_count: rspirv_reflect::BindingCount::One,
                name: "samples".to_string(),
            };

            Some((0u32, info)).into_iter().collect::<BTreeMap<_, _>>()
        };

        res.allocate_desc_set_raw(&layout_info, &set_info, |res, builder| {
            let buffer = &res[buffer];
            let info = ash::vk::DescriptorBufferInfo::builder()
                .buffer(buffer.buffer)
                .offset(0)
                .range(ash::vk::WHOLE_SIZE)
                .build();
            let buffer_info = [info];
            builder.bind_buffer(0, &buffer_info);
            Ok(())
        })
    }
}
