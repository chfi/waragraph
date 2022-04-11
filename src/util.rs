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

pub mod buffer;

pub use buffer::*;

// TransactionResult<_, TransactionError<()>
pub type TxResult<T> = TransactionResult<T, TransactionError<Vec<u8>>>;

pub struct LabelStorage {
    // pub db: sled::Db,
    pub tree: sled::Tree,

    pub label_names: HashMap<Vec<u8>, u64>,

    buffers: Vec<BufferIx>,
    desc_sets: Vec<DescSetIx>,
}

impl LabelStorage {
    const POS_MASK: [u8; 10] = *b"p:01234567";
    const TEXT_MASK: [u8; 10] = *b"t:01234567";

    const BUF_MASK: [u8; 12] = *b"buf:01234567";
    const SET_MASK: [u8; 12] = *b"set:01234567";

    const TEXT_BUF_LEN: usize = 256;

    pub fn set_text_for(&self, name: &[u8], contents: &str) -> Result<()> {
        let key = self
            .text_key_for(name)
            .ok_or(anyhow!("Could not find label '{}'", name.as_bstr()))?;

        let bytes = contents.as_bytes();
        let len = bytes.len().min(Self::TEXT_BUF_LEN - 1);
        let value = &bytes[..len];
        self.tree.update_and_fetch(key, |_| Some(value))?;

        Ok(())
    }

    pub fn set_text_for_id(&self, id: u64, contents: &str) -> Result<()> {
        let key = self.text_key_for_id(id);

        let bytes = contents.as_bytes();
        let len = bytes.len().min(Self::TEXT_BUF_LEN - 1);
        let value = &bytes[..len];
        self.tree.update_and_fetch(key, |_| Some(value))?;

        Ok(())
    }

    pub fn label_len(&self, name: &[u8]) -> Result<usize> {
        let key = self
            .text_key_for(name)
            .ok_or(anyhow!("Could not find label '{}'", name.as_bstr()))?;

        let v = self.tree.get(&key)?.unwrap();
        Ok(v.len())
    }

    pub fn new(db: &sled::Db) -> Result<Self> {
        // db.drop_tree("labels")?;
        let tree = db.open_tree("labels")?;
        let label_names = HashMap::default();
        let buffers = Vec::new();
        let desc_sets = Vec::new();

        Ok(Self {
            tree,
            label_names,
            buffers,
            desc_sets,
        })
    }

    pub fn buffer_for_id(&self, id: u64) -> Result<Option<BufferIx>> {
        use zerocopy::FromBytes;

        let key = self
            .buf_key_for_id(id)
            .ok_or(anyhow!("Buffer not found for label ID '{}'", id))?;

        let bytes = self.tree.get(key)?;

        let result = bytes.and_then(|b| {
            let raw: u64 = u64::read_from(b.as_ref())?;
            let index = Index::from_bits(raw)?;
            Some(BufferIx(index))
        });

        Ok(result)
    }

    pub fn desc_set_for_id(&self, id: u64) -> Result<Option<DescSetIx>> {
        use zerocopy::FromBytes;

        let key = self
            .set_key_for_id(id)
            .ok_or(anyhow!("Descriptor set not found for label ID {}", id))?;

        let bytes = self.tree.get(key)?;

        let result = bytes.and_then(|b| {
            let raw: u64 = u64::read_from(b.as_ref())?;
            let index = Index::from_bits(raw)?;
            Some(DescSetIx(index))
        });

        Ok(result)
    }

    pub fn buffer_for(&self, name: &[u8]) -> Result<Option<BufferIx>> {
        use zerocopy::FromBytes;

        let key = self.buf_key_for(name).ok_or(anyhow!(
            "Buffer not found for label '{}'",
            name.as_bstr()
        ))?;

        let bytes = self.tree.get(key)?;

        let result = bytes.and_then(|b| {
            let raw: u64 = u64::read_from(b.as_ref())?;
            let index = Index::from_bits(raw)?;
            Some(BufferIx(index))
        });

        Ok(result)
    }

