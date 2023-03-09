use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
    sync::Arc,
};

use tokio::task::JoinHandle;
use waragraph_core::graph::{Bp, PathId, PathIndex};

use anyhow::Result;

use crate::{app::resource::GraphDataCache, util::BufferDesc};

use super::view::View1D;

// TODO: still using string-typed keys for data sources and viz. modes! bad!
pub type SlotKey = (PathId, String);

#[derive(
    Clone, Copy, PartialEq, PartialOrd, bytemuck::Zeroable, bytemuck::Pod,
)]
#[repr(C)]
pub struct SlotVertex {
    position: [f32; 2],
    size: [f32; 2],
    slot_id: u32,
}

type SlotTaskHandle = JoinHandle<Result<([Bp; 2], Vec<u8>)>>;

#[derive(Default)]
struct SlotState {
    last_updated_view: Option<[Bp; 2]>,
    task_handle: Option<SlotTaskHandle>,
}

impl SlotState {
    fn task_ready(&self) -> bool {
        if let Some(handle) = self.task_handle.as_ref() {
            handle.is_finished()
        } else {
            false
        }
    }

    fn task_results(
        &mut self,
        rt: &tokio::runtime::Handle,
    ) -> Option<([Bp; 2], Vec<u8>)> {
        let handle = self.task_handle.take()?;
        if !handle.is_finished() {
            self.task_handle = Some(handle);
            return None;
        }

        rt.block_on(handle).ok()?.ok()
    }
}

// encapsulates both the GPU buffer side and the task scheduling
// pub struct SlotCache<K: Ord> {
pub struct SlotCache {
    last_dispatched_view: Option<[Bp; 2]>,
    last_received_update_view: Option<[Bp; 2]>,
    slot_state: HashMap<SlotKey, SlotState>,

    // indexed by SlotId
    slot_id_cache: Vec<Option<(SlotKey, u64)>>,
    // map to SlotId
    slot_id_map: HashMap<SlotKey, usize>,
    slot_id_generation: u64,

    pub data_buffer: BufferDesc,
    rows: usize,

    bin_count: usize,

    pub vertex_buffer: Option<BufferDesc>,
    pub vertex_count: usize,

    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,
}

impl SlotCache {
    pub fn new(
        state: &raving_wgpu::State,
        path_index: Arc<PathIndex>,
        data_cache: Arc<GraphDataCache>,
        row_count: usize,
        bin_count: usize,
    ) -> Result<Self> {
        let data_buffer =
            Self::allocate_data_buffer(state, row_count, bin_count)?;

        let slot_id_cache = vec![None; row_count];
        let slot_id_map = HashMap::default();

        Ok(Self {
            last_dispatched_view: None,
            last_received_update_view: None,

            slot_state: HashMap::default(),
            slot_id_cache,
            slot_id_map,
            slot_id_generation: 0,

            data_buffer,
            rows: row_count,

            bin_count,

            vertex_buffer: None,
            vertex_count: 0,
            path_index,
            data_cache,
        })
    }

    // returns the view transform, based on the last dispatched view
    // and the current view, for use in a fragment uniform buffer
    pub fn get_view_transform(&self, current_view: &View1D) -> [f32; 2] {
        if let Some(last_view) = self.last_dispatched_view {
            let [l0, r0] = last_view;
            let view0 = (l0.0)..(r0.0);
            let view1 = current_view.range();
            super::Viewer1D::sample_index_transform(&view0, view1)
        } else {
            [1.0, 0.0]
        }
    }

