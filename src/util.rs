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

use crossbeam::atomic::AtomicCell;

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result};
// TransactionResult<_, TransactionError<()>
pub type TxResult<T> = TransactionResult<T, TransactionError<Vec<u8>>>;

pub struct LabelStorage {
    db: sled::Db,

    label_names: HashMap<Vec<u8>, u64>,

    buffers: Vec<BufferIx>,
    desc_sets: Vec<DescSetIx>,
}

impl LabelStorage {
    const POS_MASK: [u8; 10] = *b"p:01234567";
    const TEXT_MASK: [u8; 10] = *b"t:01234567";

    pub fn new() -> Result<Self> {
        let db = sled::open("waragraph_labels")?;

        let label_names = HashMap::default();
        let buffers = Vec::new();
        let desc_sets = Vec::new();

        Ok(Self {
            db,
            label_names,
            buffers,
            desc_sets,
        })
    }

    // pub fn set_label_pos(&mut self, x: u32, y: u32) -> Option<()> {}

    fn pos_key_for(&self, name: &str) -> Option<[u8; 10]> {
        let id = self.label_names.get(name.as_bytes())?;
        let mut res = Self::POS_MASK;
        res[2..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    pub fn set_label_pos(&self, name: &str, x: u32, y: u32) -> Result<()> {
        let key = self
            .pos_key_for(name)
            .ok_or(anyhow!("could not find key for label '{}'", name))?;
        let mut val = [0u8; 8];
        val[..4].clone_from_slice(&x.to_le_bytes());
        val[4..].clone_from_slice(&y.to_le_bytes());
        self.db.insert(key, &val)?;
        Ok(())
    }

    fn text_key_for(&self, name: &str) -> Option<[u8; 10]> {
        let id = self.label_names.get(name.as_bytes())?;
        let mut res = Self::TEXT_MASK;
        res[2..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    pub fn set_label_text(&self, name: &str, contents: &str) -> Result<()> {
        let key = self
            .text_key_for(name)
            .ok_or(anyhow!("could not find text key for label '{}'", name))?;

        let max_len = contents.len().min(254);
        self.db.insert(key, contents[..max_len].as_bytes())?;
        Ok(())
    }

    pub fn allocate_label(
        &mut self,
        engine: &mut VkEngine,
        name: &str,
    ) -> Result<()> {
        let id = self.db.generate_id()?;

        self.label_names.insert(name.as_bytes().to_vec(), id);

        let tx_result: TxResult<()> = self.db.transaction(|db| {
            let pk = self.pos_key_for(name).unwrap();
            let tk = self.text_key_for(name).unwrap();

            db.insert(&pk, &[0u8; 8])?;
            // TODO remove test placeholder
            db.insert(&tk, b"hello world")?;

            Ok(())
        });

        match tx_result {
            Ok(_) => {
                // all good
            }
            Err(err) => {
                //
            }
        }

        /*
        let result: TransactionResult<_, TransactionError<()>> =
            self.db.transaction(|db| {
                // self.db.transaction(|db| {
                db.insert(b"k1", b"cats")?;
                db.insert(b"k2", b"dogs")?;
                Ok(())
            });
        let x = result?;
        */

        Ok(())
    }
}

#[derive(Default)]
pub struct LabelBuffers {
    names: HashMap<String, usize>,

    label_len: Vec<usize>,
    buffers: Vec<BufferIx>,
    desc_sets: Vec<DescSetIx>,
}

impl LabelBuffers {
    pub const BUFFER_LEN: usize = 255;

    pub fn get_desc_set(&self, name: &str) -> Option<DescSetIx> {
        let ix = *self.names.get(name)?;
        self.desc_sets.get(ix).copied()
    }

    pub fn get_len(&self, name: &str) -> Option<usize> {
        let ix = *self.names.get(name)?;
        self.label_len.get(ix).copied()
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

        self.label_len.push(0);
        self.buffers.push(buf_ix);
        self.desc_sets.push(set_ix);

        Ok(())
    }

    // pub fn write_buffer(&mut self, res: &mut GpuResources, name: &str, contents: &[u8]) -> Option<()> {
    pub fn write_buffer(
        &mut self,
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

        self.label_len[ix] = len;

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
