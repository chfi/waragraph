use std::collections::BTreeSet;

use ultraviolet::Vec2;
use waragraph_core::graph::Node;

use crate::{
    annotations::{AnnotationSetId, GlobalAnnotationId},
    app::SharedState,
};

use super::layout::NodePositions;

type AnnotObjId = usize;

struct AnnotObj {
    obj_id: AnnotObjId,
    annot_id: GlobalAnnotationId,

    anchor_pos: Vec2,
    label_pos: Vec2,

    shape_size: Option<Vec2>,
}

struct AnchorSet {
    nodes: BTreeSet<Node>,
}

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
        for a_id in annot_ids {
            let obj_id = self.annot_objs.len();

            // step through annot. path range to build anchor set

            // initialize anchor pos to middle of random node in set

            // initialize label pos some distance out from the anchor
            // pos, along the node's normal

            // shape size can't be set before rendering
        }

        todo!();
    }
}