    pub fn desc_set_for(&self, name: &[u8]) -> Result<Option<DescSetIx>> {
        use zerocopy::FromBytes;

        let key = self.set_key_for(name).ok_or(anyhow!(
            "Descriptor set not found for label '{}'",
            name.as_bstr()
        ))?;

        let bytes = self.tree.get(key)?;

        let result = bytes.and_then(|b| {
            let raw: u64 = u64::read_from(b.as_ref())?;
            let index = Index::from_bits(raw)?;
            Some(DescSetIx(index))
        });

        Ok(result)
    }

    fn buf_key_for_id(&self, id: u64) -> Option<[u8; 12]> {
        let mut res = Self::BUF_MASK;
        res[4..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    fn set_key_for_id(&self, id: u64) -> Option<[u8; 12]> {
        let mut res = Self::SET_MASK;
        res[4..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    fn pos_key_for_id(&self, id: u64) -> [u8; 10] {
        let mut res = Self::POS_MASK;
        res[2..].clone_from_slice(&id.to_le_bytes());
        res
    }

    fn buf_key_for(&self, name: &[u8]) -> Option<[u8; 12]> {
        let id = self.label_names.get(name)?;
        let mut res = Self::BUF_MASK;
        res[4..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    fn set_key_for(&self, name: &[u8]) -> Option<[u8; 12]> {
        let id = self.label_names.get(name)?;
        let mut res = Self::SET_MASK;
        res[4..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    fn pos_key_for(&self, name: &[u8]) -> Option<[u8; 10]> {
        let id = self.label_names.get(name)?;
        let mut res = Self::POS_MASK;
        res[2..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    pub fn set_pos_for_id(&self, id: u64, x: u32, y: u32) -> Result<()> {
        let key = self.pos_key_for_id(id);
        let mut val = [0u8; 8];
        val[..4].clone_from_slice(&x.to_le_bytes());
        val[4..].clone_from_slice(&y.to_le_bytes());
        self.tree.insert(key, &val)?;
        Ok(())
    }

    pub fn set_label_pos(&self, name: &[u8], x: u32, y: u32) -> Result<()> {
        let key = self.pos_key_for(name).ok_or(anyhow!(
            "could not find key for label '{}'",
            name.as_bstr()
        ))?;
        let mut val = [0u8; 8];
        val[..4].clone_from_slice(&x.to_le_bytes());
        val[4..].clone_from_slice(&y.to_le_bytes());
        self.tree.insert(key, &val)?;
        Ok(())
    }

    pub fn get_label_pos(&self, name: &[u8]) -> Result<(u32, u32)> {
        use zerocopy::FromBytes;

        let val = self
            .pos_key_for(name)
            .and_then(|k| self.tree.get(k).ok().flatten())
            .ok_or(anyhow!(
                "could not find key for label '{}'",
                name.as_bstr()
            ))?;

        let x = u32::read_from(&val[0..4]).unwrap();
        let y = u32::read_from(&val[4..8]).unwrap();

        Ok((x, y))
    }

    pub fn get_label_pos_id(&self, id: u64) -> Result<(u32, u32)> {
        let key = self.pos_key_for_id(id);
        let val = self
            .tree
            .get(key)?
            .ok_or(anyhow!("could not find key for label ID {}", id))?;

        let x = u32::read_from(&val[0..4]).unwrap();
        let y = u32::read_from(&val[4..8]).unwrap();

        Ok((x, y))
    }

    pub fn get_label_id(&self, name: &[u8]) -> Option<u64> {
        self.label_names.get(name).copied()
    }

    pub fn get_label_text_by_id(&self, id: u64) -> Option<sled::IVec> {
        let key = self.text_key_for_id(id);
        self.tree.get(key).ok().flatten()
    }

    fn text_key_for_id(&self, id: u64) -> [u8; 10] {
        let mut res = Self::TEXT_MASK;
        res[2..].clone_from_slice(&id.to_le_bytes());
        res
    }

    fn text_key_for(&self, name: &[u8]) -> Option<[u8; 10]> {
        let id = self.label_names.get(name)?;
        let mut res = Self::TEXT_MASK;
        res[2..].clone_from_slice(&id.to_le_bytes());
        Some(res)
    }

    pub fn set_label_text(&self, name: &[u8], contents: &str) -> Result<()> {
        let key = self.text_key_for(name).ok_or(anyhow!(
            "could not find text key for label '{}'",
            name.as_bstr()
        ))?;

        let max_len = contents.len().min(254);
        let value = contents[..max_len].as_bytes();
        self.tree.update_and_fetch(key, |_| Some(value))?;
        Ok(())
    }

    pub fn allocate_label(
        &mut self,
        db: &sled::Db,
        engine: &mut VkEngine,
        name: &str,
    ) -> Result<u64> {
        let id = db.generate_id()?;

        let (buf, set) = engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
            let usage = vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST;

            let buffer = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                4,
                Self::TEXT_BUF_LEN,
                usage,
                Some(name),
            )?;

            let buf_ix = res.insert_buffer(buffer);

            let desc_set = allocate_buffer_desc_set(buf_ix, res)?;

            let set_ix = res.insert_desc_set(desc_set);

            Ok((buf_ix, set_ix))
        })?;

        self.label_names.insert(name.as_bytes().to_vec(), id);

        let buf_u64 = buf.0.to_bits();
        let set_u64 = set.0.to_bits();

        let tx_result: TxResult<()> = self.tree.transaction(|db| {
            let nb = name.as_bytes();
            let pk = self.pos_key_for(nb).unwrap();
            let tk = self.text_key_for(nb).unwrap();

            let buf_key = self.buf_key_for(nb).unwrap();
            let set_key = self.set_key_for(nb).unwrap();

            db.insert(&buf_key, &buf_u64.to_le_bytes())?;
            db.insert(&set_key, &set_u64.to_le_bytes())?;

            db.insert(&pk, &[0u8; 8])?;
            db.insert(&tk, b"")?;

            Ok(())
        });

        match tx_result {
            Ok(_) => {}
            Err(err) => {
                //
            }
        }

        Ok(id)
    }
}

pub fn gradient_color_fn(
    gradient: colorous::Gradient,
    n: usize,
) -> impl Fn(usize) -> [u8; 16] {
    move |ix| {
        let color = gradient.eval_rational(ix, n);
        let to_bytes = |c| ((c as f32) / 255.0).to_ne_bytes();
        let r = to_bytes(color.r);
        let g = to_bytes(color.g);
        let b = to_bytes(color.b);
        let a = 1.0f32.to_ne_bytes();

        [
            r[0], r[1], r[2], r[3], g[0], g[1], g[2], g[3], b[0], b[1], b[2],
            b[3], a[0], a[1], a[2], a[3],
        ]
    }
}

pub fn alloc_gradient_buffer(
    engine: &mut VkEngine,
    name: Option<&str>,
    gradient: colorous::Gradient,
    n: usize,
) -> Result<BufferIx> {
    let usage = vk::BufferUsageFlags::TRANSFER_DST
        | vk::BufferUsageFlags::STORAGE_BUFFER;

    alloc_buffer_with(engine, name, usage, true, 0..n, |ix| {
        let color = gradient.eval_rational(ix, n);
        let to_bytes = |c| ((c as f32) / 255.0).to_ne_bytes();
        let r = to_bytes(color.r);
        let g = to_bytes(color.g);
        let b = to_bytes(color.b);
        let a = 1.0f32.to_ne_bytes();

        [
            r[0], r[1], r[2], r[3], g[0], g[1], g[2], g[3], b[0], b[1], b[2],
            b[3], a[0], a[1], a[2], a[3],
        ]
    })
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

pub fn allocate_buffer_desc_set(
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
