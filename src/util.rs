use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crossbeam::atomic::AtomicCell;

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};

#[derive(Default)]
pub struct LabelBuffers {
    names: HashMap<String, usize>,

    buffers: Vec<BufferIx>,
    desc_sets: Vec<DescSetIx>,
}

impl LabelBuffers {
    pub const BUFFER_LEN: usize = 255;

    pub fn get_desc_set(&self, name: &str) -> Option<DescSetIx> {
        let ix = *self.names.get(name)?;
        self.desc_sets.get(ix).copied()
    }

    // pub fn new_buffer(&mut self, engine: &mut VkEngine, name: &str) -> Result<Option<BufferRes>> {
    pub fn new_buffer(
        &mut self,
        engine: &mut VkEngine,
        name: &str,
    ) -> Result<()> {
        let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        let (buf_ix, set_ix) = engine.with_allocators(|ctx, res, alloc| {
            // TODO use 1 byte per char
            let buf = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                4,
                Self::BUFFER_LEN,
                usage,
                Some(name),
            )?;

            let desc_set = Self::allocate_desc_set(res, &buf)?;

            let buf_ix = res.insert_buffer(buf);
            let set_ix = res.insert_desc_set(desc_set);

            Ok((buf_ix, set_ix))
        })?;

        let ix = self.buffers.len();

        self.names.insert(name.to_string(), ix);

        self.buffers.push(buf_ix);
        self.desc_sets.push(set_ix);

        Ok(())
    }

    // pub fn write_buffer(&mut self, res: &mut GpuResources, name: &str, contents: &[u8]) -> Option<()> {
    pub fn write_buffer(
        &self,
        res: &mut GpuResources,
        name: &str,
        contents: &[u8],
    ) -> Option<()> {
        let ix = *self.names.get(name)?;
        let buf_ix = self.buffers[ix];

        let buffer = &mut res[buf_ix];
        let slice = buffer.mapped_slice_mut()?;

        // TODO make sure the contents are not too big
        let len = contents.len();

        slice[0..4].clone_from_slice(&(len as u32).to_ne_bytes());

        for (chk, &b) in slice[4..].chunks_mut(4).zip(contents) {
            chk[0] = b;
            chk[1] = b;
            chk[2] = b;
            chk[3] = b;
        }

        Some(())
    }

    fn allocate_desc_set(
        res: &mut GpuResources,
        buffer: &BufferRes,
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
                name: "text".to_string(),
            };

            Some((0u32, info)).into_iter().collect::<BTreeMap<_, _>>()
        };

        res.allocate_desc_set_raw(&layout_info, &set_info, |res, builder| {
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

pub fn alloc_buffer_with<F, const N: usize>(
    engine: &mut VkEngine,
    name: Option<&str>,
    usage: vk::BufferUsageFlags,
    prefix_len: bool,
    indices: std::ops::Range<usize>,
    f: F,
) -> Result<BufferIx>
where
    F: FnMut(usize) -> [u8; N],
{
    let mut len = indices.len();

    if prefix_len {
        // TODO this isn't quite correct, it assumes 4 byte vals
        len += 2;
    }

    let mut data = Vec::with_capacity(N * len);

    if prefix_len {
        data.extend(len.to_ne_bytes().into_iter());
        data.extend(len.to_ne_bytes().into_iter());
    }

    data.extend(indices.map(f).flatten());

    let buf_ix = engine.with_allocators(|ctx, res, alloc| {
        let buf = res.allocate_buffer(
            ctx,
            alloc,
            gpu_allocator::MemoryLocation::GpuOnly,
            N,
            len,
            usage,
            name,
        )?;

        let ix = res.insert_buffer(buf);
        Ok(ix)
    })?;

    let staging_buf = Arc::new(AtomicCell::new(None));

    let arc = staging_buf.clone();

    let fill_buf_batch =
        move |ctx: &VkContext,
              res: &mut GpuResources,
              alloc: &mut Allocator,
              cmd: vk::CommandBuffer| {
            let buf = &mut res[buf_ix];

            let staging = buf.upload_to_self_bytes(
                ctx,
                alloc,
                bytemuck::cast_slice(&data),
                cmd,
            )?;

            arc.store(Some(staging));

            Ok(())
        };

    let batches = vec![&fill_buf_batch as &_];

    let fence = engine.submit_batches_fence_alt(batches.as_slice())?;

    engine.block_on_fence(fence)?;

    for buf in staging_buf.take() {
        buf.cleanup(&engine.context, &mut engine.allocator).ok();
    }

    Ok(buf_ix)
}
