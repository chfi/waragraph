use std::collections::{BTreeMap, HashMap};

use rstar::{
    primitives::{GeomWithData, Line},
    RTree,
};
use ultraviolet::Vec2;
use waragraph_core::graph::{Bp, PathId, PathIndex};

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
    /// Initializes an annotation slot given items in pangenome space.
    ///
    fn new_from_pangenome_space(
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

    /// Initializes an annotation slot given items in path space.
    /// The path ranges to pangenome space, splitting them if
    /// necessary.
    fn new_from_path_space(
        graph: &PathIndex,
        annotations: impl IntoIterator<
            Item = (PathId, std::ops::Range<Bp>, egui::Shape),
        >,
    ) -> Self {
        // let mut annot_objs = Vec::new();
        // let mut shapes = Vec::new();

        for (a_id, (path, path_range, shape)) in
            annotations.into_iter().enumerate()
        {
            todo!();

            // use path index to split path_range into pangenome range chunks
            // all chunks get the same a_id

            /*
            let geom =
                Line::new((range.start.0 as i64,), (range.end.0 as i64,));
            annot_objs.push(GeomWithData::new(geom, a_id));
            shapes.push(shape);
            */
        }

        // let anchors = vec![None; annot_objs.len()];

        // let annots = RTree::<AnnotsTreeObj>::bulk_load(annot_objs);

        // Self {
        //     annots,
        //     shapes,
        //     anchors,
        // }
        todo!();
    }

    fn draw(&self, painter: &egui::Painter, view: &View1D) {
        use rstar::AABB;
        let rect = painter.clip_rect();
        let rleft = rect.left() as f64;
        let rright = rect.right() as f64;

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

            let l_f = left.0 as f64;
            let r_f = right.0 as f64;
            // let lf = rect.left() as f64

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

// returns the range of valid anchor points along the x-axis, in
// screen space, of the `pan_range` range under the transformation
// induced by the `view` and `screen_interval`
//
// if the intersection of `view` and `pan_range` is empty, None is returned
fn anchor_interval(
    view: &View1D,
    pan_range: &std::ops::Range<u64>,
    screen_interval: &std::ops::RangeInclusive<f32>,
) -> Option<std::ops::RangeInclusive<f32>> {
    let vrange = view.range();
    let pleft = pan_range.start;
    let pright = pan_range.end;

    if pleft > vrange.end || pright < vrange.start {
        return None;
    }

    let vl = vrange.start as f32;
    let vr = vrange.end as f32;
    let vlen = vr - vl;

    let pl = pleft as f32;
    let pr = pright as f32;

    let left = pleft.max(vrange.start);
    let right = pright.min(vrange.end);

    let l = left as f32;
    let r = right as f32;

    let lt = (l - vl) / vlen;
    let rt = (r - vr) / vlen;

    let (sleft, sright) = screen_interval.clone().into_inner();
    let slen = sright - sleft;

    let a_left = sleft + lt * slen;
    let a_right = sleft + rt * slen;

    Some(a_left..=a_right)
}
