use std::{collections::BTreeSet, sync::Arc};

use waragraph_core::graph::Edge;

use super::{HubId, SpokeGraph};

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
struct VertexId(u32);

struct Vertex {
    id: VertexId,

    hubs: BTreeSet<HubId>,
    internal_edges: Vec<Edge>,
    interface_edges: Vec<Edge>,
}

// hypergraph "wrapper" over SpokeGraph
pub struct HyperSpokeGraph {
    spoke_graph: Arc<SpokeGraph>,

    vertices: Vec<Vertex>,
    // vertex_adj:
}
