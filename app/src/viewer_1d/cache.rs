/*



*/

use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
    sync::Arc,
};

use tokio::task::JoinHandle;
use waragraph_core::graph::{Bp, PathId};

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

    data_cache: Arc<GraphDataCache>,
}

impl SlotCache {
    async fn slot_task(
        data_cache: Arc<GraphDataCache>,
        key: SlotKey,
        view: [Bp; 2],
    ) -> Result<([Bp; 2], Vec<u8>)> {
        // let data_cache = self.data_cache.clone();

        // load data source into cache

        // get data from cache

        // sample data into vector
        let sample_vec = tokio::task::spawn_blocking(move || {
            //
            todo!();
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
                let task = rt.spawn(Self::slot_task(data_cache, key, cview));
                state.task_handle = Some(task);
            }

            // if let Some(state) =
            //
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
