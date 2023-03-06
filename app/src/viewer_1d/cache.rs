/*



*/

use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
};

use tokio::task::JoinHandle;
use waragraph_core::graph::{Bp, PathId};

use crate::util::BufferDesc;

use super::view::View1D;

// TODO: still using string-typed keys for data sources and viz. modes! bad!
pub type SlotKey = (PathId, String);

#[derive(Default)]
struct SlotState {
    last_updated_view: Option<[Bp; 2]>,
    task_handle: Option<JoinHandle<([Bp; 2], Vec<u8>)>>,
    row_index: Option<usize>,
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
}

impl SlotCache {
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
            }

            // if let Some(state) =
            //
        }

        todo!();
    }

    pub fn render_slots(&mut self) -> (std::ops::Range<u32>, Vec<egui::Shape>) {
        todo!();
    }
}
