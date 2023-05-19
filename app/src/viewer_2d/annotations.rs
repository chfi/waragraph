use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use egui::epaint::ahash;
use rstar::primitives::{GeomWithData, Line, Rectangle};
use rstar::RTree;
use ultraviolet::{Rotor2, Vec2};
use waragraph_core::graph::Node;

use crate::{
    annotations::{AnnotationSetId, GlobalAnnotationId},
    app::SharedState,
};

use super::{layout::NodePositions, view::View2D};

type AnnotObjId = usize;

struct AnnotObj {
    obj_id: AnnotObjId,
    annot_id: GlobalAnnotationId,
    label: Arc<String>,

    anchor_node: Node,
    anchor_pos: Vec2,
}

struct AnchorSet {
    nodes: BTreeSet<Node>,
}

type AnchorTreeObj = GeomWithData<Line<[f32; 2]>, (Node, AnnotObjId)>;

#[derive(Default)]
pub struct AnnotationLayer {
    annot_objs: Vec<AnnotObj>,

    anchor_rtree: Option<RTree<AnchorTreeObj>>,

    // indexed by AnnotObjId
    anchor_sets: Vec<AnchorSet>,
    // active_sets: BTreeSet<AnnotationSetId>,
    annot_shape_sizes: Vec<Vec2>,
}

impl AnnotationLayer {
    pub fn load_annotations(
        &mut self,
        shared: &SharedState,
        node_positions: &NodePositions,
        annot_ids: impl IntoIterator<Item = GlobalAnnotationId>,
    ) {
        let annotations = shared.annotations.blocking_read();

        let get_annotation = |annot_id: GlobalAnnotationId| {
            annotations
                .annotation_sets
                .get(&annot_id.set)
                .and_then(|set| set.get(annot_id.annot_id))
        };

        use rand::prelude::*;
        let mut rng = rand::thread_rng();

        let mut rtree_objs: Vec<AnchorTreeObj> = Vec::new();

        for annot_id in annot_ids {
            let obj_id = self.annot_objs.len();

            let annot = get_annotation(annot_id).unwrap();

            // step through annot. path range to build anchor set
            let anchor_nodes = shared
                .graph
                .path_step_range_iter(annot.path, annot.range.clone())
                .unwrap()
                .map(|(_pos, step)| step.node());

            let midpoint = annot.range.start.0
                + (annot.range.end.0 - annot.range.start.0) / 2;

            let anchor_set = AnchorSet {
                nodes: anchor_nodes.collect(),
            };

            for &node in anchor_set.nodes.iter() {
                let (p0, p1) = node_positions.node_pos(node);
                let data = (node, obj_id);
                rtree_objs.push(GeomWithData::new(
                    Line::new(p0.into(), p1.into()),
                    data,
                ));
            }

            // initialize anchor pos to random pos of random node in set
            // (if kept, should be uniform across the length of the range)
            let (anchor_node, anchor_pos) = {
                let node =
                    anchor_set.nodes.iter().choose(&mut rng).copied().unwrap();

                let (a0, a1) = node_positions.node_pos(node);

                let t = rng.gen_range(0f32..=1f32);
                let pos = a0 + t * (a1 - a0);

                (node, pos)
            };

            let obj = AnnotObj {
                obj_id,
                annot_id,
                label: annot.label.clone(),
                anchor_node,
                anchor_pos,
            };

            self.annot_objs.push(obj);
            self.anchor_sets.push(anchor_set);
        }

        if let Some(tree) = self.anchor_rtree.as_mut() {
            for obj in rtree_objs {
                tree.insert(obj);
            }
        } else {
            self.anchor_rtree = Some(RTree::bulk_load(rtree_objs));
        }
    }

    pub fn prepare_labels(&mut self, fonts: &egui::text::Fonts) {
        let obj_count = self.annot_objs.len();
        let size_count = self.annot_shape_sizes.len();

        if obj_count == size_count {
            // all annotation objects already have sizes
            return;
        } else if obj_count < size_count {
            unreachable!();
        }

        // let to_add = obj_count - size_count;

        for obj in &self.annot_objs[size_count..] {
            let shape = {
                let font = egui::FontId::proportional(16.0);
                let color = egui::Color32::WHITE;
                egui::Shape::text(
                    &fonts,
                    [0., 0.].into(),
                    egui::Align2::CENTER_CENTER,
                    &obj.label,
                    font,
                    color,
                )
            };

            let size: [f32; 2] = shape.visual_bounding_rect().size().into();
            self.annot_shape_sizes.push(size.into());
        }
    }

    const CLUSTER_RADIUS: f32 = 100.0;

