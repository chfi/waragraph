use std::{
    num::NonZeroI32,
    ops::{Add, Div, Sub},
};

use euclid::*;
use nalgebra::{Norm, Normed};
use num_traits::{FromPrimitive, ToPrimitive};
use raving::compositor::Compositor;
use rustc_hash::{FxHashMap, FxHashSet};

use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::{BufferIx, DescSetIx, GpuResources, VkEngine};

use ash::vk;

use std::sync::Arc;

use anyhow::Result;

use ndarray::prelude::*;

use crate::graph::{Node, Path, Waragraph};

use super::{
    graph::OrientedNode, ScreenPoint, ScreenRect, ScreenSize, ScreenSpace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurveId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrientedCurve(NonZeroI32);

impl From<OrientedCurve> for CurveId {
    fn from(oc: OrientedCurve) -> CurveId {
        CurveId((oc.0.get().abs() - 1) as u32)
    }
}

impl From<CurveId> for OrientedCurve {
    fn from(CurveId(id): CurveId) -> OrientedCurve {
        let id = unsafe { NonZeroI32::new_unchecked(1 + id as i32) };
        OrientedCurve(id)
    }
}

impl OrientedCurve {
    pub fn id(&self) -> CurveId {
        CurveId((self.0.get() - 1) as u32)
    }

    pub fn is_reverse(&self) -> bool {
        // self.0.get().is_negative()
        self.0.leading_zeros() == 0
    }

    pub fn new(node_id: u32, is_reverse: bool) -> Self {
        let mut id = 1 + node_id as i32;

        if is_reverse {
            id *= -1;
        }

        let id = unsafe { NonZeroI32::new_unchecked(id) };

        Self(id)
    }

    pub fn flip(self) -> Self {
        let id = unsafe { NonZeroI32::new_unchecked(self.0.get() * -1) };

        Self(id)
    }
}

#[derive(Default, Clone)]
pub struct NodeCurveNeighbors<T> {
    forward: Option<T>,
    backward: Option<T>,
}

pub struct Curve<D> {
    id: CurveId,

    seq_len: usize,

    steps: Vec<OrientedNode>,

    node_set: roaring::RoaringBitmap,
    path_set: roaring::RoaringBitmap,

    fwd_adj: Vec<OrientedCurve>,
    rev_adj: Vec<OrientedCurve>,

    step_data: Vec<D>,
}

impl<D> Curve<D> {
    pub fn new(id: CurveId) -> Self {
        Curve {
            id,

            seq_len: 0,

            steps: Vec::new(),

            node_set: Default::default(),
            path_set: Default::default(),

            fwd_adj: Vec::new(),
            rev_adj: Vec::new(),

            step_data: Vec::new(),
        }
    }

    pub fn steps(&self) -> &[OrientedNode] {
        &self.steps
    }

    pub fn paths(&self) -> impl Iterator<Item = Path> + '_ {
        self.path_set.iter().map(|i| Path::from(i))
    }

    pub fn contains_node(&self, node: Node) -> bool {
        self.node_set.contains(node.ix() as u32)
    }

    pub fn contains_path(&self, path: Path) -> bool {
        self.path_set.contains(path.ix() as u32)
    }

    pub fn first(&self) -> Option<OrientedNode> {
        self.steps.first().copied()
    }

    pub fn last(&self) -> Option<OrientedNode> {
        self.steps.last().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn seq_len(&self) -> usize {
        self.seq_len
    }
}

fn split_path_loops(
    graph: &Waragraph,
    path: Path,
) -> (Vec<Vec<OrientedNode>>, Vec<usize>) {
    let mut segments: Vec<Vec<OrientedNode>> = Vec::new();
    let mut segment_order: Vec<usize> = Vec::new();

    let mut seen_nodes: FxHashSet<Node> = FxHashSet::default();
    // let mut seen_steps: FxHashSet<OrientedNode> = FxHashSet::default();
    //
    for &strand in graph.path_steps[path.ix()].iter() {
        let step = OrientedNode::new(strand.node().into(), strand.is_reverse());
        let node = step.node();

        todo!();

        if seen_nodes.contains(&node) {
            //
        } else {
            //
        }
    }

    (segments, segment_order)
}

pub struct GraphCurveMap {
    source: Arc<Waragraph>,

    curves: Vec<Curve<()>>,

    node_curve_map: FxHashMap<Node, CurveId>,
    // path_curve_map: FxHashMap<Path, FxHashMap<
}

impl GraphCurveMap {
    pub fn new(
        graph: &Arc<Waragraph>,
        // paths: impl IntoIterator<Item = Path>,
        base_path: Path,
    ) -> Result<GraphCurveMap> {
        let mut curve_map = GraphCurveMap {
            source: graph.clone(),
            curves: Vec::new(),
            node_curve_map: FxHashMap::default(),
        };

        let mut id = CurveId(curve_map.curves.len() as u32);

        let mut curve = Curve::new(id);

        // let mut seen_nodes = jjjj

        for &strand in graph.path_steps[base_path.ix()].iter() {
            //
            //

            curve_map.curves.push(curve);
            id = CurveId(curve_map.curves.len() as u32);
            curve = Curve::new(id);
        }

        // for path in paths {
        //     curve_map.append_path(path)?;
        // }

        Ok(curve_map)
    }
}

// pub struct CurveNet {
// curves:
// }
