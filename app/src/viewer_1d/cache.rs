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

type SlotTaskHandle = JoinHandle<Result<([Bp; 2], Vec<u8>)>>;

#[derive(Default)]
struct SlotState {
    last_updated_view: Option<[Bp; 2]>,
    task_handle: Option<SlotTaskHandle>,
    row_index: Option<usize>,
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
    slot_state: HashMap<SlotKey, SlotState>,

    // indexed by SlotId
    slot_id_cache: Vec<Option<(SlotKey, u64)>>,
    // map to SlotId
    slot_id_map: HashMap<SlotKey, usize>,
    slot_id_generation: u64,

    data_buffer: BufferDesc,
    rows: usize,

    bin_count: usize,

    vertex_buffer: BufferDesc,

    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,
}

impl SlotCache {
    pub fn new(
        path_index: Arc<PathIndex>,
        data_cache: Arc<GraphDataCache>,
        row_count: usize,
        bin_count: usize,
    ) -> Result<Self> {
        todo!();
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

    fn allocate_data_buffer(
        state: &raving_wgpu::State,
        rows: usize,
        bin_count: usize,
    ) -> Result<BufferDesc> {
        todo!();
    }

    // returns the view transform, based on the last dispatched view
    // and the current view, for use in a fragment uniform buffer
    pub fn get_view_transform(&self, view: &View1D) -> [f32; 2] {
        todo!();
    }

    pub fn sample_and_update<I>(
        &mut self,
        rt: &tokio::runtime::Handle,
        view: &View1D,
        layout: I,
    ) where
        I: IntoIterator<Item = (SlotKey, egui::Rect)>,
    {
        let vl = view.range().start;
        let vr = view.range().end;
        let cview = [Bp(vl), Bp(vr)];

        let layout = layout.into_iter().collect::<HashMap<_, _>>();

        {
            let active_count = layout.len() + self.rows / 8;
            let oldest_gen = self
                .slot_id_generation
                .checked_sub(active_count as u64)
                .unwrap_or(0);
            let mut slot_ids_by_gen = self
                .slot_id_cache
                .iter_mut()
                .filter_map(|entry| {
                    let entry = entry.as_mut()?;
                    let is_active = layout.contains_key(&entry.0);
                    let is_old = entry.1 < oldest_gen;

                    (!is_active && is_old).then_some(entry)
                })
                .enumerate()
                .collect::<Vec<_>>();

            slot_ids_by_gen.sort_by_key(|(_, (_, gen))| gen);

            // iterates over cache entries that are not used by the input layout
            // and are old enough to be cleared
            let mut cache_iter = slot_ids_by_gen.into_iter();

            // the slots in the layout are the ones we really care about,
            // but we can't just throw away what we have in case the user
            // scrolls down and back up, for example
            //
            // this is also where we assign the slot IDs for each slot in the layout
            for (key, rect) in layout.iter() {
                let state = self.slot_state.entry(key.clone()).or_default();

                // assign slot ID
                if let Some(slot_id) = self.slot_id_map.get(key) {
                    // this should never happen, but
                    if self.slot_id_cache[*slot_id].is_none() {
                        let new_gen = self.slot_id_generation;
                        self.slot_id_generation += 1;
                        self.slot_id_cache[*slot_id] =
                            Some((key.clone(), new_gen));
                    }
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
                    let (slot_id, cache_entry) = cache_iter.next().unwrap();
                    // update the slot key -> slot ID map in the cache
                    *cache_entry = (key.clone(), new_gen);
                    self.slot_id_map.insert(key.clone(), slot_id);
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

        // for each slot with a finished task, if the task contains data
        // for the correct view, find the first row in the data buffer that
        // has is not mapped to a slot key that has been used for a while
        // (or use the slot ID if already in the cache)

        let mut slot_index = 0usize;
        for (key, state) in self.slot_state.iter_mut() {
            if let Some((task_view, data)) = state.task_results(rt) {
                if Some(task_view) != self.last_dispatched_view {
                    // discarding
                    continue;
                }

                // store slot ID for slot key
                let slot_id = if let Some(id) = self.slot_id_map.get(key) {
                    *id
                } else {
                    unreachable!();
                    // todo!();
                };

                // update slot in buffer
                todo!();
            }
        }

        let mut vertices: Vec<([f32; 4], u32)> = Vec::new();

        // add a vertex for each slot in the layout that has an up to date
        // row in the data buffer
        for (key, rect) in layout {
            if let Some(state) = self.slot_state.get(&key) {
                // let

                let last_dispatch = self.last_dispatched_view;
                let last_update = state.last_updated_view;

                if last_update == last_dispatch && last_dispatch.is_some() {
                    //
                    todo!();
                }
            }
        }

        todo!();
    }

    pub fn render_slots(&mut self) -> (std::ops::Range<u32>, Vec<egui::Shape>) {
        todo!();
    }
}
