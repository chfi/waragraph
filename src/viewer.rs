use std::collections::{BTreeMap, HashMap};

use ash::vk;
use bstr::ByteSlice;
use gpu_allocator::vulkan::Allocator;
use parking_lot::RwLock;
use raving::vk::{
    context::VkContext, descriptor::DescriptorLayoutInfo, BufferIx, BufferRes,
    DescSetIx, GpuResources, VkEngine,
};
use rspirv_reflect::DescriptorInfo;

use sled::IVec;
use zerocopy::{AsBytes, FromBytes};

use anyhow::{anyhow, Result};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

pub mod app;
pub mod gui;

pub mod sampler;
pub mod slots;

pub use sampler::*;
pub use slots::*;

use crate::{
    graph::{Node, Path, Waragraph},
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

    pub row_max: usize,
    pub row_view: Arc<AtomicCell<(usize, usize)>>,

    update: Arc<AtomicCell<bool>>,

    pub width: usize,

    pub slots: Arc<RwLock<SlotCache>>,

    // pub slots: Vec<PathViewSlot>,
    // slot_cache: HashMap<(usize, (usize, usize), IVec), usize>,
    sample_buf: Vec<(Node, usize)>,

    new_samples: AtomicCell<bool>,
}

impl PathViewer {
    const SLOT_MASK: [u8; 7] = *b"slot:02";

    pub fn new(
        engine: &mut VkEngine,
        db: &sled::Db,
        labels: &mut LabelStorage,
        width: usize,
        slot_count: usize,
        path_count: usize,
    ) -> Result<Self> {
        let tree = db.open_tree(b"path_viewer")?;

        let slot_count = slot_count.min(path_count);

        let mut slots = SlotCache::default();
        for _ in 0..slot_count {
            slots.allocate_slot(engine, db, labels, width)?;
        }

        let row_view = Arc::new(AtomicCell::new((0, slot_count)));
        let row_max = path_count;

        let mut result = Self {
            tree,
            width,

            slots: Arc::new(RwLock::new(slots)),

            row_view,
            row_max,

            sample_buf: Vec::new(),
            update: Arc::new(true.into()),
            new_samples: false.into(),
        };

        Ok(result)
    }

    pub fn force_update_fn(&self) -> Arc<dyn Fn() + Send + Sync + 'static> {
        let update = self.update.clone();
        Arc::new(move || update.store(true))
    }

    pub fn force_update(&self) {
        self.update.store(true);
    }

    pub fn force_update_cell(&self) -> &Arc<AtomicCell<bool>> {
        &self.update
    }

    pub fn should_update(&self) -> bool {
        let r = self.update.load();
        self.update.store(false);
        r
    }

    pub fn sample(&mut self, graph: &Waragraph, view: &ViewDiscrete1D) {
        if self.width > 0 {
            graph.sample_node_lengths(self.width, view, &mut self.sample_buf);
            self.new_samples.store(true);
        }
    }

    pub fn has_new_samples(&self) -> bool {
        self.new_samples.load()
    }

    pub fn scroll_up(&self) {
        let (o, l) = self.row_view.load();
        if o > 0 {
            let no = (o - 1).clamp(0, self.row_max - l);
            self.row_view.store((no, l));
            self.update.store(true);
        }
    }

    pub fn scroll_down(&self) {
        let (o, l) = self.row_view.load();
        let no = (o + 1).clamp(0, self.row_max - l);
        self.row_view.store((no, l));
        self.update.store(true);
    }

    /*
    pub fn update_slots(
        &mut self,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        updater: &SlotUpdateFn<u32>,
    ) -> Option<()> {

        // let paths = self.slot_cache.

        Some(())
    }
    */

    pub fn apply_update(
        &mut self,
        res: &mut GpuResources,
        slot_fn: rhai::ImmutableString,
        slot_ix: usize,
        data: &[u32],
        view: (usize, usize),
        width: usize,
    ) -> Option<()> {
        if let Some(slot) = self.slots.write().slots.get_mut(slot_ix) {
            if slot.width == Some(width) {
                // if slot.view == Some(view) && slot.width == Some(width) {
                slot.slot.fill_from(res, data)?;
                slot.updating.store(false);
                if slot.slot_function != &slot_fn {
                    slot.slot_function = slot_fn.to_owned();
                }
            }
        }
        Some(())
    }

    pub fn update_from(
        &mut self,
        res: &mut GpuResources,
        graph: &Arc<Waragraph>,
        updater: &SlotUpdateFn<u32>,
        view: ViewDiscrete1D,
    ) -> Option<()> {
        let paths = self.visible_paths(graph);
        let samples = &self.sample_buf;

        let mut buffer = Vec::new();

        let cur_view = Some((view.offset, view.len));

        for path in paths {
            if let Some(slot) = self.slots.write().get_slot_mut_for(path) {
                if slot.view != cur_view || slot.width != Some(self.width) {
                    buffer.clear();
                    buffer.extend(
                        (0..self.width).map(|i| updater(samples, path, i)),
                    );
                    slot.slot.fill_from(res, &buffer)?;
                    slot.view = cur_view;
                    slot.width = Some(self.width);
                }
            }
        }

        self.new_samples.store(false);

        Some(())
    }

    pub fn update_labels(
        &self,
        graph: &Waragraph,
        txt: &LabelStorage,
        offset: [u32; 2],
        y_delta: u32,
        max_len: u8,
    ) -> Result<()> {
        let [x, y] = offset;
        let yd = y_delta;
        let max_len = max_len as usize;

        for (ix, path) in self.visible_paths(graph).enumerate() {
            if let Some(path_name) = graph.path_names.get_by_left(&path) {
                if let Some(slot) = self.slots.read().get_slot_for(path) {
                    let label_id = slot.label_id;

                    if path_name.len() < max_len {
                        let name = format!("{}", path_name.as_bstr());
                        txt.set_text_for_id(label_id, &name)?;
                    } else {
                        let prefix = path_name[..max_len - 3].as_bstr();
                        let name = format!("{}...", prefix);
                        txt.set_text_for_id(label_id, &name)?;
                    }

                    txt.set_pos_for_id(label_id, x, y + yd * ix as u32)?;
                }
            }
        }
        Ok(())
    }

    pub fn visible_paths(
        &self,
        graph: &Waragraph,
    ) -> impl Iterator<Item = Path> {
        let (offset, len) = self.row_view.load();
        (offset..offset + len).map(|i| Path::from(i))
    }

    // pub fn visible_indices(&self) -> std::ops::Range<usize> {
    //     let (offset, len) = self.row_view.load();
    //     offset..offset + len
    // }

    pub fn resize(
        &mut self,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
        new_width: usize,
        fill: u32,
    ) -> Result<()> {
        for slot in self.slots.write().slots.iter_mut() {
            slot.slot.resize(ctx, res, alloc, new_width, fill)?;
        }
        self.width = new_width;
        Ok(())
    }
}
