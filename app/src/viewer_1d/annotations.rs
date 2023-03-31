use std::collections::{BTreeMap, HashMap, HashSet};

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

    path_annot_slot: HashMap<PathId, AnnotSlotId>,
}

impl Annots1D {
    pub fn get_path_slot_id(&self, path: PathId) -> Option<AnnotSlotId> {
        let slot = self.path_annot_slot.get(&path)?;
        Some(*slot)
    }

    pub fn insert_slot(
        &mut self,
        path: PathId,
        slot: AnnotSlot,
    ) -> AnnotSlotId {
        let slot_id = self.next_slot_id;
        self.slots.insert(slot_id, slot);
        self.path_annot_slot.insert(path, slot_id);
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
    // really corresponds to the anchor regions
    annots: RTree<AnnotsTreeObj>,

    // annots: BTreeMap<[Bp; 2], usize>,
    shape_fns: Vec<ShapeFn>,
    // shape_positions: Vec<Option<Vec2>>,
    anchors: Vec<Option<f32>>,

    dynamics: AnnotSlotDynamics,
}

#[derive(Default)]
struct AnnotSlotDynamics {
    // annot id -> annot_shape_objs ix
    annot_obj_map: HashMap<usize, usize>,
    annot_shape_objs: Vec<AnnotObj>,

    deltas: Vec<Vec2>,

    cur_view: Option<View1D>,
    prev_view: Option<View1D>,
}

impl AnnotSlotDynamics {
    fn get_annot_obj(&self, a_id: usize) -> Option<&AnnotObj> {
        let i = *self.annot_obj_map.get(&a_id)?;
        Some(&self.annot_shape_objs[i])
    }

    fn get_annot_obj_mut(&mut self, a_id: usize) -> Option<&mut AnnotObj> {
        let i = *self.annot_obj_map.get(&a_id)?;
        Some(&mut self.annot_shape_objs[i])
    }

    fn get_or_insert_annot_obj_mut(
        &mut self,
        a_id: usize,
        pos: Vec2,
    ) -> &mut AnnotObj {
        if let Some(i) = self.annot_obj_map.get(&a_id) {
            &mut self.annot_shape_objs[*i]
        } else {
            let obj_i = self.annot_shape_objs.len();
            let obj = AnnotObj::with_pos(a_id, pos);
            self.annot_obj_map.insert(a_id, obj_i);
            self.annot_shape_objs.push(obj);
            &mut self.annot_shape_objs[obj_i]
        }
    }

