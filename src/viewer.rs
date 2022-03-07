use std::collections::BTreeMap;

use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources,
};
use rspirv_reflect::DescriptorInfo;

use anyhow::{anyhow, Result};

#[derive(Clone, Copy)]
pub struct ViewDiscrete1D {
    max: usize,
    offset: usize,
    len: usize,
}

impl ViewDiscrete1D {
    pub fn new(max: usize) -> Self {
        Self {
            max,

            offset: 0,
            len: max,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_valid(&self) -> bool {
        self.len > 0 && (self.offset + self.len <= self.max)
    }

    pub fn reset(&mut self) {
        self.offset = 0;
        self.len = self.max;
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..(self.offset + self.len)
    }

    pub fn translate(&mut self, delta: isize) {
        let d = delta.abs() as usize;

        if delta.is_negative() {
            if d > self.offset {
                self.offset = 0;
            } else {
                self.offset -= d;
            }
        } else if delta.is_positive() {
            if self.offset + d + self.len > self.max {
                self.offset = self.max - self.len;
            } else {
                self.offset += d;
            }
        }
    }

    pub fn resize(&mut self, new_len: usize) {
        let mid = self.offset + (self.len / 2);

        let new_hl = new_len / 2;

        self.len = new_len;
        if new_hl > mid {
            self.offset = 0;
        } else if mid + new_hl > self.max {
            self.offset = self.max - new_len;
        } else {
            self.offset = mid - new_hl;
        }
    }

    /*
    pub fn resize_around(&mut self, origin: usize, delta: isize) {
        // make sure the origin is actually within the current view? how?

        let range = self.range();
        let origin = origin.clamp(range.start, range.end - 1);
    }
    */
}

pub struct PathViewer {
    width: usize,
    slots: Vec<PathViewSlot>,
}

impl PathViewer {
    pub fn new(
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        width: usize,
        slot_count: usize,
        fill: u32,
        name_prefix: &str,
    ) -> Result<Self> {
        let mut slots = Vec::with_capacity(slot_count);

        for i in 0..slot_count {
            let name = format!("{}_slot_{}", name_prefix, i);
            let slot =
                PathViewSlot::new(ctx, res, alloc, width, Some(&name), |_| {
                    fill
                })?;

            slots.push(slot);
        }

        Ok(Self { width, slots })
    }
}

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

        // for (i, win) in slice.chunks_exact_mut(4).enumerate() {
        //     let bytes = fill(i).to_ne_bytes();
        //     bytes.into_iter().zip(win).for_each(|(b, w)| *w = b);
        // }

        let data = (0..width)
            .flat_map(|i| fill(i).to_ne_bytes())
            .collect::<Vec<u8>>();

        slice.clone_from_slice(&data);

        let name = name.map(String::from);

        let buffer = res.insert_buffer(buffer);

        let desc_set = Self::allocate_desc_set(buffer, res)?;
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
        // log::warn!(
        //     "old_width: {}\tcapacity: {}\tnew_width: {}",
        //     self.width,
        //     self.capacity,
        //     new_width
        // );
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
        for _ in 0..(new_width - self.width) {
            new_data.extend_from_slice(&fb);
        }

        let name = self.name.as_deref();

        let mut new_buffer =
            Self::allocate_buffer(ctx, res, alloc, new_width, name)?;

        let slice = new_buffer
            .mapped_slice_mut()
            .ok_or(anyhow!("Path slot buffer could not be memory mapped"))?;

        slice.clone_from_slice(&new_data[..slice.len()]);

        if let Some(old_buffer) = res.insert_buffer_at(buffer_ix, new_buffer) {
            res.free_buffer(ctx, alloc, old_buffer)?;
        }

        self.width = new_width;
        self.capacity = new_width;

        let desc_set = Self::allocate_desc_set(self.buffer, res)?;
        let _ = res.insert_desc_set_at(self.desc_set, desc_set);

        Ok(())
    }

    pub fn update_from<F>(
        &mut self,
        res: &mut GpuResources,
        buf: &mut Vec<u8>,
        mut fill: F,
    ) -> Option<()>
    where
        F: FnMut(usize) -> u32,
    {
        let slice = res[self.buffer].mapped_slice_mut()?;
        buf.clear();
        buf.extend((0..self.width).flat_map(|i| fill(i).to_ne_bytes()));
        slice[..buf.len()].clone_from_slice(&buf);

        /*
        if slice.len() % 4 != 0 {
            log::error!("buffer chunks shouldn't have any remainder");
        }

        let chunks = slice.chunks_exact_mut(4);

        for (i, win) in slice.chunks_exact_mut(4).enumerate() {
            let bytes = fill(i).to_ne_bytes();
            bytes.into_iter().zip(win).for_each(|(b, w)| *w = b);
        }
        */

        Some(())
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
        buffer: BufferIx,
        res: &mut GpuResources,
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
