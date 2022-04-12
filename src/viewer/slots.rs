use std::collections::{BTreeMap, HashMap};

use ash::vk;
use bstr::ByteSlice;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use rustc_hash::{FxHashMap, FxHashSet};
use sprs::CsVecI;
use zerocopy::{AsBytes, FromBytes};

use anyhow::{anyhow, Result};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

use crate::{
    graph::{Node, Path, Waragraph},
    util::LabelStorage,
};

// pub type Path = usize;

pub type DataSource =
    Arc<dyn Fn(usize, Node) -> Option<u32> + Send + Sync + 'static>;

pub type SlotUpdateFn<T> =
    Arc<dyn Fn(&[(Node, usize)], usize, usize) -> T + Send + Sync + 'static>;

pub struct Slot {
    pub path: Option<usize>,
    pub view: Option<(usize, usize)>,
    // the updated width, not the allocated
    pub width: Option<usize>,

    pub slot: PathViewSlot,

    pub label_id: u64,
}

#[derive(Default)]
pub struct SlotCache {
    pub slots: Vec<Slot>,
    pub path_map: FxHashMap<Path, usize>, // value is index into `slots`
}

impl SlotCache {
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn bind_paths(
        &mut self,
        paths: impl IntoIterator<Item = Path>,
    ) -> Result<()> {
        let mut to_insert = paths.into_iter().collect::<FxHashSet<_>>();

        let mut to_remove = Vec::new();

        for path in self.path_map.keys() {
            if to_insert.remove(path) {
            } else {
                to_remove.push(*path);
            }
        }

        for path in to_remove {
            self.unbind_path(path);
        }

        for path in to_insert {
            if let Some(slot) =
                self.bind_path(path).and_then(|i| self.slots.get_mut(i))
            {
                slot.path = Some(path);
            }
        }

        Ok(())
    }

    pub fn allocate_slot(
        &mut self,
        engine: &mut VkEngine,
        db: &sled::Db,
        txt: &mut LabelStorage,
        width: usize,
    ) -> Result<usize> {
        let i = self.slots.len();
        let name = format!("path-viewer-slot-{}", i);

        let label_id = txt.allocate_label(db, engine, &name)?;

        let slot = engine.with_allocators(|ctx, res, alloc| {
            PathViewSlot::new(ctx, res, alloc, width, Some(&name))
        })?;

        let slot = Slot {
            path: None,
            view: None,
            width: None,
            label_id,

            slot,
        };
        self.slots.push(slot);

        Ok(i)
    }

    pub fn update_bound_slot(
        &mut self,
        res: &mut GpuResources,
        path: Path,
        view_offset: usize,
        view_len: usize,
        contents: &[u32],
    ) -> Option<()> {
        let slot = self
            .path_map
            .get(&path)
            .and_then(|&i| self.slots.get_mut(i))?;

        if slot.path != Some(path) {
            return None;
        }

        slot.slot.fill_from(res, contents)?;
        slot.view = Some((view_offset, view_len));

        Some(())
    }

    pub fn get_slot_for(&self, path: Path) -> Option<&Slot> {
        let slot_ix = *self.path_map.get(&path)?;
        let slot = self.slots.get(slot_ix)?;
        slot.path.is_some().then(|| slot)
    }

    pub fn get_slot_mut_for(&mut self, path: Path) -> Option<&mut Slot> {
        let slot_ix = *self.path_map.get(&path)?;
        let slot = self.slots.get_mut(slot_ix)?;
        slot.path.is_some().then(|| slot)
    }

