use std::{collections::BTreeSet, sync::Arc};

use waragraph_core::graph::{Edge, OrientedNode};

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
    // vertices: Vec<Vertex>,
    // vertex_adj:
}

impl HyperSpokeGraph {
    pub fn new(spoke_graph: Arc<SpokeGraph>) -> Self {
        todo!();
    }

    /*
    pub fn new_from_partitions<I, P>(
        spoke_graph: Arc<SpokeGraph>,
        hub_partitions: I,
    ) -> Self
    where
        I: IntoIterator<Item = P>,
        P: IntoIterator<Item = HubId>,
    {
        let mut vertices = Vec::new();

        // we assume iterator produces partitions
        for partition in hub_partitions {
            // combine partition into a single vertex

            let hub_ids = partition.into_iter().collect::<BTreeSet<_>>();

            let edges = hub_ids
                .iter()
                .filter_map(|hid| {
                    let hub = spoke_graph.hubs.get(hid.0 as usize)?;
                    Some(hub)
                })
                .flat_map(|hub| {
                    hub.edges.iter().map(|(from, to)| Edge::new(*from, *to))
                });

            let onode_in_hub = |onode: OrientedNode| {
                let hub = spoke_graph.node_endpoint_hub(onode);
                hub.map(|hid| hub_ids.contains(&hid)).unwrap_or(false)
            };

            let (internal_edges, interface_edges): (Vec<_>, Vec<_>) = edges
                .partition(|edge| {
                    // if both are inside the graph, it's an internal edge
                    // if only one, it's an interface edge

                    let from_inside = onode_in_hub(edge.from);
                    let to_inside = onode_in_hub(edge.to);

                    if from_inside && to_inside {
                        true
                    } else if from_inside || to_inside {
                        false
                    } else {
                        unreachable!()
                    }
                });

            let vertex_id = VertexId(vertices.len() as u32);

            vertices.push(Vertex {
                id: vertex_id,
                hubs: hub_ids,
                internal_edges,
                interface_edges,
            });
        }

        Self {
            spoke_graph,
            vertices,
        }
    }
    */
}