    fn prepare(
        &mut self,
        annots: &RTree<AnnotsTreeObj>,
        screen_rect: egui::Rect,
        view: &View1D,
    ) {
        // initialize AnnotObjPos for the annotations in the view
        // treat X as anchor X; use separate objects with spring constraint later

        use rstar::AABB;
        let rleft = screen_rect.left();
        let rright = screen_rect.right();

        let screen_interval = rleft..=rright;

        let range = view.range();

        let aabb =
            AABB::from_corners((range.start as i64, 0), (range.end as i64, 0));

        let in_view = annots.locate_in_envelope_intersecting(&aabb);

        self.prev_view = self.cur_view.clone();
        self.cur_view = Some(view.clone());

        let view_changed =
            self.prev_view != self.cur_view && self.cur_view.is_some();

        /*
        if view_changed, use prev_view & cur_view to derive the delta
        transformation to be applied to the (screenspace) annotation
        positions
        */

        for line in in_view {
            let a_id = line.data;
            let left = line.geom().from.0 as u64;
            let right = line.geom().to.0 as u64;

            let reset_pos =
                if let Some(pos) = self.get_annot_obj(a_id).map(|o| o.pos) {
                    pos.pos_now.x < rleft || pos.pos_now.x > rright
                } else {
                    true
                };

            let anchor_range =
                anchor_interval(view, &(left..right), &screen_interval);

            if reset_pos {
                if let Some(a_range) = anchor_range.as_ref() {
                    let (l, r) = a_range.clone().into_inner();
                    let x = l + (r - l) * 0.5;

                    // let y = screen_rect.center().y + 20.0 * (x * 10.0).sin();
                    let y = screen_rect.center().y + 20.0 * x.sin();

                    let _obj =
                        self.get_or_insert_annot_obj_mut(a_id, Vec2::new(x, y));
                }
            } else if view_changed {
                // only apply the view transform to annots that haven't been reset

                let transform = self
                    .prev_view
                    .as_ref()
                    .and_then(|v0| Some((v0, self.cur_view.as_ref()?)))
                    .map(|(v0, v1)| {
                        super::Viewer1D::sample_index_transform(
                            v0.range(),
                            v1.range(),
                        )
                    });

                if let Some([a, b]) = transform {
                    let w = screen_rect.width();
                    let x0 = screen_rect.left();

                    let apply_tf = |p: &mut Vec2| {
                        let x = p.x - x0;
                        let x_ = x * a - w * b;
                        p.x = x_ + x0;
                    };

                    if let Some(obj) = self.get_annot_obj_mut(a_id) {
                        apply_tf(&mut obj.pos.pos_now);
                        // apply_tf(&mut obj.pos.pos_old);
                    }
                }
            }

            {
                // TODO: update the closest point to the anchor set from the
                // annotation's current position

                if let Some((a_range, obj)) = self
                    .get_annot_obj_mut(a_id)
                    .and_then(|obj| Some((anchor_range?, obj)))
                {
                    let (left, right) = a_range.into_inner();
                    let range_x = left + (right - left) * 0.5;
                    let obj_x = obj.pos.pos_now.x;

                    let dist = (range_x - obj_x).abs();

                    if let Some(cur_closest) = obj.closest_anchor_pos.as_mut() {
                        if (*cur_closest - obj_x).abs() > dist {
                            *cur_closest = range_x;
                        }
                    } else {
                        obj.closest_anchor_pos = Some(range_x);
                    }
                };
            }
        }
    }

    // also updates the size on screen of the cached annot objects
    fn draw(
        &mut self,
        annots: &RTree<AnnotsTreeObj>,
        shape_fns: &[ShapeFn],
        painter: &egui::Painter,
        view: &View1D,
    ) {
        for (&a_id, &obj_i) in &self.annot_obj_map {
            let obj = &mut self.annot_shape_objs[obj_i];

            let pos = obj.pos();

            let shape = (shape_fns[a_id])(painter, egui::pos2(pos.x, pos.y));
            let rect = shape.visual_bounding_rect();

            let size = rect.size();
            obj.shape_size = Some(Vec2::new(size.x, size.y));

            if obj.closest_anchor_pos.is_none() {
                continue;
            }

            painter.add(shape);
        }
    }

