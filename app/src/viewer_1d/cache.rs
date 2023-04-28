use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::RangeInclusive,
    sync::Arc,
};

use tokio::{task::JoinHandle, time::Instant};
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

type SlotTaskHandle = JoinHandle<Result<([Bp; 2], Vec<u8>, u64)>>;

pub type SlotMsg = String;

#[derive(Default)]
pub struct SlotState {
    pub data_generation: Option<u64>,
    pub updated_at: Option<Instant>,
    pub last_updated_view: Option<[Bp; 2]>,
    task_handle: Option<SlotTaskHandle>,
    pub last_msg: Option<SlotMsg>,
    pub last_rect: Option<egui::Rect>,
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
    ) -> Option<([Bp; 2], Vec<u8>, u64)> {
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
    slot_state: HashMap<SlotKey, SlotState>,

    // indexed by SlotId
    slot_id_cache: Vec<Option<SlotKey>>,
    // map to SlotId
    slot_id_map: HashMap<SlotKey, usize>,
    slot_id_generation: u64,

    pub data_buffer: BufferDesc,
    rows: usize,

    bin_count: usize,

    pub vertex_buffer: Option<BufferDesc>,
    pub vertex_count: usize,
    pub transform_buffer: Option<BufferDesc>,

    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,

    slot_msg_rx: crossbeam::channel::Receiver<(SlotKey, SlotMsg)>,
    slot_msg_tx: crossbeam::channel::Sender<(SlotKey, SlotMsg)>,

    pub(super) msg_shapes: Vec<egui::Shape>,

    pub(super) last_update: Option<std::time::Instant>,
    generation: u64,
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

        let (slot_msg_tx, slot_msg_rx) = crossbeam::channel::unbounded();

        Ok(Self {
            last_dispatched_view: None,

            slot_state: HashMap::default(),
            slot_id_cache,
            slot_id_map,
            slot_id_generation: 0,

            data_buffer,
            rows: row_count,

            bin_count,

            vertex_buffer: None,
            vertex_count: 0,
            transform_buffer: None,

            path_index,
            data_cache,

            slot_msg_tx,
            slot_msg_rx,

            msg_shapes: Vec::new(),
            last_update: None,
            generation: 0,
        })
    }

    pub(super) fn debug_window(&self, egui_ctx: &egui::Context) {
        let entry_uis = |ui: &mut egui::Ui, slot: usize, key: &SlotKey| {
            if let Some(state) = self.slot_state.get(key) {
                let (path, data) = key;

                let path_name =
                    self.path_index.path_names.get_by_left(&path).unwrap();

                let running = if state.task_handle.is_some() {
                    "Running"
                } else {
                    "N/A"
                };

                let key = format!("{path_name} - [{running}]");
                ui.label(&format!("slot {slot} - {key}"));
                ui.separator();
            }
        };

        egui::Window::new("Slot Cache Debug").show(egui_ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical(|ui| {
                    for (ix, entry) in self.slot_id_cache.iter().enumerate() {
                        if let Some(key) = entry.as_ref() {
                            entry_uis(ui, ix, key);
                        }
                    }
                });
            });
        });
    }

    // returns the view transform, based on the last dispatched view
    // and the current view, for use in a fragment uniform buffer
    pub fn get_view_transform(&self, current_view: &View1D) -> [f32; 2] {
        Self::view_transform(self.last_dispatched_view, current_view)
    }

    fn view_transform(
        last_view: Option<[Bp; 2]>,
        current_view: &View1D,
    ) -> [f32; 2] {
        if let Some(last_view) = last_view {
            let [l0, r0] = last_view;
            let view0 = (l0.0)..(r0.0);
            let view1 = current_view.range();
            super::Viewer1D::sample_index_transform(&view0, view1)
        } else {
            [1.0, 0.0]
        }
    }

    pub fn sample_for_data(
        &mut self,
        state: &raving_wgpu::State,
        rt: &tokio::runtime::Handle,
        view: &View1D,
        data_key: &str,
        paths: impl IntoIterator<Item = PathId>,
    ) -> Result<()> {
        let vl = view.range().start;
        let vr = view.range().end;
        let current_view = [Bp(vl), Bp(vr)];

        // spawn tasks for each of the out-of-date paths in the
        // iterator (based on the current view)

        // TODO limit the task spawn rate by storing the Instant each
        // slot was last updated

        // first i need to assign slot IDs/indices to the SlotKeys,
        // which are created from the paths + the shared data_key

        let slots = paths
            .into_iter()
            .map(|path| (path, data_key.to_string()))
            .collect::<Vec<_>>();

        let result = self.assign_rows_for_slots(slots.iter(), current_view);

        if let Err(SlotCacheError::OutOfRows) = result {
            // TODO reallocate
            log::error!("Slot cache full! TODO reallocate");
        }

        for slot_key in &slots {
            let state = if let Some(state) = self.slot_state.get_mut(slot_key) {
                state
            } else {
                log::warn!(
                    "Slot key (Path {}, {}) missing state",
                    slot_key.0.ix(),
                    slot_key.1
                );
                continue;
            };

            if state.task_handle.is_some()
                || state.last_updated_view == Some(current_view)
            {
                continue;
            }

            if let Some(time_since_update) =
                state.updated_at.map(|s| s.elapsed())
            {
                if time_since_update.as_secs_f32() < 0.1 {
                    continue;
                }
            }

            let data_cache = self.data_cache.clone();
            let bin_count = self.bin_count;
            let path_index = self.path_index.clone();

            let task = rt.spawn(Self::slot_task(
                self.slot_msg_tx.clone(),
                self.generation,
                path_index,
                data_cache,
                bin_count,
                slot_key.clone(),
                current_view,
            ));
            state.task_handle = Some(task);
        }

        self.last_dispatched_view = Some(current_view);

        // update messages on slots
        while let Ok((key, msg)) = self.slot_msg_rx.try_recv() {
            if let Some(state) = self.slot_state.get_mut(&key) {
                state.last_msg = Some(msg);
            }
        }

        Ok(())
    }

    pub fn update(
        &mut self,
        state: &raving_wgpu::State,
        rt: &tokio::runtime::Handle,
        view: &View1D,
        slot_rects: &HashMap<SlotKey, egui::Rect>,
    ) -> Result<()> {
        // queue updates from completed tasks to be uploaded to the GPU
        for (slot_key, slot_state) in self.slot_state.iter_mut() {
            if let Some((task_view, data, data_ts)) =
                slot_state.task_results(rt)
            {
                //
                if slot_state
                    .data_generation
                    .map(|prev_ts| prev_ts > data_ts)
                    .unwrap_or(false)
                {
                    continue;
                }

                let slot_id = if let Some(id) = self.slot_id_map.get(slot_key) {
                    *id
                } else {
                    continue;
                };

                let prefix_size = std::mem::size_of::<[u32; 2]>();
                let elem_size = std::mem::size_of::<f32>();
                let offset = prefix_size + slot_id * self.bin_count * elem_size;

                let size = std::num::NonZeroU64::new(
                    (elem_size * self.bin_count) as u64,
                )
                .unwrap();
                let mut write_view = state.queue.write_buffer_with(
                    &self.data_buffer.buffer,
                    offset as u64,
                    size,
                );
                log::debug!(
                    "writing buffer slot ({}, {}) -> {slot_id}",
                    slot_key.0.ix(),
                    slot_key.1
                );

                slot_state.last_updated_view = Some(task_view);
                slot_state.data_generation = Some(data_ts);
                slot_state.updated_at = Some(Instant::now());

                // schedule an update to the corresponding row
                write_view.copy_from_slice(&data);
            }
        }

        // create vertices for the slots that contain data
        let mut vertices: Vec<SlotVertex> = Vec::new();

        for (slot_key, rect) in slot_rects {
            let (state, &id) = {
                let state = self.slot_state.get(slot_key);
                let id = self.slot_id_map.get(slot_key);
                let state_id = state.zip(id);

                if state_id.is_none() {
                    continue;
                }

                state_id.unwrap()
            };

            if state.data_generation.is_none() {
                continue;
            }

            let vx = SlotVertex {
                position: [rect.left(), rect.bottom()],
                size: [rect.width(), rect.height()],
                slot_id: id as u32,
            };

            vertices.push(vx);
        }

        self.vertex_count = vertices.len();

        self.prepare_vertex_buffer(state, &vertices)?;
        self.prepare_transform_buffer(state, &vertices, view)?;

        Ok(())
    }

    fn assign_rows_for_slots<'a>(
        &mut self,
        slots: impl Iterator<Item = &'a SlotKey>,
        current_view: [Bp; 2],
    ) -> std::result::Result<(), SlotCacheError> {
        // evict from cache based on last updated *time*, as in Instant

        // let time_since_update = self.last_update

        // any row without a
        let mut free_rows = self
            .slot_id_cache
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(ix, key)| key.is_some().then_some(ix))
            .collect::<Vec<_>>();

        let mut eviction_cands: Vec<(SlotKey, tokio::time::Duration)> = self
            .slot_state
            .iter()
            .filter_map(|(slot_key, state)| {
                if state.task_handle.is_some() {
                    return None;
                }

                if let Some(view) = state.last_updated_view {
                    if view == current_view {
                        return None;
                    }
                }

                let updated_at = state.updated_at?;
                let time_since = updated_at.elapsed();
                Some((slot_key.clone(), time_since))
            })
            .collect();

        // sort so oldest are last
        eviction_cands.sort_by_key(|(_, dur)| *dur);

        for slot_key in slots {
            if let Some(row_id) = free_rows.pop() {
                // use the free row, no eviction needed

                let new_state = SlotState::default();
                self.slot_id_cache[row_id] = Some(slot_key.clone());
                self.slot_id_map.insert(slot_key.clone(), row_id);
                self.slot_state.insert(slot_key.clone(), new_state);

                continue;
            }

            // otherwise try to evict an old row

            let candidate = eviction_cands.pop();
            let row_id = candidate
                .as_ref()
                .and_then(|(slot_key, _)| self.slot_id_map.get(slot_key))
                .copied();

            if let Some(((old_slot_key, _dur), row_id)) = candidate.zip(row_id)
            {
                let _old_state = self.slot_state.remove(&old_slot_key);
                self.slot_id_map.remove(&old_slot_key);

                let new_state = SlotState::default();
                self.slot_id_cache[row_id] = Some(slot_key.clone());
                self.slot_id_map.insert(slot_key.clone(), row_id);
                self.slot_state.insert(slot_key.clone(), new_state);

                continue;
            }

            // by this point we can't find a row for this slot, so return with an error
            return Err(SlotCacheError::OutOfRows);
        }

        Ok(())
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

        let should_update = self
            .last_update
            .map(|t| t.elapsed().as_micros() < 2_00_000)
            .unwrap_or(true);

        // TODO: this seems to break things (no sampling happens) if
        // the first frame takes too long, in particular if
        // annotations are used
        // if !should_update {
        //     return Ok(());
        // }

        self.last_update = Some(std::time::Instant::now());

        let mut vertices: Vec<SlotVertex> = Vec::new();

        let mut slot_id_count: HashMap<usize, usize> = HashMap::new();

        // add a vertex for each slot in the layout that has an up to date
        // row in the data buffer
        for (key, rect) in layout.iter().clone() {
            if let Some(state) = self.slot_state.get(&key) {
                // let last_dispatch = self.last_dispatched_view;
                let last_update = state.last_updated_view;

                // if last_update == last_dispatch && last_dispatch.is_some() {
                if last_update.is_some() {
                    let slot_id = *self
                        .slot_id_map
                        .get(&key)
                        .expect("Slot was ready but unbound!");

                    let vx = SlotVertex {
                        position: [rect.left(), rect.bottom()],
                        size: [rect.width(), rect.height()],
                        slot_id: slot_id as u32,
                    };

                    *slot_id_count.entry(slot_id).or_default() += 1;

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
        }

        // update the vertex buffer, reallocating if needed
        self.vertex_count = vertices.len();

        for state in self.slot_state.values_mut() {
            state.last_rect = None;
        }

        // TODO: reallocate data buffer if `layout` contains more
        // rows than are available

        {
            // simplifying so that slots are assigned to only the slot
            // keys used in the layout, rather than try to cache slots
            // when scrolling the list

            // or just store the last frame's set as well? does it matter

            let used_keys =
                layout.iter().map(|(k, _)| k).collect::<HashSet<_>>();

            let mut to_remove = Vec::new();

            for (key, slot_id) in self.slot_id_map.iter() {
                if used_keys.contains(key) {
                    continue;
                }

                to_remove.push((key.clone(), *slot_id));
            }

            for (key, slot_id) in to_remove {
                self.slot_id_map.remove(&key);
                self.slot_id_cache[slot_id] = None;

                self.slot_state.remove(&key);
            }

            let mut available_slot_ids = self
                .slot_id_cache
                .iter()
                .enumerate()
                .filter_map(|(ix, entry)| {
                    if let Some(key) = entry {
                        let is_active = layout.contains_key(key);
                        (!is_active).then_some(ix)
                    } else {
                        Some(ix)
                    }
                })
                .collect::<Vec<_>>();

            let mut next_slot_id = available_slot_ids.into_iter();

            for (key, rect) in layout.iter() {
                if let Some(slot_id) = self.slot_id_map.get(key) {
                    // if already assigned to a slot, don't need to do anything
                } else {
                    // we need to assign the slot,
                    // assume we can, though; no realloc yet/here

                    let slot_id = next_slot_id.next().unwrap();
                    // log::error!("allocating new slot!!! id: {slot_id}");
                    // let (slot_id, cache_entry) = cache_iter.next().unwrap();

                    if let Some(old_key) =
                        self.slot_id_cache.get(slot_id).and_then(|e| e.as_ref())
                    {
                        // make sure the old slot key is unassigned in the map
                        self.slot_id_map.remove(old_key);
                        self.slot_state.remove(old_key);
                    }

                    // update the slot key -> slot ID map in the cache
                    self.slot_id_cache[slot_id] = Some(key.clone());
                    self.slot_id_map.insert(key.clone(), slot_id);
                }

                let state = self.slot_state.entry(key.clone()).or_default();
                state.last_rect = Some(*rect);

                if state.task_handle.is_some() {
                    continue;
                }

                if state.last_updated_view == Some(cview) {
                    continue;
                }

                if state
                    .data_generation
                    .map(|prev_gen| self.generation.abs_diff(prev_gen) < 120)
                    .unwrap_or(false)
                {
                    // log::warn!("skipping new enough slot!");
                    continue;
                }

                let data_cache = self.data_cache.clone();
                let bin_count = self.bin_count;
                let path_index = self.path_index.clone();

                let task = rt.spawn(Self::slot_task(
                    self.slot_msg_tx.clone(),
                    self.generation,
                    path_index,
                    data_cache,
                    bin_count,
                    key.clone(),
                    cview,
                ));
                state.task_handle = Some(task);
            }
        }

        self.last_dispatched_view = Some(cview);

        // update messages on slots
        while let Ok((key, msg)) = self.slot_msg_rx.try_recv() {
            if let Some(state) = self.slot_state.get_mut(&key) {
                state.last_msg = Some(msg);
            }
        }

        // for each slot with a finished task, if the task contains data
        // for the correct view, find the first row in the data buffer that
        // has is not mapped to a slot key that has been used for a while
        // (or use the slot ID if already in the cache)

        {
            let mut slot_index = 0usize;

            for (key, slot_state) in self.slot_state.iter_mut() {
                if let Some((task_view, data, data_gen)) =
                    slot_state.task_results(rt)
                {
                    if slot_state
                        .data_generation
                        .map(|prev_gen| prev_gen > data_gen)
                        .unwrap_or(false)
                    {
                        continue;
                    }

                    // get the slot ID, which is assigned on task dispatch above
                    let slot_id = if let Some(id) = self.slot_id_map.get(key) {
                        *id
                    } else {
                        // just discard if the slot has been unmapped
                        // (e.g. the user scrolled far enough)
                        continue;
                    };

                    // the slot_id can be used to derive the slice in the buffer
                    // that this slot key is bound to
                    let prefix_size = std::mem::size_of::<[u32; 2]>();
                    let elem_size = std::mem::size_of::<f32>();
                    let offset =
                        prefix_size + slot_id * self.bin_count * elem_size;

                    let size = std::num::NonZeroU64::new(
                        (elem_size * self.bin_count) as u64,
                    )
                    .unwrap();
                    let mut write_view = state.queue.write_buffer_with(
                        &self.data_buffer.buffer,
                        offset as u64,
                        size,
                    );
                    log::debug!(
                        "writing buffer slot ({}, {}) -> {slot_id}",
                        key.0.ix(),
                        key.1
                    );

                    slot_state.last_updated_view = Some(task_view);
                    slot_state.data_generation = Some(data_gen);
                    slot_state.updated_at = Some(Instant::now());

                    // schedule an update to the corresponding row
                    write_view.copy_from_slice(&data);
                }
            }
        }

        self.prepare_vertex_buffer(state, &vertices)?;
        self.prepare_transform_buffer(state, &vertices, view)?;

        self.generation += 1;

        Ok(())
    }

    pub fn update_displayed_messages(
        &mut self,
        show_state: impl Fn(&SlotState) -> Option<egui::Shape>,
    ) {
        self.msg_shapes.clear();

        self.slot_state
            .values()
            .filter_map(|state| Some((state, show_state(state)?)))
            .for_each(|(state, shape)| self.msg_shapes.push(shape));
    }

    pub fn total_data_buffer_size(&self) -> usize {
        let prefix_size = std::mem::size_of::<[u32; 4]>();
        prefix_size + self.rows * self.bin_count
    }

    pub fn slot_task_running(&self, slot_id: usize) -> bool {
        self.slot_id_cache
            .get(slot_id)
            .and_then(|key| {
                let key = key.as_ref()?;
                let state = self.slot_state.get(key)?;
                Some(state.task_handle.is_some())
            })
            .unwrap_or(false)
    }
}

