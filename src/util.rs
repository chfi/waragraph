use ash::vk;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{context::VkContext, BufferIx, GpuResources, VkEngine};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use anyhow::{anyhow, bail, Result};

pub fn alloc_buffer_with<F, const N: usize>(
    engine: &mut VkEngine,
    name: Option<&str>,
    usage: vk::BufferUsageFlags,
    f: F,
    indices: std::ops::Range<usize>,
) -> Result<BufferIx>
where
    F: FnMut(usize) -> [u8; N],
{
    let len = indices.len();
    let data = indices.map(f).flatten().collect::<Vec<u8>>();

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