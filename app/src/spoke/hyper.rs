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

/*

the idea is to allow the merging of a spoke graph's vertices by
providing a *partition of the vertices* (e.g. a function that maps
each `HubId` to its vertex partition identifier)

each vertex in the new graph will represent a *subgraph* of the input
graph, such that
- all edges in the original graph that go between two
nodes that are both inside the vertex are the "internal" edges, and
- the edges where only one of the nodes are inside the vertex are the
"interface" edges

the interface edges are those that link an "external" node (i.e. an
*oriented segment*) with a node/segment (an edge in the underlying
`SpokeGraph`) contained within the vertex' subgraph.

by providing the 3-edge-connected equivalence classes (NB: must be
computed using an algorithm that supports multigraphs, which
rs-3-edge doesn't) the result is a cactus graph


we can find chain pairs by looking at the projection from the original
graph (a la the paper), and we can create the bridge forest by providing
a different partition, giving the bridge pairs

walks through these graphs should give the snarl hierarchy, and the
graph construction makes it easy to find which subpaths (as sequences
of steps) map to which "hypervertex" (as each such vertex is both a
partition/subgraph of the original graph, and consists of a set of
edges, which can be used to generate a sequence of steps)



*/

pub struct HyperSpokeGraph {
    spoke_graph: Arc<SpokeGraph>,

    vertices: Vec<Vertex>,
    // vertex_adj:
}

impl HyperSpokeGraph {
    pub fn new<I, P>(spoke_graph: Arc<SpokeGraph>, hub_partitions: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: IntoIterator<Item = HubId>,
    {
        for partition in hub_partitions {
            // assume iterator produces a partition, or check?
            for hub_id in partition {
                //
            }
        }

        todo!();
    }
}
