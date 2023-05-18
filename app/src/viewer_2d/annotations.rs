use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use egui::epaint::ahash;
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

#[derive(Default)]
pub struct AnnotationLayer {
    annot_objs: Vec<AnnotObj>,

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

            let anchor_node = shared
                .graph
                .step_at_pos(annot.path, midpoint)
                .map(|s| s.node())
                .or(anchor_set.nodes.first().copied())
                .unwrap();

            // initialize anchor pos to random pos of random node in set
            // (if kept, should be uniform across the length of the range)
            let (anchor_node, anchor_pos) = {
                let node =
                    anchor_set.nodes.iter().choose(&mut rng).copied().unwrap();

                let (a0, a1) = node_positions.node_pos(anchor_node);

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

    fn cluster_for_draw(
        &self,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
    ) -> Vec<(AnnotObjId, [f32; 2])> {
        use kiddo::distance::squared_euclidean;
        use kiddo::KdTree;

        let mat = view.to_viewport_matrix(dims);

        let mut kdtree: KdTree<f32, 2> = KdTree::new();

        let mut clusters: Vec<Vec<AnnotObjId>> = Vec::new();

        for obj in &self.annot_objs {
            let pos = (mat * obj.anchor_pos.into_homogeneous_point()).xy();

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

        for (_cl_id, objs) in clusters.into_iter().enumerate() {
            for obj_id in objs.into_iter().take(1) {
                let obj = &self.annot_objs[obj_id];

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

                    pos + normal * 70.0
                };

                to_draw.push((obj_id, *label_pos.as_array()))
            }
        }

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

        let to_draw = self.cluster_for_draw(node_positions, view, dims);

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

            // add only if no collision, for now
            // if to_draw.contains(&obj.obj_id) {
            painter.add(shape);
        }
    }
}
