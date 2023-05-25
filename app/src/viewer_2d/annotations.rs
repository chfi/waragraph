use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use egui::epaint::ahash;
use rstar::primitives::{GeomWithData, Line, Rectangle};
use rstar::RTree;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
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
    state: Arc<RwLock<AnnotationLayerState>>,

    last_view: Option<View2D>,

    to_draw_task: Option<JoinHandle<(View2D, Vec<(Arc<String>, [f32; 2])>)>>,
    to_draw_cache: Vec<(Arc<String>, [f32; 2])>,
}

impl AnnotationLayer {
    pub fn load_annotations(
        &self,
        shared: &SharedState,
        node_positions: Arc<NodePositions>,
        annot_ids: impl IntoIterator<Item = GlobalAnnotationId>,
    ) {
        let mut state = self.state.blocking_write();
        state.load_annotations(shared, &node_positions, annot_ids);
    }

    pub fn draw(
        &self,
        // shared: &SharedState,
        tokio_rt: &tokio::runtime::Handle,
        node_positions: &Arc<NodePositions>,
        view: &View2D,
        dims: Vec2,
        painter: &egui::Painter,
    ) {
        {
            let state = self.state.blocking_read();
            if state.annot_shape_sizes.len() < state.annot_objs.len() {
                let _ = state;
                let state = self.state.blocking_write();
                painter.fonts(|fonts| state.prepare_labels(fonts));
            }
        }

        if self.last_view.as_ref() != Some(view) && self.to_draw_task.is_none()
        {
            // spawn task
            let view = view.clone();

            let state = self.state.clone();
            let node_pos = node_positions.clone();

            self.to_draw_task = tokio_rt.spawn(async move {
                let visible_objs =
                    Self::reset_anchors(&state, &node_pos, view, dims).await;

                let to_draw = Self::cluster_for_draw(
                    &state,
                    &node_pos,
                    view,
                    dims,
                    &visible_objs,
                )
                .await;

                (view, to_draw)
            });

            // tokio_rt.
            // let visible_objs = self.reset_anchors(node_positions, view, dims);

            // let to_draw =
            //     self.cluster_for_draw(node_positions, view, dims, &visible_objs);
        }

        // get task results if ready

        // use latest task results to draw labels

        todo!();
        /*
        painter.fonts(|fonts| self.prepare_labels(fonts));

        let visible_objs = self.reset_anchors(node_positions, view, dims);

        let to_draw =
            self.cluster_for_draw(node_positions, view, dims, &visible_objs);

        for (obj_id, pos) in to_draw {
            let obj = &self.annot_objs[obj_id];

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
        */
    }
}

#[derive(Default)]
pub struct AnnotationLayerState {
    annot_objs: Vec<AnnotObj>,

    anchor_rtree: Option<RTree<AnchorTreeObj>>,

    // indexed by AnnotObjId
    anchor_sets: Vec<AnchorSet>,
    // active_sets: BTreeSet<AnnotationSetId>,
    annot_shape_sizes: Vec<Vec2>,
    // pub(super) pinned_annots: Arc<RwLock<HashSet<GlobalAnnotationId>>>,
}

impl AnnotationLayerState {
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
                .get(&annot_id.set_id)
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

