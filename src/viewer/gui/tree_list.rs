use bstr::ByteSlice;
use crossbeam::atomic::AtomicCell;
use parking_lot::RwLock;
use raving::script::console::frame::FrameBuilder;
use raving::script::console::BatchBuilder;
use raving::vk::{
    BatchInput, BufferIx, DescSetIx, FrameResources, FramebufferIx,
    GpuResources, PipelineIx, RenderPassIx, VkEngine,
};

use raving::vk::resource::WindowResources;

use ash::{vk, Device};

use rhai::plugin::RhaiResult;
use rustc_hash::{FxHashMap, FxHashSet};
use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::config::ConfigMap;
use crate::console::{RhaiBatchFn2, RhaiBatchFn4, RhaiBatchFn5};
use crate::graph::{Node, Waragraph};
use crate::util::{BufFmt, BufId, BufferStorage, LabelStorage};
use crate::viewer::{SlotRenderers, ViewDiscrete1D};

use std::collections::{BTreeMap, HashMap};

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use zerocopy::{AsBytes, FromBytes};

use rhai::plugin::*;

#[derive(Clone)]
pub struct LabelSpace {
    name: rhai::ImmutableString,

    offsets: BTreeMap<rhai::ImmutableString, (usize, usize)>,

    text: Vec<u8>,

    capacity: usize,
    used_bytes: usize,

    pub text_buffer: BufferIx,
    pub text_set: DescSetIx,
}

impl LabelSpace {
    pub fn new(
        engine: &mut VkEngine,
        name: &str,
        capacity: usize,
    ) -> Result<Self> {
        let name = format!("label-space:{}", name);

        let (text_buffer, text_set) =
            engine.with_allocators(|ctx, res, alloc| {
                let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
                let usage = vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST;

                let buffer = res.allocate_buffer(
                    ctx,
                    alloc,
                    mem_loc,
                    4,
                    capacity / 4,
                    usage,
                    Some(&name),
                )?;

                let buf_ix = res.insert_buffer(buffer);

                let desc_set =
                    crate::util::allocate_buffer_desc_set(buf_ix, res)?;

                let set_ix = res.insert_desc_set(desc_set);

                Ok((buf_ix, set_ix))
            })?;

        Ok(Self {
            name: name.into(),

            offsets: BTreeMap::default(),
            text: Vec::new(),

            capacity,
            used_bytes: 0,

            text_buffer,
            text_set,
        })
    }

    pub fn write_buffer(&self, res: &mut GpuResources) -> Option<()> {
        if self.used_bytes == 0 {
            return Some(());
        }
        let buf = &mut res[self.text_buffer];
        let slice = buf.mapped_slice_mut()?;
        slice[0..self.used_bytes].clone_from_slice(&self.text);
        Some(())
    }

    pub fn bounds_for(&mut self, text: &str) -> Result<(usize, usize)> {
        if let Some(bounds) = self.offsets.get(text) {
            return Ok(*bounds);
        }

        let offset = self.used_bytes;
        let len = text.as_bytes().len();

        if self.used_bytes + len > self.capacity {
            anyhow::bail!("Label space out of memory");
        }

        let bounds = (offset, len);

        self.text.extend(text.as_bytes());
        self.offsets.insert(text.into(), bounds);

        self.used_bytes += len;

        Ok(bounds)
    }
}

pub struct TreeList {
    // list: Vec<rhai::ImmutableString>,
    list: Vec<rhai::Dynamic>,

    rects: super::RectVertices,
    // labels:

    // layers: Vec<Vec<()>>,
    text_buffers: Vec<BufferIx>,
    text_sets: Vec<DescSetIx>,

    rhai_module: Arc<rhai::Module>,
}

impl TreeList {
    pub fn new(engine: &mut VkEngine) -> Result<Self> {
        todo!();
    }
}

#[export_module]
pub mod rhai_module {
    //
}
