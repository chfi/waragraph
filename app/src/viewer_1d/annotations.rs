use std::collections::{BTreeMap, HashMap};

use rstar::{
    primitives::{GeomWithData, Line},
    RTree,
};
use ultraviolet::Vec2;
use waragraph_core::graph::{Bp, PathId};

use super::view::View1D;

/// Rendering annotations into 1D viewer slots

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotSlotId(pub(super) u32);

pub struct Annots1D {
    slots: HashMap<AnnotSlotId, AnnotSlot>,
    next_slot_id: AnnotSlotId,
}

type AnnotsTreeObj = GeomWithData<Line<(i64,)>, usize>;

// Container for annotations displayed in a single 1D slot,
// with the annotations "flattened" to the pangenome coordinate
// space, down from the path-range space
pub struct AnnotSlot {
    // id: AnnotSlotId
    annots: RTree<AnnotsTreeObj>,

    // annots: BTreeMap<[Bp; 2], usize>,
    shapes: Vec<egui::Shape>,
    anchors: Vec<Option<Vec2>>,
}

impl AnnotSlot {
    fn new(
        annotations: impl IntoIterator<Item = (std::ops::Range<Bp>, egui::Shape)>,
    ) -> Self {
        let mut annot_objs = Vec::new();
        let mut shapes = Vec::new();

        for (a_id, (range, shape)) in annotations.into_iter().enumerate() {
            let geom =
                Line::new((range.start.0 as i64,), (range.end.0 as i64,));
            annot_objs.push(GeomWithData::new(geom, a_id));
            shapes.push(shape);
        }

        let anchors = vec![None; annot_objs.len()];

        let annots = RTree::<AnnotsTreeObj>::bulk_load(annot_objs);

        Self {
            annots,
            shapes,
            anchors,
        }
    }

    fn draw(&self, painter: &egui::Painter, view: &View1D) {
        use rstar::AABB;
        let rect = painter.clip_rect();

        let range = view.range();
        // let start = [Bp(range.start), Bp(range.start)];
        // let end = [Bp(range.end), Bp(range.end)];

        let aabb =
            AABB::from_corners((range.start as i64,), (range.end as i64,));

        let in_view = self.annots.locate_in_envelope_intersecting(&aabb);

        for line in in_view {
            let a_id = line.data;
            let left = Bp(line.geom().from.0 as u64);
            let right = Bp(line.geom().to.0 as u64);

            //
        }

        // let s_ix = self.annots.binary_search_by(|(&[l, r], _a_id)| {
        //
        // });

        /*
        for (&a_range, &a_id) in self.annots.range(start..end) {
        let [left, right] = a_range;
        let shape = &self.shapes[a_id];

        if self.anchors[a_id].is_none() {
            // create anchor
        }
        */
        //
        // }

        todo!();
    }
}
