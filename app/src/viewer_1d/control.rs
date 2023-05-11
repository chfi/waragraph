use waragraph_core::graph::{Bp, PathId};

use super::view::View1D;

pub enum Msg {
    View(ViewMsg),
}

pub enum ViewMsg {
    GotoPos {
        path: Option<PathId>,
        pos: Bp,
    },
    GotoRange {
        path: Option<PathId>,
        range: std::ops::Range<Bp>,
    },
}

impl ViewMsg {
    pub fn apply(self, view: &mut View1D) {
        todo!();
    }
}
