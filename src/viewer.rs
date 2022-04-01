use std::collections::BTreeMap;

use ash::vk;
use bstr::ByteSlice;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources,
};
use rspirv_reflect::DescriptorInfo;

use zerocopy::{AsBytes, FromBytes};

use anyhow::{anyhow, Result};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

pub mod app;

pub mod sampler;
pub mod slots;

pub use sampler::*;
pub use slots::*;

use crate::{
    graph::{Node, Waragraph},
    util::LabelStorage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewDiscrete1D {
    pub max: usize,
    pub offset: usize,
    pub len: usize,
}

impl ViewDiscrete1D {
    pub fn as_bytes(&self) -> [u8; 24] {
        let max = self.max.to_le_bytes();
        let offset = self.offset.to_le_bytes();
        let len = self.len.to_le_bytes();

        let mut result = [0; 24];
        result[0..8].clone_from_slice(&max);
        result[8..16].clone_from_slice(&offset);
        result[16..24].clone_from_slice(&len);
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let max = bytes.get(0..8)?;
        let offset = bytes.get(8..16)?;
        let len = bytes.get(16..24)?;

        let max = usize::read_from(max)?;
        let offset = usize::read_from(offset)?;
        let len = usize::read_from(len)?;

        Some(Self { max, offset, len })
    }

    pub fn new(max: usize) -> Self {
        Self {
            max,

            offset: 0,
            len: max,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn is_valid(&self) -> bool {
        self.len > 0 && (self.offset + self.len <= self.max)
    }

    pub fn reset(&mut self) {
        self.offset = 0;
        self.len = self.max;
    }

    pub fn set(&mut self, offset: usize, len: usize) {
        assert!(len > 0);
        assert!(offset + len <= self.max);
        self.offset = offset;
        self.len = len;
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..(self.offset + self.len)
    }

    pub fn translate(&mut self, delta: isize) {
        let d = delta.abs() as usize;

        // let offset = (self.offset as isize) + delta;
        // let offset = offset.clamp(0, (self.max - self.len) as isize);
        // self.offset = offset as usize;

        if delta.is_negative() {
            if d > self.offset {
                self.offset = 0;
            } else {
                self.offset -= d;
            }
        } else if delta.is_positive() {
            self.offset += d;
            if self.offset + self.len >= self.max {
                self.offset = self.max - self.len;
            }
        }
    }

    pub fn resize(&mut self, mut new_len: usize) {
        new_len = new_len.clamp(1, self.max);

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
    pub tree: sled::Tree,

    view_max: usize,
    view: Arc<AtomicCell<(usize, usize)>>,

    update: AtomicCell<bool>,

    pub width: usize,
    pub slots: Vec<PathViewSlot>,
    // slot_path_map: Vec<usize>,
    sample_buf: Vec<(Node, usize)>,

    new_samples: AtomicCell<bool>,
}

impl PathViewer {
    const SLOT_MASK: [u8; 7] = *b"slot:02";

    pub fn new(
        db: &sled::Db,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        width: usize,
        slot_count: usize,
        name_prefix: &str,
        path_count: usize,
    ) -> Result<Self> {
        // db.drop_tree(b"path_viewer")?;
        let tree = db.open_tree(b"path_viewer")?;

        let mut slots = Vec::with_capacity(slot_count);

        let slot_count = slot_count.min(path_count);

        for i in 0..slot_count {
            let name = format!("{}_slot_{}", name_prefix, i);
            let slot = PathViewSlot::new(ctx, res, alloc, width, Some(&name))?;

            slots.push(slot);
        }

        let view = Arc::new(AtomicCell::new((0, slot_count)));
        let view_max = path_count;

        Ok(Self {
            tree,
            width,
            slots,
            view,
            view_max,

            sample_buf: Vec::new(),
            update: false.into(),
            new_samples: false.into(),
        })
    }

    pub fn should_update(&self) -> bool {
        let r = self.update.load();
        self.update.store(false);
        r
    }

    pub fn sample(&mut self, graph: &Waragraph, view: &ViewDiscrete1D) {
        graph.sample_node_lengths(self.width, view, &mut self.sample_buf);
        self.new_samples.store(true);
    }

    pub fn has_new_samples(&self) -> bool {
        self.new_samples.load()
    }

    pub fn scroll_up(&self) {
        let (o, l) = self.view.load();
        if o > 0 {
            let no = (o - 1).clamp(0, self.view_max - l);
            self.view.store((no, l));
            self.update.store(true);
        }
    }

    pub fn scroll_down(&self) {
        let (o, l) = self.view.load();
        let no = (o + 1).clamp(0, self.view_max - l);
        self.view.store((no, l));
        self.update.store(true);
    }

    pub fn update_from_alt(
        &mut self,
        res: &mut GpuResources,
        updater: &SlotUpdateFn<u32>,
    ) -> Option<()> {
        let vis = self.visible_indices();

        let samples = &self.sample_buf;

        for (path, slot) in vis.zip(self.slots.iter_mut()) {
            slot.update_from(res, |ix| updater(samples, path, ix));
        }

        self.new_samples.store(false);

        Some(())
    }

    pub fn update_from<F>(
        &mut self,
        res: &mut GpuResources,
        mut fill: F,
    ) -> Option<()>
    where
        F: FnMut(usize, usize) -> u32,
    {
        let vis = self.visible_indices();

        for (path, slot) in vis.zip(self.slots.iter_mut()) {
            slot.update_from(res, |ix| fill(path, ix));
        }

        Some(())
    }

    pub fn update_labels(
        &self,
        graph: &Waragraph,
        txt: &LabelStorage,
    ) -> Result<()> {
        let x = 34u32;
        let y = 40u32;
        let yd = 66u32;

        for (ix, path_i) in self.visible_indices().enumerate() {
            let path_name = &graph.path_names[path_i];

            let txt_key = format!("path-name-{}", ix);

            let name = format!("{}", path_name.as_bstr());
            txt.set_text_for(txt_key.as_bytes(), &name)?;
            txt.set_label_pos(txt_key.as_bytes(), x, y + yd * ix as u32)?;
        }
        Ok(())
    }

    pub fn visible_indices(&self) -> std::ops::Range<usize> {
        let (offset, len) = self.view.load();
        offset..offset + len
    }

    pub fn list_offset(&self) -> Result<usize> {
        let o_bs = self.tree.get("list_offset")?.unwrap();
        let offset = usize::read_from(o_bs.as_ref()).unwrap();
        Ok(offset)
    }

    pub fn list_view_len(&self) -> Result<usize> {
        let o_bs = self.tree.get("list_view_len")?.unwrap();
        let view_len = usize::read_from(o_bs.as_ref()).unwrap();
        Ok(view_len)
    }
    pub fn list_max(&self) -> Result<usize> {
        let o_bs = self.tree.get("list_max")?.unwrap();
        let max = usize::read_from(o_bs.as_ref()).unwrap();
        Ok(max)
    }

    pub fn resize(
        &mut self,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        new_width: usize,
        fill: u32,
    ) -> Result<()> {
        for slot in self.slots.iter_mut() {
            slot.resize(ctx, res, alloc, new_width, fill)?;
        }
        self.width = new_width;
        Ok(())
    }
}
