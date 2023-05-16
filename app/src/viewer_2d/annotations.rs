use std::collections::BTreeSet;

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

            let anchor_set = AnchorSet {
                nodes: anchor_nodes.collect(),
            };

            // initialize anchor pos to middle of random node in set

            let anchor_node =
                anchor_set.nodes.iter().choose(&mut rng).copied().unwrap();
            let (a0, a1) = node_positions.node_pos(anchor_node);
            let da = a1 - a0;
            let anchor_pos = a0 + 0.5 * da;

            // initialize label pos some distance out from the anchor
            // pos, along the node's normal

            let label_pos = {
                let rotor = Rotor2::from_rotation_between(
                    Vec2::unit_x(),
                    Vec2::unit_y(),
                )
                .normalized();
                let normal = da.normalized().rotated_by(rotor);

                anchor_pos + normal * 80.0
            };

            let obj = AnnotObj {
                obj_id,
                annot_id,
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

    pub fn render(
        &mut self,
        shared: &SharedState,
        node_positions: &NodePositions,
        view: &View2D,
        painter: &mut egui::Painter,
    ) {
        todo!();
    }
}