impl SlotCache {
    fn prepare_transform_buffer(
        &mut self,
        state: &raving_wgpu::State,
        vertices: &[SlotVertex],
        current_view: &View1D,
    ) -> Result<()> {
        // reallocate if needed
        let t_stride = std::mem::size_of::<[f32; 2]>();

        let need_realloc = if let Some(buf) = self.transform_buffer.as_ref() {
            buf.size < vertices.len() * t_stride
        } else {
            true
        };

        if need_realloc {
            let usage =
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

            let buf_size = t_stride * vertices.len().next_power_of_two();

            let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Slot Cache Fragment Transform Buffer"),
                usage,
                size: buf_size as u64,
                mapped_at_creation: false,
            });

            self.transform_buffer = Some(BufferDesc {
                buffer,
                size: buf_size,
            });
        }

        //
        if let Some(buf) = self.transform_buffer.as_ref() {
            let data = vertices
                .iter()
                .filter_map(|vx| {
                    let slot = vx.slot_id;
                    let key = self
                        .slot_id_cache
                        .get(slot as usize)
                        .and_then(|s| s.as_ref())?;

                    let last_update =
                        self.slot_state.get(key)?.last_updated_view;

                    let transform =
                        Self::view_transform(last_update, current_view);
                    Some(transform)
                })
                .collect::<Vec<_>>();

            // {
            //     let tdata: &[[f32; 2]] = bytemuck::cast_slice(&data);
            //     println!("transform: {tdata:?}");
            // }

            state.queue.write_buffer(
                &buf.buffer,
                0,
                bytemuck::cast_slice(&data),
            );

            debug_assert_eq!(data.len(), vertices.len());
            Ok(())
        } else {
            unreachable!();
        }
    }

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
        // let prefix = [rows * cols, cols, !0u32, !0u32];
        let prefix = [rows * cols, cols];

        let prefix_size = std::mem::size_of::<[u32; 2]>();
        let elem_size = std::mem::size_of::<f32>();
        let size = prefix_size + row_count * bin_count * elem_size;

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
        msg_tx: crossbeam::channel::Sender<(SlotKey, SlotMsg)>,
        generation: u64,
        path_index: Arc<PathIndex>,
        data_cache: Arc<GraphDataCache>,
        bin_count: usize,
        key: SlotKey,
        view: [Bp; 2],
    ) -> Result<([Bp; 2], Vec<u8>, u64)> {
        use waragraph_core::graph::sampling;

        let (path, data_key) = key.clone();

        let msg = format!(
            "Fetching data ({}{}), [{}, {}]",
            path.ix(),
            &data_key,
            view[0].0,
            view[1].0
        );
        let _ = msg_tx.try_send((key.clone(), msg));

        let t0 = std::time::Instant::now();

        // let seconds = 3;

        // for sec in (0..seconds).rev() {
        //     let msg = format!(
        //         "Sleeping for {sec} - (path {}, {}), [{}, {}]",
        //         path.ix(),
        //         &data_key,
        //         view[0].0,
        //         view[1].0
        //     );
        //     let _ = msg_tx.try_send((key.clone(), msg));

        //     tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        // }

        // load data source into cache & get data
        let data = data_cache.fetch_path_data(&data_key, path).await?;

        let fetch_time = t0.elapsed().as_secs_f32();

        let msg = format!(
            "Sampling (path {}, {}), [{}, {}] - data fetched in {fetch_time:.2} sec",
            path.ix(),
            &data_key,
            view[0].0,
            view[1].0
        );
        let _ = msg_tx.try_send((key.clone(), msg));

        // sample data into vector
        let sample_vec = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 4 * bin_count];

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

        Ok((view, sample_vec, generation))
    }

    fn any_slot_id_collisions(&self) -> bool {
        let mut id_count: HashMap<usize, usize> = HashMap::new();

        for (key, slot_id) in &self.slot_id_map {
            *id_count.entry(*slot_id).or_default() += 1;
        }

        let mut collision = false;

        for (slot_id, count) in id_count {
            if count > 1 {
                collision = true;
                log::error!("Slot ID collision: {slot_id}");
            }
        }

        let mut key_count: HashMap<&SlotKey, usize> = HashMap::new();

        self.slot_id_cache
            .iter()
            .enumerate()
            .filter_map(|(slot_id, entry)| Some((slot_id, entry.as_ref()?)))
            .for_each(|(_slot_id, key)| {
                *key_count.entry(key).or_default() += 1;
            });

        for ((path, data), count) in key_count {
            if count > 1 {
                collision = true;
                log::error!("Key collision: [{}-{data}]", path.ix());
            }
        }

        collision
    }
}

#[derive(Debug)]
pub enum SlotCacheError {
    OutOfRows,
}

impl std::fmt::Display for SlotCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotCacheError::OutOfRows => {
                write!(f, "Cannot assign rows without reallocating")
            }
        }
    }
}

impl std::error::Error for SlotCacheError {}