    async fn reset_anchors(
        state_lock: &RwLock<Self>,
        // &mut self,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
    ) -> roaring::RoaringBitmap {
        use rand::prelude::*;
        use rstar::AABB;
        let mut rng = rand::thread_rng();

        let mat = view.to_viewport_matrix(dims);

        let state = state_lock.read().await;

        if state.anchor_rtree.is_none() {
            panic!("`reset_anchors` must be called after `load_annotations`");
        }

        let (x0, x1) = view.x_range();
        let (y0, y1) = view.y_range();

        let view_rect = AABB::from_corners([x0, y0], [x1, y1]);

        let rtree = state.anchor_rtree.as_ref().unwrap();

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

        let _ = state;
        let mut state = state_lock.write().await;

        let t0 = std::time::Instant::now();
        for (obj_id, anchor_cands) in visible_annots {
            let obj = &mut state.annot_objs[obj_id];

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

    async fn cluster_for_draw(
        state_lock: &RwLock<Self>,
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

        // let pinned_annots = self.pinned_annots.blocking_read().clone();

        let mut clusters: Vec<Vec<AnnotObjId>> = Vec::new();

        let state = state_lock.read().await;

        for obj in &state.annot_objs {
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

        // let mut label_rtree: RTree<Rectangle<[f32; 2]>> = RTree::new();
        let mut label_rtree: RTree<GeomWithData<Rectangle<[f32; 2]>, usize>> =
            RTree::new();

        // let mut combined_clusters: RTree<
        //     GeomWithData<Rectangle<[f32; 2]>, usize>,
        // > = RTree::new();

        // for (cl_id, objs) in clusters.into_iter().enumerate() {
        //     // we really want to iterate through all objects here?
        //     // i guess we *have* to?
        //     for obj_id in objs {
        //         //
        //     }
        // }

        for (cl_id, objs) in clusters.into_iter().enumerate() {
            for obj_id in objs.into_iter().take(1) {
                let obj = &state.annot_objs[obj_id];

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
                    let p0 = (mat * a0.into_homogeneous_point()).xy();
                    let p1 = (mat * a1.into_homogeneous_point()).xy();
                    let normal = (a1 - a0).normalized().rotated_by(rotor);
                    let normal = (mat * normal.into_homogeneous_vector())
                        .normalized()
                        .xy();

                    let pos = p0 + 0.5 * (p1 - p0);

                    let label_size = state.annot_shape_sizes[obj_id];

                    pos + normal * normal.dot(label_size) * 2.0
                };

                let label_size = state.annot_shape_sizes[obj_id];

                let p0 = label_pos - label_size / 2.0;
                let p1 = label_pos + label_size / 2.0;

                let aabb = AABB::from_corners(p0.into(), p1.into());

                let overlapping_cluster = label_rtree
                    .locate_in_envelope_intersecting(&aabb)
                    .next()
                    .copied();

                if let Some(other) = overlapping_cluster {
                    // add to existing cluster
                } else {
                    // todo don't push to to_draw here; do it one in a final stage after this loop
                    to_draw.push((obj_id, *label_pos.as_array()));
                    let rect = Rectangle::from_corners(p0.into(), p1.into());
                    let value = GeomWithData::new(rect, cl_id);
                    label_rtree.insert(value);
                }
            }
        }

        // println!("to_draw.len() {}", to_draw.len());

        to_draw
    }
}

type ClusterId = usize;

struct Clusters {
    anchor_kdtree: kiddo::KdTree<f32, 2>,
    label_rtree: RTree<GeomWithData<Rectangle<[f32; 2]>, ClusterId>>,

    clusters: Vec<Cluster>,

    cluster_radius: f32,
}

struct Cluster {
    // aabb:
    annotations: Vec<AnnotObjId>,
}

impl Clusters {
    fn cluster_for_view_<'a>(
        &mut self,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
        // object with size, kinda messy but refactor later
        annot_objs: impl Iterator<Item = (&'a AnnotObj, Vec2)>,
        force_visible: bool,
    ) {
        use rstar::AABB;

        let mat = view.to_viewport_matrix(dims);

        for (obj, label_size) in annot_objs {
            let label_pos = {
                let rotor = Rotor2::from_rotation_between(
                    Vec2::unit_y(),
                    Vec2::unit_x(),
                )
                .normalized();

                let (a0, a1) = node_positions.node_pos(obj.anchor_node);
                let p0 = (mat * a0.into_homogeneous_point()).xy();
                let p1 = (mat * a1.into_homogeneous_point()).xy();
                let normal = (a1 - a0).normalized().rotated_by(rotor);
                let normal =
                    (mat * normal.into_homogeneous_vector()).normalized().xy();

                let pos = p0 + 0.5 * (p1 - p0);

                pos + normal * normal.dot(label_size) * 2.0
            };

            let p0 = label_pos - label_size / 2.0;
            let p1 = label_pos + label_size / 2.0;

            let aabb = AABB::from_corners(p0.into(), p1.into());

            // let overlaps = self.label_rtree.pop_nearest_neighbor(
            // let overlaps = self.label_rtree.drain_in_envelope(

            // let mut overlaps_iter =

            let overlapping = self
                .label_rtree
                .locate_in_envelope_intersecting(&aabb)
                .next()
                .copied();

            if let Some(cl_id) = overlapping {
                // if this overlaps with any existing label, try to insert
                // this label as a rectangle with the same cluster ID in
                // the appropriate location
            } else {
                // if there's no overlap create a new cluster at this position
                let cl_id = self.clusters.len();
                self.clusters.push(Cluster {
                    annotations: vec![obj.obj_id],
                });
                let rect = Rectangle::from_aabb(aabb);
                self.label_rtree.insert(GeomWithData::new(rect, cl_id));
            }

            //// if the *new* label location, as a child of a new or
            //// existing cluster, would overlap with any inserted label,
            //// we can try to remove either one of them

            // also store the label and position in the cluster (we
            // want to be able to go both ways)
        }

        todo!();
    }

    fn add_label(
        &mut self,
        mat: ultraviolet::Mat3,
        obj: AnnotObj,
        size: Vec2,
        pinned: bool,
    ) {
        //

        //

        //

        //

        //

        todo!();
    }

    fn cluster_for_view<'a>(
        &mut self,
        node_positions: &NodePositions,
        view: &View2D,
        dims: Vec2,
        // object with size, kinda messy but refactor later
        annot_objs: impl Iterator<Item = (&'a AnnotObj, Vec2)>,
        force_visible: bool,
    ) {
        let mat = view.to_viewport_matrix(dims);

        let mut seen_annot_objs: ahash::HashSet<AnnotObjId> =
            ahash::HashSet::default();

        // self.anchor_kdtree.clear();

        for (obj, size) in annot_objs {
            let pos = (mat * obj.anchor_pos.into_homogeneous_point()).xy();

            if pos.x.is_nan() || pos.y.is_nan() {
                continue;
            }

            seen_annot_objs.insert(obj.obj_id);

            // nearest_one doesn't return Option so need to check here
            if self.anchor_kdtree.size() == 0 {
                // just add the cluster and continue
                self.add_cluster(obj.obj_id, pos);
            }

            let (dist, cl_id) = self.anchor_kdtree.nearest_one(
                pos.as_array(),
                &kiddo::distance::squared_euclidean,
            );

            if dist > self.cluster_radius {
                // create a new cluster
                self.add_cluster(obj.obj_id, pos);
            } else {
                // append to the existing cluster

                self.clusters[cl_id].annotations.push(obj.obj_id);
            }
        }

        self.label_rtree = RTree::new();

        for cl_id in 0..self.clusters.len() {
            // let cluster = &self.clusters[cl_id];

            //

            // we need to create the AABBs for the cluster, but we
            // can't know how many labels we actually want to draw yet...
            //

            //

            // maybe do this bit later, but:
            // sort annot objs in cluster by global annot id,
            // except if force_visible is on, sort the annot objs present
            // in seen_annot_objs such that they are first in each cluster
            // (and make sure they are visible)
        }

        todo!();
    }

    // or maybe return galleys? or shapes, idk
    // fn to_draw<'a>(
    //     &'a self,
    // ) -> impl Iterator<Item = (AnnotObjId, egui::Pos2)> + 'a {
    //     todo!();
    // }

    fn add_cluster(&mut self, obj_id: AnnotObjId, pos: Vec2) -> ClusterId {
        let cl_id = self.clusters.len();
        self.clusters.push(Cluster {
            annotations: vec![obj_id],
        });
        self.anchor_kdtree.add(pos.as_array(), cl_id);
        cl_id
    }
}
