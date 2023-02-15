use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use waragraph_core::graph::{Edge, Node, OrientedNode};

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
pub struct VertexId(u32);

#[derive(Debug)]
pub struct Vertex {
    // id: VertexId,
    hubs: BTreeSet<HubId>,
    // internal_edges: Vec<Edge>,
    // interface_edges: Vec<Edge>,
}

#[derive(Debug, Clone)]
pub struct Cycle {
    pub endpoint: VertexId,
    pub steps: Vec<OrientedNode>,
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
    pub fn vertex_spokes<'a>(
        &'a self,
        vertex: VertexId,
    ) -> impl Iterator<Item = (OrientedNode, VertexId)> + 'a {
        if self.to_delete.contains(&vertex) {
            panic!("Can't find the neighbors of a deleted node");
        }

        let vx = &self.vertices[vertex.0 as usize];

        // this looks a little ridiculous, but it's just projecting to
        // get all the segments from this vertex by looking at the
        // underlying spokegraph
        vx.hubs
            .iter()
            .filter_map(|hub| self.spoke_graph.hub_adj.get(hub.ix()))
            .flat_map(|neighbors| {
                neighbors.iter().flat_map(|(other_hub, nodes)| {
                    let dst_vx = self.hub_vertex_map[other_hub.ix()];
                    nodes.iter().map(move |n| (*n, dst_vx))
                })
            })
    }

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
}

/// Returns the cycles in a cactus graph as a sequence of segment
/// traversals. The graph must be a cactus graph.
pub fn find_cactus_graph_cycles(graph: &HyperSpokeGraph) -> Vec<Cycle> {
    let mut visited: HashSet<VertexId> = HashSet::default();
    let mut parents: HashMap<VertexId, (OrientedNode, VertexId)> =
        HashMap::default();

    let mut stack: Vec<VertexId> = Vec::new();

    let mut cycles = Vec::new();
    let mut cycle_ends: Vec<(VertexId, VertexId)> = Vec::new();

    let mut neighbors = Vec::new();

    for (vx_ix, vertex) in graph.vertices.iter().enumerate() {
        let vx_id = VertexId(vx_ix as u32);
        if visited.contains(&vx_id) || graph.to_delete.contains(&vx_id) {
            continue;
        }

        stack.push(vx_id);
        while let Some(current) = stack.pop() {
            if !visited.contains(&current) {
                visited.insert(current);

                neighbors.clear();
                neighbors.extend(graph.vertex_spokes(vx_id));

                for &(segment, adj) in neighbors.iter() {
                    if adj == current {
                        let cycle = Cycle {
                            endpoint: current,
                            steps: vec![segment],
                        };
                        cycles.push(cycle);
                    } else if !visited.contains(&adj) {
                        stack.push(adj);
                        parents.insert(adj, (segment, current));
                    } else if parents.get(&current).map(|(_, v)| v)
                        != Some(&adj)
                    {
                        cycle_ends.push((adj, current));
                    }
                }
            }
        }
    }

    for (start, end) in cycle_ends {
        let mut cycle_steps: Vec<OrientedNode> = vec![];
        let mut current = end;

        while current != start {
            if let Some((step, parent)) = parents.get(&current) {
                // cycle_steps.push((current, *parent));
                cycle_steps.push(*step);
                current = *parent;
            }
        }

        let cycle = Cycle {
            endpoint: start,
            steps: cycle_steps,
        };

        cycles.push(cycle);
    }

    cycles
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

        let mut all_neighbors = Vec::new();
        let mut neighbor_count: HashMap<_, usize> = HashMap::default();

        for (vx_ix, vx) in hyper_graph.vertices.iter().enumerate() {
            let vx_id = VertexId(vx_ix as u32);
            if hyper_graph.to_delete.contains(&vx_id) {
                continue;
            }

            let neighbors =
                hyper_graph.vertex_spokes(vx_id).collect::<Vec<_>>();

            *neighbor_count.entry(neighbors.len()).or_default() += 1usize;

            all_neighbors.push((vx_id, neighbors));
        }

        assert_eq!(neighbor_count.get(&1), Some(&3));
        assert_eq!(neighbor_count.get(&3), Some(&4));
        assert_eq!(neighbor_count.get(&4), Some(&2));
        assert_eq!(neighbor_count.get(&5), Some(&2));

        all_neighbors.sort_by_key(|(_, n)| n.len());

        for (vx_id, neighbors) in all_neighbors {
            println!("{vx_id:?}");
            print!("\t");

            for (node, dst_vx) in neighbors {
                let c = ('a' as u8 + node.node().ix() as u8) as char;
                let o = if node.is_reverse() { "-" } else { "+" };
                let dst = dst_vx.0;
                print!("{c}{o}[{dst}], ");
            }
            println!();
        }
    }

    #[test]
    fn find_cycles() {
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

        let mut cactus_graph = HyperSpokeGraph::new(spoke_graph);

        for comp in inverted_comps {
            let hubs = comp.into_iter().map(|i| HubId(i as u32));
            cactus_graph.merge_hub_partition(hubs);
        }

        todo!();
    }
}