    fn reset_anchors(
        &mut self,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
    ) -> roaring::RoaringBitmap {
        use rand::prelude::*;
        use rstar::AABB;
        let mut rng = rand::thread_rng();

        let mat = view.to_viewport_matrix(dims);

        if self.anchor_rtree.is_none() {
            panic!("`reset_anchors` must be called after `load_annotations`");
        }

        let (x0, x1) = view.x_range();
        let (y0, y1) = view.y_range();

        let view_rect = AABB::from_corners([x0, y0], [x1, y1]);

        let rtree = self.anchor_rtree.as_ref().unwrap();

        let in_view = rtree.locate_in_envelope_intersecting(&view_rect);

        let mut visible_annots: ahash::HashMap<
            AnnotObjId,
            ahash::HashSet<Node>,
        > = Default::default();

        for line in in_view {
            let (node, obj_id) = line.data;
            visible_annots.entry(obj_id).or_default().insert(node);
        }

        let mut visible_objs = roaring::RoaringBitmap::new();

        let t0 = std::time::Instant::now();
        for (obj_id, anchor_cands) in visible_annots {
            let obj = &mut self.annot_objs[obj_id];

            visible_objs.insert(obj_id as u32);

            // if !visible_nodes.contains(obj.anchor_node.ix() as u32) {
            if !anchor_cands.contains(&obj.anchor_node) {
                // reset this node
                let node = *anchor_cands.iter().next().unwrap();
                obj.anchor_node = node;

                let (a0, a1) = node_positions.node_pos(node);
                let t = rng.gen_range(0f32..=1f32);
                obj.anchor_pos = a0 + t * (a1 - a0);
            }
        }

        visible_objs
    }

    fn cluster_for_draw(
        &self,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
        visible_objs: &roaring::RoaringBitmap,
    ) -> Vec<(AnnotObjId, [f32; 2])> {
        use kiddo::distance::squared_euclidean;
        use kiddo::KdTree;
        use rstar::AABB;

        let mat = view.to_viewport_matrix(dims);

        let mut kdtree: KdTree<f32, 2> = KdTree::new();

        let mut clusters: Vec<Vec<AnnotObjId>> = Vec::new();

        for obj in &self.annot_objs {
            let pos = (mat * obj.anchor_pos.into_homogeneous_point()).xy();

            if pos.x.is_nan() || pos.y.is_nan() {
                continue;
            }

            // nearest_one doesn't return Option so need to check here
            if kdtree.size() == 0 {
                // just add the cluster and continue
                let cl_id = clusters.len();
                clusters.push(vec![obj.obj_id]);
                kdtree.add(pos.as_array(), cl_id);
            }

            let (dist, cl_id) =
                kdtree.nearest_one(pos.as_array(), &squared_euclidean);

            if dist > Self::CLUSTER_RADIUS {
                // create a new cluster
                let cl_id = clusters.len();
                clusters.push(vec![obj.obj_id]);
                kdtree.add(pos.as_array(), cl_id);
            } else {
                // append to the existing cluster
                clusters[cl_id].push(obj.obj_id);
            }
        }

        let mut to_draw = Vec::new();

        let mut label_rtree: RTree<Rectangle<[f32; 2]>> = RTree::new();

        for (_cl_id, objs) in clusters.into_iter().enumerate() {
            for obj_id in objs.into_iter().take(1) {
                let obj = &self.annot_objs[obj_id];

                if !visible_objs.contains(obj_id as u32) {
                    continue;
                }

                let label_pos = {
                    let rotor = Rotor2::from_rotation_between(
                        Vec2::unit_y(),
                        Vec2::unit_x(),
                    )
                    .normalized();

                    let (a0, a1) = node_positions.node_pos(obj.anchor_node);
                    let normal = (a1 - a0).normalized().rotated_by(rotor);

                    let pos =
                        (mat * obj.anchor_pos.into_homogeneous_point()).xy();

                    let label_size = self.annot_shape_sizes[obj_id];

                    pos + normal * normal.dot(label_size) * 2.0
                };

                let label_size = self.annot_shape_sizes[obj_id];

                let p0 = label_pos - label_size / 2.0;
                let p1 = label_pos + label_size / 2.0;

                let aabb = AABB::from_corners(p0.into(), p1.into());

                if label_rtree
                    .locate_in_envelope_intersecting(&aabb)
                    .next()
                    .is_none()
                {
                    to_draw.push((obj_id, *label_pos.as_array()));
                    let rect = Rectangle::from_corners(p0.into(), p1.into());
                    label_rtree.insert(rect);
                }
            }
        }

        println!("to_draw.len() {}", to_draw.len());

        to_draw
    }

    pub fn draw(
        &mut self,
        // shared: &SharedState,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
        painter: &egui::Painter,
    ) {
        painter.fonts(|fonts| self.prepare_labels(fonts));

        let visible_objs = self.reset_anchors(node_positions, view, dims);

        let to_draw =
            self.cluster_for_draw(node_positions, view, dims, &visible_objs);

        for (obj_id, pos) in to_draw {
            let obj = &mut self.annot_objs[obj_id];

            let shape = painter.fonts(|fonts| {
                let font = egui::FontId::proportional(16.0);
                let color = egui::Color32::WHITE;
                egui::Shape::text(
                    &fonts,
                    pos.into(),
                    egui::Align2::CENTER_CENTER,
                    &obj.label,
                    font,
                    color,
                )
            });

            painter.add(shape);
        }
    }
}
