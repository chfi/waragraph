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
    label_pos: Vec2,

    shape_size: Option<Vec2>,
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

            // initialize anchor pos to middle of random node in set

            let (anchor_node, anchor_pos, da) = {
                let node =
                    anchor_set.nodes.iter().choose(&mut rng).copied().unwrap();

                let (a0, a1) = node_positions.node_pos(anchor_node);

                let t = rng.gen_range(0f32..=1f32);
                let pos = a0 + t * (a1 - a0);

                (node, pos, a1 - a0)
            };
            // let (a0, a1) = node_positions.node_pos(anchor_node);
            // let da = a1 - a0;
            // let anchor_pos = a0 + 0.5 * da;

            // initialize label pos some distance out from the anchor
            // pos, along the node's normal

            let label_pos = {
                let rotor = Rotor2::from_rotation_between(
                    Vec2::unit_y(),
                    Vec2::unit_x(),
                )
                .normalized();
                let normal = da.normalized().rotated_by(rotor);

                anchor_pos + normal * 80.0
            };

            let obj = AnnotObj {
                obj_id,
                annot_id,
                label: annot.label.clone(),
                anchor_node,
                anchor_pos,
                label_pos,
                // shape size can't be set before rendering
                shape_size: None,
            };

            self.annot_objs.push(obj);
            self.anchor_sets.push(anchor_set);
        }
    }

    fn cluster_for_draw(
        &self,
        view: &View2D,
        dims: Vec2,
    ) -> ahash::HashSet<AnnotObjId> {
        // use kiddo::KdTree;
        use kiddo::float::kdtree::KdTree;
        let mat = view.to_viewport_matrix(dims);

        let mut kdtree: KdTree<_, usize, 2, 64, u32> = KdTree::new();

        for obj in &self.annot_objs {
            let pos = (mat * obj.anchor_pos.into_homogeneous_point()).xy();
            kdtree.add(pos.as_array(), obj.obj_id);
        }

        // annotations whose anchor points are "close enough"
        // (probably within a few, maybe 10 pixels) should be combined
        // into a single cluster for drawing (possibly not drawing any
        // labels, instead an icon or marker)

        let mut obj_cluster_map: ahash::HashMap<AnnotObjId, usize> =
            Default::default();
        let mut clusters: Vec<([f32; 2], ahash::HashSet<AnnotObjId>)> =
            Vec::new();

        let radius = 1f32;

        let mut noncolliding = roaring::RoaringBitmap::new();
        noncolliding.insert_range(0..self.annot_objs.len() as u32);

        for obj in &self.annot_objs {
            let pos = (mat * obj.anchor_pos.into_homogeneous_point()).xy();
            let neighbors = kdtree.within_unsorted(
                pos.as_array(),
                radius,
                &kiddo::distance::squared_euclidean,
            );

            for n in neighbors {
                let other_obj_id = n.item;
                if obj.obj_id != other_obj_id {
                    noncolliding.remove(other_obj_id as u32);
                }
            }
        }

        noncolliding.into_iter().map(|i| i as usize).collect()
    }

    pub fn draw(
        &mut self,
        // shared: &SharedState,
        // node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
        painter: &egui::Painter,
    ) {
        let mat = view.to_viewport_matrix(dims);

        let to_draw = self.cluster_for_draw(view, dims);

        for obj in self.annot_objs.iter_mut() {
            let p = mat * obj.label_pos.into_homogeneous_point();
            let pos = egui::pos2(p.x, p.y);

            let shape = painter.fonts(|fonts| {
                let font = egui::FontId::proportional(16.0);
                let color = egui::Color32::WHITE;
                egui::Shape::text(
                    &fonts,
                    pos,
                    egui::Align2::CENTER_CENTER,
                    &obj.label,
                    font,
                    color,
                )
            });

            let size = shape.visual_bounding_rect().size();
            obj.shape_size = Some(mint::Vector2::<f32>::from(size).into());

            // add only if no collision, for now
            if to_draw.contains(&obj.obj_id) {
                painter.add(shape);
            }
        }
    }
}