    fn update(&mut self, screen_rect: egui::Rect, dt: f32) {
        let objs = self.annot_shape_objs.len();

        self.deltas.clear();
        self.deltas.resize(objs, Vec2::zero());

        for i in 0..objs {
            for j in 0..objs {
                if i == j {
                    continue;
                }

                let (rect_i, rect_j) = {
                    let ri = self.annot_shape_objs[i].egui_rect();
                    let rj = self.annot_shape_objs[j].egui_rect();

                    if let (Some(ri), Some(rj)) = (ri, rj) {
                        (ri, rj)
                    } else {
                        continue;
                    }
                };

                let delta = AnnotObj::intersect_delta(rect_i, rect_j);

                // if delta.y.abs() > 0.0 {
                //     log::warn!("obj {i} - {delta:?}");
                // }

                self.annot_shape_objs[i].pos.pos_now += delta;

                // obj.pos.pos_now += delta;
                // self.deltas[i] += delta;
            }
        }

        for (_obj_i, (&delta, obj)) in self
            .deltas
            .iter()
            .zip(self.annot_shape_objs.iter_mut())
            .enumerate()
        {
            // obj.pos.pos_now += delta * dt;
            // obj.pos.pos_now += delta;
            // obj.pos.accel = delta;

            // gravity
            obj.pos.accel.y += 10.0;
            let v = obj.pos.pos_now - obj.pos.pos_old;

            if let Some(anchor) = obj.closest_anchor_pos {
                // let dist = anchor - obj.pos.pos_now.x;
                obj.pos.pos_now.x = anchor;
                /*
                let dist = obj.pos.pos_now.x - anchor;

                if dist.abs() > 1.0 {
                    // let k = 1.0;
                    // let k = 0.1;
                    // let k = 0.5;
                    let k = 0.5;

                    let f = -k * dist;
                    let a = f;

                    obj.pos.accel.x += a;
                } else {
                    let damp = 10.0;
                    obj.pos.accel.x = -v.x * damp;
                }
                */

                // obj.pos.pos_now.x = anchor;
                // obj.closest_anchor_pos = None;
            }

            // TODO: disabled until the collision & update_position behave correctly
            // obj.pos.update_position(dt);

            if let Some(rect) = obj.egui_rect() {
                if rect.bottom() > screen_rect.bottom() {
                    // obj.pos.pos_now.y = screen_rect.bottom() - rect.height();
                    obj.pos.pos_now.y -= rect.bottom() - screen_rect.bottom();
                }
            }

            // reset
            obj.closest_anchor_pos = None;

            // apply anchor constraint
            // if let Some(anchor) = obj.closest_anchor_pos {
            //     obj.pos.pos_now.x = anchor;
            //     obj.closest_anchor_pos = None;
            // }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AnnotObjPos {
    pos_now: Vec2,
    pos_old: Vec2,
    accel: Vec2,
}

impl AnnotObjPos {
    fn at_pos(pos: Vec2) -> Self {
        Self {
            pos_now: pos,
            pos_old: pos,
            accel: Vec2::zero(),
        }
    }
    fn update_position(&mut self, dt: f32) {
        let vel = self.pos_now - self.pos_old;
        self.pos_old = self.pos_now;
        self.pos_now = self.pos_now + vel + self.accel * dt * dt;
        self.accel = Vec2::zero();
    }

    fn accelerate(&mut self, acc: Vec2) {
        self.accel += acc;
    }
}

#[derive(Debug, Clone, Copy)]
struct AnnotObj {
    annot_id: usize,

    pos: AnnotObjPos,
    closest_anchor_pos: Option<f32>,
    // anchor_pos: Option<AnnotObjPos>,
    shape_size: Option<Vec2>,
}

impl AnnotObj {
    fn with_pos(annot_id: usize, pos: Vec2) -> Self {
        Self {
            annot_id,
            pos: AnnotObjPos::at_pos(pos),
            closest_anchor_pos: None,
            shape_size: None,
        }
    }

    fn pos(&self) -> Vec2 {
        self.pos.pos_now
    }

    fn size(&self) -> Option<Vec2> {
        self.shape_size
    }

    fn egui_rect(&self) -> Option<egui::Rect> {
        let pos = self.pos();
        let size = self.size()?;
        let rect = egui::Rect::from_center_size(
            egui::pos2(pos.x, pos.y),
            egui::vec2(size.x, size.y),
        );
        Some(rect)
    }

    fn collides_impl(&self, other: &Self) -> Option<bool> {
        let a = self.egui_rect()?;
        let b = other.egui_rect()?;
        Some(a.intersects(b))
    }

    fn collides(&self, other: &Self) -> bool {
        self.collides_impl(other).unwrap_or(false)
    }

    // outputs the delta that when applied to `this` resolves half of
    // the collision between the two
    fn intersect_delta(this: egui::Rect, other: egui::Rect) -> Vec2 {
        if !this.intersects(other) {
            return Vec2::zero();
        }

        let t_center: Vec2 = mint::Point2::from(this.center()).into();
        let o_center: Vec2 = mint::Point2::from(this.center()).into();
        let diff = t_center - o_center;

        let intersection = this.intersect(other);

        if diff.x.abs() < diff.y.abs() {
            let dx = if diff.x < 0.0 {
                //   `this` left of `other`
                -intersection.width()
            } else {
                //   `this` right of `other`
                intersection.width()
            };

            let dx = dx * 0.5;

            Vec2::new(dx, 0.0)
        } else {
            let dy = if diff.y < 0.0 {
                //   `this` above `other`
                -intersection.height()
            } else {
                //   `this` below `other`
                intersection.height()
            };

            let dy = dy * 0.5;

            Vec2::new(0.0, dy)
        }
    }
}

impl AnnotSlot {
    /// Initializes an annotation slot given items in pangenome space.
    ///
    pub fn new_from_pangenome_space(
        annotations: impl IntoIterator<Item = (std::ops::Range<Bp>, ShapeFn)>,
    ) -> Self {
        let mut annot_objs = Vec::new();
        let mut shape_fns = Vec::new();

        for (a_id, (range, shape)) in annotations.into_iter().enumerate() {
            let geom =
                Line::new((range.start.0 as i64, 0), (range.end.0 as i64, 0));
            annot_objs.push(GeomWithData::new(geom, a_id));
            shape_fns.push(shape);
        }

        let anchors = vec![None; shape_fns.len()];

        let annots = RTree::<AnnotsTreeObj>::bulk_load(annot_objs);

        Self {
            annots,
            shape_fns,
            anchors,
            dynamics: Default::default(),
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
        let mut annot_objs = Vec::new();
        let mut shape_fns = Vec::new();

        for (a_id, (path, path_range, shape)) in
            annotations.into_iter().enumerate()
        {
            shape_fns.push(shape);
            let range_end = path_range.end;
            if let Some(steps) = graph.path_step_range_iter(path, path_range) {
                for (start, step) in steps {
                    let len = graph.node_length(step.node()).0 as usize;
                    let end = (start + len).min(range_end.0 as usize);
                    let geom = Line::new((start as i64, 0), (end as i64, 0));
                    annot_objs.push(GeomWithData::new(geom, a_id));
                }
            }
        }

        let anchors = vec![None; shape_fns.len()];

        let annots = RTree::<AnnotsTreeObj>::bulk_load(annot_objs);

        Self {
            annots,
            shape_fns,
            anchors,
            dynamics: Default::default(),
        }
    }

    pub(super) fn draw(&mut self, painter: &egui::Painter, view: &View1D) {
        let screen_rect = painter.clip_rect();
        self.dynamics.prepare(&self.annots, screen_rect, view);

        self.dynamics
            .draw(&self.annots, &self.shape_fns, painter, view);

        // debug rendering
        for (obj_i, delta) in self.dynamics.deltas.iter().enumerate() {
            let obj = &self.dynamics.annot_shape_objs[obj_i];

            let pos: egui::Pos2 = mint::Point2::from(obj.pos.pos_now).into();
            let vec: egui::Vec2 = mint::Vector2::from(*delta).into();

            let stroke = egui::Stroke::new(2.0, egui::Color32::RED);

            painter.arrow(pos, vec, stroke);
        }
    }

    pub(super) fn update(&mut self, screen_rect: egui::Rect, dt: f32) {
        self.dynamics.update(screen_rect, dt);
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

    let left = pleft.max(vrange.start);
    let right = pright.min(vrange.end);

    let l = left as f32;
    let r = right as f32;

    let lt = (l - vl) / vlen;
    let rt = (r - vl) / vlen;

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

    pub(crate) fn pangenome_range_labels<L: ToString>(
        labels: impl IntoIterator<Item = (std::ops::Range<Bp>, L)>,
    ) -> AnnotSlot {
        let annots = labels
            .into_iter()
            .map(|(range, label)| (range, text_shape(label)));
        AnnotSlot::new_from_pangenome_space(annots)
    }
}