    pub fn sample_and_update<I>(
        &mut self,
        state: &raving_wgpu::State,
        rt: &tokio::runtime::Handle,
        view: &View1D,
        layout: I,
    ) -> Result<()>
    where
        I: IntoIterator<Item = (SlotKey, egui::Rect)>,
    {
        let vl = view.range().start;
        let vr = view.range().end;
        let cview = [Bp(vl), Bp(vr)];

        let layout = layout.into_iter().collect::<HashMap<_, _>>();

        {
            let mut active = 0;
            for state in self.slot_state.values() {
                if state.task_handle.is_some() {
                    active += 1;
                }
            }
            log::warn!("# of active tasks before update: {active}");
            log::warn!("last dispatched view: {:?}", self.last_dispatched_view);
        }

        // TODO: reallocate data buffer if `layout` contains more
        // rows than are available

        {
            let active_count = layout.len() + self.rows / 8;
            let oldest_gen = self
                .slot_id_generation
                .checked_sub(active_count as u64)
                .unwrap_or(0);

            let mut available_slot_ids = self
                .slot_id_cache
                .iter()
                .enumerate()
                .filter_map(|(ix, entry)| {
                    if let Some(entry) = entry {
                        let is_active = layout.contains_key(&entry.0);
                        let is_old = entry.1 < oldest_gen;

                        (!is_active && is_old).then_some(ix)
                    } else {
                        Some(ix)
                    }
                })
                .collect::<Vec<_>>();

            let mut next_slot_id = available_slot_ids.into_iter();

            // iterates over cache entries that are not used by the input layout
            // and are old enough to be cleared
            // let mut cache_iter = slot_ids_by_gen.into_iter();

            // the slots in the layout are the ones we really care about,
            // but we can't just throw away what we have in case the user
            // scrolls down and back up, for example
            //
            // this is also where we assign the slot IDs for each slot in the layout
            for (key, rect) in layout.iter() {
                let state = self.slot_state.entry(key.clone()).or_default();

                // assign slot ID
                if let Some(slot_id) = self.slot_id_map.get(key) {
                    // todo!();
                    let entry = self
                        .slot_id_cache
                        .get_mut(*slot_id)
                        .and_then(|e| e.as_mut());
                    if let Some(entry) = entry {
                        let new_gen = self.slot_id_generation;
                        self.slot_id_generation += 1;
                        entry.1 = new_gen;
                    } else {
                        // this should never happen, but
                        let new_gen = self.slot_id_generation;
                        self.slot_id_generation += 1;
                        self.slot_id_cache[*slot_id] =
                            Some((key.clone(), new_gen));
                    }

                    // if self.slot_id_cache[*slot_id].is_none() {
                    // }
                    // ensure the cache actually is assigned to the correct slot key
                    debug_assert_eq!(
                        self.slot_id_cache.get(*slot_id).and_then(|k| {
                            let (key, _gen) = k.as_ref()?;
                            Some(key)
                        }),
                        Some(key)
                    );
                } else {
                    let new_gen = self.slot_id_generation;
                    self.slot_id_generation += 1;

                    // find the first available slot
                    let slot_id = next_slot_id.next().unwrap();
                    // let (slot_id, cache_entry) = cache_iter.next().unwrap();

                    if let Some((old_key, _old_gen)) =
                        self.slot_id_cache.get(slot_id).and_then(|e| e.as_ref())
                    {
                        // make sure the old slot key is unassigned in the map
                        self.slot_id_map.remove(old_key);
                    }

                    // update the slot key -> slot ID map in the cache
                    self.slot_id_cache[slot_id] = Some((key.clone(), new_gen));
                    self.slot_id_map.insert(key.clone(), slot_id);
                    // self.slot_id_map.remove(&cache_entry.0);
                }

                if state.task_handle.is_some() {
                    continue;
                }

                if state.last_updated_view != Some(cview) {
                    let data_cache = self.data_cache.clone();
                    let bin_count = self.bin_count;
                    let path_index = self.path_index.clone();

                    let task = rt.spawn(Self::slot_task(
                        path_index,
                        data_cache,
                        bin_count,
                        key.clone(),
                        cview,
                    ));
                    state.task_handle = Some(task);
                }
            }
        }

        self.last_dispatched_view = Some(cview);

        // for each slot with a finished task, if the task contains data
        // for the correct view, find the first row in the data buffer that
        // has is not mapped to a slot key that has been used for a while
        // (or use the slot ID if already in the cache)

        {
            let mut slot_index = 0usize;
            for (key, slot_state) in self.slot_state.iter_mut() {
                if let Some((task_view, data)) = slot_state.task_results(rt) {
                    if Some(task_view) != self.last_dispatched_view {
                        // out of date, discarding
                        continue;
                    }

                    // get the slot ID, which is assigned on task dispatch above
                    let slot_id = if let Some(id) = self.slot_id_map.get(key) {
                        *id
                    } else {
                        unreachable!();
                    };

                    // the slot_id can be used to derive the slice in the buffer
                    // that this slot key is bound to
                    let prefix_size = std::mem::size_of::<[u32; 4]>();
                    let offset = prefix_size + slot_id * self.bin_count;

                    let size =
                        std::num::NonZeroU64::new(4 * self.bin_count as u64)
                            .unwrap();
                    let mut write_view = state.queue.write_buffer_with(
                        &self.data_buffer.buffer,
                        offset as u64,
                        size,
                    );

                    slot_state.last_updated_view = Some(task_view);

                    // schedule an update to the corresponding row
                    write_view.copy_from_slice(&data);
                }
            }
        }

        let mut vertices: Vec<SlotVertex> = Vec::new();

        // add a vertex for each slot in the layout that has an up to date
        // row in the data buffer
        for (key, rect) in layout {
            if let Some(state) = self.slot_state.get(&key) {
                let last_dispatch = self.last_dispatched_view;
                let last_update = state.last_updated_view;

                if last_update == last_dispatch && last_dispatch.is_some() {
                    let slot_id = *self
                        .slot_id_map
                        .get(&key)
                        .expect("Slot was ready but unbound!");

                    let vx = SlotVertex {
                        position: [rect.left(), rect.bottom()],
                        size: [rect.width(), rect.height()],
                        slot_id: slot_id as u32,
                    };

                    vertices.push(vx);
                }
            }
        }

        {
            let mut active = 0;
            for state in self.slot_state.values() {
                if state.task_handle.is_some() {
                    active += 1;
                }
            }
            log::warn!("# of active tasks after update: {active}");
        }

        // update the vertex buffer, reallocating if needed
        self.prepare_vertex_buffer(state, &vertices)?;
        self.vertex_count = vertices.len();

        Ok(())
    }

