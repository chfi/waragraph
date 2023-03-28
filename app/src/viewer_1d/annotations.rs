use std::collections::{BTreeMap, HashMap};

use rstar::{
    primitives::{GeomWithData, Line},
    RTree,
};
use ultraviolet::Vec2;
use waragraph_core::graph::{Bp, PathId, PathIndex};

use super::view::View1D;

/// Rendering annotations into 1D viewer slots

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotSlotId(pub(super) u32);

#[derive(Default)]
pub struct Annots1D {
    slots: HashMap<AnnotSlotId, AnnotSlot>,
    next_slot_id: AnnotSlotId,
}

impl Annots1D {
    pub fn insert_slot(&mut self, slot: AnnotSlot) -> AnnotSlotId {
        let slot_id = self.next_slot_id;
        self.slots.insert(slot_id, slot);
        self.next_slot_id = AnnotSlotId(slot_id.0 + 1);
        slot_id
    }

    pub fn get(&self, slot_id: &AnnotSlotId) -> Option<&AnnotSlot> {
        self.slots.get(slot_id)
    }

    pub fn get_mut(&mut self, slot_id: &AnnotSlotId) -> Option<&mut AnnotSlot> {
        self.slots.get_mut(slot_id)
    }
}

type AnnotsTreeObj = GeomWithData<Line<(i64, i64)>, usize>;

// type ShapeFn = Box<dyn Fn(egui::Pos2, egui::Rect) -> egui::Shape>;
type ShapeFn = Box<dyn Fn(&egui::Painter, egui::Pos2) -> egui::Shape>;

pub fn text_shape<L: ToString>(label: L) -> ShapeFn {
    let label = label.to_string();
    Box::new(move |painter, pos| {
        let fonts = painter.fonts();
        let font = egui::FontId::proportional(16.0);
        let color = egui::Color32::WHITE;
        egui::Shape::text(
            &fonts,
            pos,
            egui::Align2::CENTER_CENTER,
            label.clone(),
            font,
            color,
        )
    })
}

// Container for annotations displayed in a single 1D slot,
// with the annotations "flattened" to the pangenome coordinate
// space, down from the path-range space
pub struct AnnotSlot {
    // id: AnnotSlotId
    annots: RTree<AnnotsTreeObj>,

    // annots: BTreeMap<[Bp; 2], usize>,
    shapes: Vec<ShapeFn>,
    anchors: Vec<Option<f64>>,
}

impl AnnotSlot {
    /// Initializes an annotation slot given items in pangenome space.
    ///
    pub fn new_from_pangenome_space(
        annotations: impl IntoIterator<Item = (std::ops::Range<Bp>, ShapeFn)>,
    ) -> Self {
        let mut annot_objs = Vec::new();
        let mut shapes = Vec::new();

        for (a_id, (range, shape)) in annotations.into_iter().enumerate() {
            let geom =
                Line::new((range.start.0 as i64, 0), (range.end.0 as i64, 0));
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
    pub fn new_from_path_space(
        graph: &PathIndex,
        annotations: impl IntoIterator<
            Item = (PathId, std::ops::Range<Bp>, ShapeFn),
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

    pub(super) fn draw(&mut self, painter: &egui::Painter, view: &View1D) {
        use rstar::AABB;
        let rect = painter.clip_rect();
        let rleft = rect.left() as f64;
        let rright = rect.right() as f64;

        let screen_interval = rleft..=rright;

        let range = view.range();

        let aabb =
            AABB::from_corners((range.start as i64, 0), (range.end as i64, 0));

        let in_view = self.annots.locate_in_envelope_intersecting(&aabb);

        for line in in_view {
            let a_id = line.data;
            let left = line.geom().from.0 as u64;
            let right = line.geom().to.0 as u64;

            if self.anchors[a_id].is_none() {
                if let Some(a_range) =
                    anchor_interval(view, &(left..right), &screen_interval)
                {
                    let (l, r) = a_range.into_inner();
                    let x = l + (r - l) * 0.5;
                    self.anchors[a_id] = Some(x);
                }
            }

            let y = rect.center().y;
            if let Some(x) = self.anchors[a_id] {
                let pos = egui::pos2(x as f32, y);
                let shape = (self.shapes[a_id])(painter, pos);
                painter.add(shape);
            }
        }
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
    screen_interval: &std::ops::RangeInclusive<f64>,
) -> Option<std::ops::RangeInclusive<f64>> {
    let vrange = view.range();
    let pleft = pan_range.start;
    let pright = pan_range.end;

    if pleft > vrange.end || pright < vrange.start {
        return None;
    }

    let vl = vrange.start as f64;
    let vr = vrange.end as f64;
    let vlen = vr - vl;

    let pl = pleft as f64;
    let pr = pright as f64;

    let left = pleft.max(vrange.start);
    let right = pright.min(vrange.end);

    let l = left as f64;
    let r = right as f64;

    let lt = (l - vl) / vlen;
    let rt = (r - vr) / vlen;

    let (sleft, sright) = screen_interval.clone().into_inner();
    let slen = sright - sleft;

    let a_left = sleft + lt * slen;
    let a_right = sleft + rt * slen;

    Some(a_left..=a_right)
}

pub(crate) mod util {
    use waragraph_core::graph::{Bp, Node};

    use super::*;

    pub(crate) fn label_nodes<L: ToString>(
        graph: &PathIndex,
        labels: impl IntoIterator<Item = (Node, L)>,
    ) -> AnnotSlot {
        let annots = labels.into_iter().map(|(node, label)| {
            let node_range = graph.node_pangenome_range(node);
            (node_range, text_shape(label))
        });

        AnnotSlot::new_from_pangenome_space(annots)
    }
}
