use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use waragraph_core::graph::{Node, OrientedNode, PathIndex};

pub mod app;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
#[repr(transparent)]
pub struct HubId(u32);

pub struct SpokeLayout {
    graph: Arc<PathIndex>,

    node_hub_map: BTreeMap<Node, (Option<HubId>, Option<HubId>)>,

    hubs: Vec<Hub>,

    geometry: HubSpokeGeometry,
}

struct HubSpokeGeometry {
    // implicitly indexed by NodeId
    node_lengths: Vec<f32>,

    // outer Vec implicitly indexed by HubId,
    // inner Vec corresponds to `Hub`s `spokes` field
    hub_spoke_angles: Vec<Vec<f32>>,
}

pub struct Hub {
    // the set of edges that is mapped to this hub
    edges: BTreeSet<(OrientedNode, OrientedNode)>,

    // spokes are given clockwise, and map to the corresponding node endpoint, i.e.
    // [(a+, b+)] would map to the spokes [a+, b-]
    spokes: Vec<OrientedNode>,

    node_spoke_map: BTreeMap<Node, usize>,
}