    pub fn total_data_buffer_size(&self) -> usize {
        let prefix_size = std::mem::size_of::<[u32; 4]>();
        prefix_size + self.rows * self.bin_count
    }

    // pub fn render_slots(&mut self) -> (std::ops::Range<u32>, Vec<egui::Shape>) {
    //     todo!();
    // }
}

impl SlotCache {
    fn prepare_vertex_buffer(
        &mut self,
        state: &raving_wgpu::State,
        vertices: &[SlotVertex],
    ) -> Result<()> {
        // reallocate vertex buffer if needed
        let vx_count = vertices.len();
        let vx_stride = std::mem::size_of::<SlotVertex>();

        let need_realloc = if let Some(buf) = self.vertex_buffer.as_ref() {
            buf.size < vertices.len() * vx_stride
        } else {
            true
        };

        if need_realloc {
            let usage =
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST;

            let buf_size = vx_stride * vertices.len().next_power_of_two();

            let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Slot Cache Vertex Buffer"),
                usage,
                size: buf_size as u64,
                mapped_at_creation: false,
            });

            self.vertex_buffer = Some(BufferDesc {
                buffer,
                size: buf_size,
            });
        }

        // fill the vertex buffer
        if let Some(buf) = self.vertex_buffer.as_ref() {
            state.queue.write_buffer(
                &buf.buffer,
                0,
                bytemuck::cast_slice(vertices),
            );
        } else {
            unreachable!();
        }

        Ok(())
    }

    fn allocate_data_buffer(
        state: &raving_wgpu::State,
        row_count: usize,
        bin_count: usize,
    ) -> Result<BufferDesc> {
        let rows = row_count as u32;
        let cols = bin_count as u32;
        let prefix = [rows * cols, cols, !0u32, !0u32];

        let prefix_size = std::mem::size_of::<[u32; 4]>();
        let size = prefix_size + row_count * bin_count;

        let usage = wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::STORAGE;

        let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Viewer 1D Data Buffer"),
            size: size as u64,
            usage,
            mapped_at_creation: false,
        });

        state
            .queue
            .write_buffer(&buffer, 0, bytemuck::cast_slice(&prefix));

        Ok(BufferDesc { buffer, size })
    }

    async fn slot_task(
        path_index: Arc<PathIndex>,
        data_cache: Arc<GraphDataCache>,
        bin_count: usize,
        key: SlotKey,
        view: [Bp; 2],
    ) -> Result<([Bp; 2], Vec<u8>)> {
        use waragraph_core::graph::sampling;

        let (path, data_key) = key;

        // load data source into cache & get data
        let data = data_cache.fetch_path_data(&data_key, path).await?;

        // sample data into vector
        let sample_vec = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; bin_count * 4];

            let l = view[0].0;
            let r = view[1].0;

            sampling::sample_data_into_buffer(
                &path_index,
                path,
                &data.path_data,
                l..r,
                bytemuck::cast_slice_mut(&mut buf),
            );

            buf
        })
        .await?;

        Ok((view, sample_vec))
    }
}