    /// returns the index for the path if it is (or already was)
    /// successfully bound to a slot
    ///
    /// if not successful, more slots need to be allocated
    pub fn bind_path(&mut self, path: Path) -> Option<usize> {
        if let Some(slot_ix) = self.path_map.get(&path) {
            return Some(*slot_ix);
        }

        if let Some((slot_ix, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.path.is_none())
        {
            slot.view = None;
            slot.width = None;
            self.path_map.insert(path, slot_ix);
            return Some(slot_ix);
        }

        None
    }

    pub fn unbind_path(&mut self, path: Path) {
        if let Some(slot) = self
            .path_map
            .remove(&path)
            .and_then(|i| self.slots.get_mut(i))
        {
            slot.path = None;
            slot.view = None;
            slot.width = None;
        }
    }
}

#[derive(Default)]
pub struct SlotRenderers {
    data_sources: HashMap<String, DataSource>,
}

impl SlotRenderers {
    pub fn register_data_source<F>(&mut self, id: &str, f: F)
    where
        F: Fn(usize, Node) -> Option<u32> + Send + Sync + 'static,
    {
        let data_source = Arc::new(f) as DataSource;
        self.data_sources.insert(id.to_string(), data_source);
    }

    pub fn get_data_source(&self, id: &str) -> Option<&DataSource> {
        self.data_sources.get(id)
    }

    pub fn create_sampler_prefix_sum_mean_with<F>(
        &self,
        graph: &Arc<Waragraph>,
        val_data_source: &str,
        sum_data_source: &str,
        f: F,
    ) -> Option<SlotUpdateFn<u32>>
    where
        F: Fn(f32) -> u32 + Send + Sync + 'static,
    {
        let val_data_source = self.get_data_source(val_data_source)?.clone();
        let sum_data_source = self.get_data_source(sum_data_source)?.clone();
        let graph = graph.clone();

        let f = move |samples: &[(Node, usize)], path, ix: usize| {
            let left_ix = ix.min(samples.len() - 1);
            let right_ix = (ix + 1).min(samples.len() - 1);

            let (left, l_offset) = samples[left_ix];
            let (right, r_offset) = samples[right_ix];

            let li: usize = left.into();
            let ri: usize = right.into();

            let left_start = graph.node_sum_lens[li];
            let right_start = graph.node_sum_lens[ri];

            let l_node_val =
                val_data_source(path, left).unwrap_or_default() as usize;
            let r_node_val =
                val_data_source(path, right).unwrap_or_default() as usize;

            let mut len = right_start - left_start;

            let l_val = sum_data_source(path, left).unwrap_or_default();
            let r_val = sum_data_source(path, right).unwrap_or_default();

            let l_val = l_val as usize;
            let r_val = r_val as usize;

            let mut val = r_val - l_val;

            // add the left chunk of the right node
            val += r_offset * r_node_val;
            // remove the left chunk of the left node
            val = val.checked_sub(l_offset * l_node_val).unwrap_or_default();

            len -= l_offset;
            len += r_offset;

            let avg = val as f32 / len as f32;

            f(avg)
        };
        Some(Arc::new(f) as SlotUpdateFn<u32>)
    }

    pub fn create_sampler_mean_with<F>(
        &self,
        data_source: &str,
        f: F,
    ) -> Option<SlotUpdateFn<u32>>
    where
        F: Fn(f32) -> u32 + Send + Sync + 'static,
    {
        let data_source = self.get_data_source(data_source)?.clone();

        let f = move |samples: &[(Node, usize)], path, ix: usize| {
            let left_ix = ix.min(samples.len() - 1);
            let right_ix = (ix + 1).min(samples.len() - 1);

            let (left, _offset) = samples[left_ix];
            let (right, _offset) = samples[right_ix];

            let mut total = 0;
            let mut count = 0;

            let l: u32 = left.into();
            let r: u32 = right.into();

            for n in l..r {
                let node = Node::from(n);
                if let Some(v) = data_source(path, node) {
                    total += v;
                    count += 1;
                }
            }

            let avg = if count == 0 {
                0.0
            } else {
                (total as f32) / (count as f32)
            };

            f(avg)
        };
        Some(Arc::new(f) as SlotUpdateFn<u32>)
    }

