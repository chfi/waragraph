/*



*/

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
    // slot_map:
    slot_state: HashMap<SlotKey, SlotState>,

    data_buffer: BufferDesc,
    rows: usize,

    bin_count: usize,

    vertex_buffer: BufferDesc,

    path_index: Arc<PathIndex>,
    data_cache: Arc<GraphDataCache>,
}

impl SlotCache {
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

    pub fn sample_and_update<I>(
        &mut self,
        rt: &tokio::runtime::Handle,
        view: &View1D,
        layout: I,
    ) where
        I: IntoIterator<Item = (SlotKey, egui::Rect)>,
    {
        let mut vertices: Vec<([f32; 4], u32)> = Vec::new();

        let vl = view.range().start;
        let vr = view.range().end;
        let cview = [Bp(vl), Bp(vr)];

        // the slots in the layout are the ones we really care about,
        // but we can't just throw away what we have in case the user
        // scrolls down and back up, for example
        for (key, rect) in layout {
            let state = self.slot_state.entry(key.clone()).or_default();

            if state.last_updated_view != Some(cview) {
                // spawn task
                let data_cache = self.data_cache.clone();
                let bin_count = self.bin_count;
                let path_index = self.path_index.clone();

                let task = rt.spawn(Self::slot_task(
                    path_index, data_cache, bin_count, key, cview,
                ));
                state.task_handle = Some(task);
            }
        }

        for (key, state) in self.slot_state.iter_mut() {
            // if state.task_ready() {
            if let Some((tview, data)) = state.task_results(rt) {
                if tview == cview {
                    // update slot in buffer
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
