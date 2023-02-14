use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

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

#[derive(Debug)]
struct Vertex {
    // id: VertexId,
    hubs: BTreeSet<HubId>,
    // internal_edges: Vec<Edge>,
    // interface_edges: Vec<Edge>,
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

#[derive(Debug)]
pub struct HyperSpokeGraph {
    spoke_graph: Arc<SpokeGraph>,

    // implicitly indexed by HubId
    hub_vertex_map: Vec<VertexId>,

    // implicitly indexed by VertexId
    vertices: Vec<Vertex>,
    to_delete: HashSet<VertexId>,
}

impl HyperSpokeGraph {
    pub fn vertex_count(&self) -> usize {
        self.vertices
            .len()
            .checked_sub(self.to_delete.len())
            .unwrap_or_default()
    }

    pub fn new(spoke_graph: Arc<SpokeGraph>) -> Self {
        let mut hub_vertex_map = Vec::with_capacity(spoke_graph.hub_count());
        let mut vertices = Vec::with_capacity(spoke_graph.hub_count());

        for hub_ix in 0..spoke_graph.hub_count() {
            let hub_id = HubId(hub_ix as u32);
            let vx = Vertex {
                hubs: BTreeSet::from_iter([hub_id]),
            };

            let vx_id = VertexId(vertices.len() as u32);

            hub_vertex_map.push(vx_id);
            vertices.push(vx);
        }

        Self {
            spoke_graph,
            vertices,
            hub_vertex_map,
            to_delete: HashSet::default(),
        }
    }

    // pub fn flush_deleted(&mut self) {
    //     let to_delete = std::mem::take(&mut self.to_delete);
    //     todo!();
    // }

    // merges set into a single vertex, marking the other vertices
    // as deleted and updating the HubId -> VertexId map
    pub fn merge_hub_partition(
        &mut self,
        set: impl IntoIterator<Item = HubId>,
    ) {
        let hubs = set.into_iter().collect::<Vec<_>>();
        if hubs.len() < 2 {
            return;
        }

        // hubs.sort();
        // hubs.dedup();

        let mut hubs = hubs.into_iter();

        let tgt_vx = hubs
            .next()
            .map(|hub| self.hub_vertex_map[hub.ix()])
            .unwrap();

        for hub in hubs {
            let vx = self.hub_vertex_map[hub.ix()];
            self.hub_vertex_map[hub.ix()] = tgt_vx;
            self.to_delete.insert(vx);
        }
    }

    // pub fn merge_vertices(&mut self, set: impl IntoIterator<Item = VertexId>

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

#[cfg(test)]
mod tests {
    use waragraph_core::graph::Node;

    use super::super::SpokeGraph;
    use super::*;

    #[test]
    fn merging_3ec_components() {
        let edges = super::super::tests::example_graph_edges();
        let graph = SpokeGraph::new(edges);

        let node_count = 18;

        let inverted_comps = {
            let seg_hubs = (0..node_count as u32)
                .map(|i| {
                    let node = Node::from(i);
                    let left = graph.node_endpoint_hub(node.as_reverse());
                    let right = graph.node_endpoint_hub(node.as_forward());
                    (left, right)
                })
                .filter(|(a, b)| a != b)
                .collect::<Vec<_>>();

            let tec_graph = three_edge_connected::Graph::from_edges(
                seg_hubs.into_iter().map(|(l, r)| (l.ix(), r.ix())),
            );

            let components =
                three_edge_connected::find_components(&tec_graph.graph);

            let inverted = tec_graph.invert_components(components);

            inverted
        };

        let spoke_graph = Arc::new(graph);

        let mut hyper_graph = HyperSpokeGraph::new(spoke_graph);

        for comp in inverted_comps {
            let hubs = comp.into_iter().map(|i| HubId(i as u32));
            hyper_graph.merge_hub_partition(hubs);
        }

        assert_eq!(hyper_graph.vertex_count(), 11);
    }
}