    pub fn create_sampler_mean_round(
        &self,
        data_source: &str,
    ) -> Option<SlotUpdateFn<u32>> {
        self.create_sampler_mean_with(data_source, |v| v as u32)
    }

    pub fn create_sampler_mid_with<F>(
        &self,
        data_source: &str,
        f: F,
    ) -> Option<SlotUpdateFn<u32>>
    where
        F: Fn(u32) -> u32 + Send + Sync + 'static,
    {
        let data_source = self.get_data_source(data_source)?.clone();

        let f = move |samples: &[(Node, usize)], path, ix: usize| {
            let left_ix = ix.min(samples.len() - 1);
            let right_ix = (ix + 1).min(samples.len() - 1);

            let (left, _offset) = samples[left_ix];
            let (right, _offset) = samples[right_ix];

            let l: u32 = left.into();
            let r: u32 = right.into();

            let node = l + (r - l) / 2;
            let v = data_source(path, node.into()).unwrap_or_default();

            f(v)
        };
        Some(Arc::new(f) as SlotUpdateFn<u32>)
    }

    pub fn create_sampler_mid(
        &self,
        data_source: &str,
    ) -> Option<SlotUpdateFn<u32>> {
        self.create_sampler_mid_with(data_source, |v| v)
    }
}

pub struct PathViewSlot {
    capacity: usize,
    width: usize,

    // uniform: BufferIx,
    buffer: BufferIx,
    desc_set: DescSetIx,

    name: Option<String>,

    visible: bool,
}

impl PathViewSlot {
    pub fn new(
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        width: usize,
        name: Option<&str>,
    ) -> Result<Self> {
        let buffer = Self::allocate_buffer(ctx, res, alloc, width, name)?;
        let name = name.map(String::from);

        let buffer = res.insert_buffer(buffer);

        let desc_set = crate::util::allocate_buffer_desc_set(buffer, res)?;
        let desc_set = res.insert_desc_set(desc_set);

        Ok(Self {
            capacity: width,
            width,

            buffer,
            desc_set,

            name,

            visible: true,
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

        let size = new_buffer.alloc.size().min(new_data.len() as u64) as usize;

        let slice = new_buffer
            .mapped_slice_mut()
            .ok_or(anyhow!("Path slot buffer could not be memory mapped"))?;

        slice[..size].clone_from_slice(&new_data[..size]);

        if let Some(old_buffer) = res.insert_buffer_at(buffer_ix, new_buffer) {
            res.free_buffer(ctx, alloc, old_buffer)?;
        }

        self.width = new_width;
        self.capacity = new_width;

        let desc_set = crate::util::allocate_buffer_desc_set(self.buffer, res)?;
        let _ = res.insert_desc_set_at(self.desc_set, desc_set);

        Ok(())
    }

    pub fn fill_from(
        &mut self,
        res: &mut GpuResources,
        data: &[u32],
    ) -> Option<()> {
        let dst = res[self.buffer].mapped_slice_mut()?;

        dst.chunks_exact_mut(4).take(self.width).zip(data).for_each(
            |(dst, src)| {
                let src = &[*src];
                let src: &[u8] = bytemuck::cast_slice(src);
                dst[0] = src[0];
                dst[1] = src[1];
                dst[2] = src[2];
                dst[3] = src[3];
            },
        );

        Some(())
    }

    pub fn update_from<F>(
        &mut self,
        res: &mut GpuResources,
        mut fill: F,
    ) -> Option<()>
    where
        F: FnMut(usize) -> u32,
    {
        // let src = (0..self.width).map(fill).collect::<Vec<_>>();
        // self.fill_from(res, &src)?;

        let slice = res[self.buffer].mapped_slice_mut()?;

        slice
            .chunks_exact_mut(4)
            .take(self.width)
            .enumerate()
            .for_each(|(i, c)| c.clone_from_slice(&fill(i).to_ne_bytes()));

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
}
