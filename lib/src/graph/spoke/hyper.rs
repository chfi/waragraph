use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use crate::graph::{Edge, Node, OrientedNode};
use roaring::RoaringBitmap;

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
pub struct VertexId(pub u32);

impl VertexId {
    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone)]
pub struct Vertex {
    // id: VertexId,
    pub hubs: BTreeSet<HubId>,
    // internal_edges: Vec<Edge>,
    // interface_edges: Vec<Edge>,
}

#[derive(Debug, Clone)]
pub struct Cycle {
    pub endpoint: VertexId,
    pub steps: Vec<OrientedNode>,
    pub step_endpoints: Vec<VertexId>,
}

impl Cycle {
    pub fn len(&self) -> usize {
        self.steps.len()
    }
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

#[derive(Debug, Clone)]
pub struct HyperSpokeGraph {
    pub spoke_graph: Arc<SpokeGraph>,

    // implicitly indexed by HubId
    pub hub_vertex_map: Vec<VertexId>,

    // implicitly indexed by VertexId
    vertices: Vec<Vertex>,
    to_delete: HashSet<VertexId>,
}

impl HyperSpokeGraph {
    pub fn endpoint_vertex(&self, endpoint: OrientedNode) -> VertexId {
        let c = ('a' as u8 + endpoint.node().ix() as u8) as char;
        let o = if endpoint.is_reverse() { "-" } else { "+" };
        // println!("endpoint_vertex: {c}{o}");
        let hub = self.spoke_graph.endpoint_hubs[endpoint.ix()];
        self.hub_vertex_map[hub.ix()]
    }

    pub fn get_vertex(&self, v: VertexId) -> &Vertex {
        &self.vertices[v.ix()]
    }

    pub fn vertices<'a>(
        &'a self,
    ) -> impl Iterator<Item = (VertexId, &'a Vertex)> {
        self.vertices.iter().enumerate().filter_map(|(i, v)| {
            let vx_id = VertexId(i as u32);
            if self.to_delete.contains(&vx_id) {
                None
            } else {
                Some((vx_id, v))
            }
        })
    }

    pub fn dfs_preorder(
        &self,
        source: Option<VertexId>,
        mut callback: impl FnMut(usize, Option<(VertexId, OrientedNode)>, VertexId),
    ) {
        let mut visited: HashSet<VertexId> = HashSet::default();

        let mut stack: Vec<(Option<(VertexId, OrientedNode)>, VertexId)> =
            Vec::new();

        let mut count = 0;

        if let Some(src) = source {
            stack.push((None, src));
        }

        for (vx_id, _vertex) in self.vertices() {
            if stack.is_empty() {
                stack.push((None, vx_id));
            }

            while let Some((step, current)) = stack.pop() {
                if !visited.contains(&current) {
                    visited.insert(current);
                    callback(count, step, current);
                    count += 1;

                    for (step, next) in self.vertex_spokes(current) {
                        if !visited.contains(&next) {
                            stack.push((Some((current, step)), next));
                        }
                    }
                }
            }
        }
    }

    pub fn links_between_vertices(
        &self,
        a: VertexId,
        b: VertexId,
    ) -> Vec<OrientedNode> {
        // TODO this could be done better/faster, but good enough for now
        let neighbors = self.vertex_spokes(a);

        let segments = neighbors
            .filter_map(|(seg, vx)| (vx == b).then_some(seg))
            .collect::<Vec<_>>();

        segments
    }

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

            let hub = spoke_graph.hub_adj.get(hub_ix).unwrap();
            // let degree = hub.values().map(|s| s.len()).sum();

            let vx = Vertex {
                hubs: BTreeSet::from_iter([hub_id]),
                // degree,
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

    pub fn apply_deletions(&mut self) {
        let to_delete = std::mem::take(&mut self.to_delete);

        let mut next_id = 0u32;
        // let mut kept_vertices = Vec::with_capacity(self.vertices.len());
        let mut new_ids = HashMap::new();

        // walk the vertices, creating the map from old to new vertex IDs
        for (vx_id, _vertex) in self.vertices() {
            if to_delete.contains(&vx_id) {
                continue;
            }

            new_ids.insert(vx_id, VertexId(next_id));
            next_id += 1;
        }

        // update all existing vertices by applying the map
        let vertices = std::mem::take(&mut self.vertices);
        self.vertices = vertices
            .into_iter()
            .enumerate()
            .filter_map(|(i, vx)| {
                let v = VertexId(i as u32);
                (!to_delete.contains(&v)).then_some(vx)
            })
            .collect();

        for (hub_ix, vx_id) in self.hub_vertex_map.iter_mut().enumerate() {
            *vx_id = new_ids[&vx_id];
        }
    }

    pub fn contract_edge(&mut self, va: VertexId, vb: VertexId) {
        println!("va == vb: {}", va == vb);
        println!("va deleted: {}", self.to_delete.contains(&va));
        println!("vb deleted: {}", self.to_delete.contains(&vb));

        if va == vb
            || self.to_delete.contains(&va)
            || self.to_delete.contains(&vb)
        {
            return;
        }

        println!("contracting {va:?} -- {vb:?}");

        // find all the hubs that map to vertex `vb`
        let new_hubs = self.vertices[vb.ix()]
            .hubs
            .iter()
            .copied()
            .collect::<Vec<_>>();

        self.vertices[va.ix()].hubs.extend(new_hubs.iter().copied());

        for hub in new_hubs {
            self.hub_vertex_map[hub.ix()] = va;
        }

        self.to_delete.insert(vb);
    }

    // TODO: this one needs to be rewritten; the Hub indirection
    // doesn't help, and it'd be easier to ensure correctness if a
    // single function takes care of merging all of the partitions
    //
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

        let mut hubs = hubs.into_iter();

        let tgt_vx = hubs
            .next()
            .map(|hub| self.hub_vertex_map[hub.ix()])
            .unwrap();

        for hub in hubs {
            let vx = self.hub_vertex_map[hub.ix()];
            if vx != tgt_vx && !self.to_delete.contains(&vx) {
                self.hub_vertex_map[hub.ix()] = tgt_vx;
                self.to_delete.insert(vx);

                let to_add =
                    std::mem::take(&mut self.vertices[vx.0 as usize].hubs);
                self.vertices[tgt_vx.0 as usize].hubs.extend(to_add);
                self.vertices[tgt_vx.0 as usize].hubs.insert(hub);

                // TODO update cached vertex degree
            }
        }

        #[cfg(feature = "debug")]
        for (hub_ix, vx_id) in self.hub_vertex_map.iter().enumerate() {
            assert!(!self.to_delete.contains(vx_id));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::graph::Node;

    use super::super::SpokeGraph;
    use super::*;

    pub(crate) fn alt_paper_cactus_graph() -> HyperSpokeGraph {
        let edges = super::super::tests::alt_paper_graph_edges();

        let node_count = 14;
        let graph = SpokeGraph::new(node_count, edges);

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

        cactus_graph.apply_deletions();

        cactus_graph
    }

    pub(crate) fn paper_cactus_graph() -> HyperSpokeGraph {
        let edges = super::super::tests::example_graph_edges();

        let node_count = 18;
        let graph = SpokeGraph::new(node_count, edges);

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

        cactus_graph.apply_deletions();

        cactus_graph
    }

    #[test]
    fn merging_3ec_components() {
        let edges = super::super::tests::example_graph_edges();

        let node_count = 18;
        let graph = SpokeGraph::new(node_count, edges);

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

        hyper_graph.apply_deletions();

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

        assert_eq!(neighbor_count.get(&1), Some(&3));
        assert_eq!(neighbor_count.get(&3), Some(&3));
        assert_eq!(neighbor_count.get(&4), Some(&2));
        assert_eq!(neighbor_count.get(&5), Some(&2));
    }

    #[test]
    fn cactus_graph_dfs() {
        let graph = paper_cactus_graph();

        let mut visit = Vec::new();

        graph.dfs_preorder(None, |i, step, vertex| {
            visit.push((i, step, vertex))
        });

        for (i, step, vertex) in &visit {
            //
            if let Some((parent, step)) = step {
                let v = parent.0;
                let c = ('a' as u8 + step.node().ix() as u8) as char;
                let o = if step.is_reverse() { "-" } else { "+" };
                print!("{i:2} - [{v}]{c}{o},\t");
            } else {
                print!("{i:2} -        \t");
            }

            println!("{vertex:?}");
        }

        let walked = visit
            .iter()
            .filter_map(|(_, step, _)| step.map(|(_, s)| s.node()))
            .collect::<HashSet<_>>();

        let remaining_segments = (0..18)
            .map(|i| Node::from(i as u32))
            .filter(|s| !walked.contains(&s))
            .collect::<HashSet<_>>();

        let mut all_neighbors = Vec::new();
        let mut neighbor_count: HashMap<_, usize> = HashMap::default();

        for (vx_ix, vx) in graph.vertices.iter().enumerate() {
            let vx_id = VertexId(vx_ix as u32);
            if graph.to_delete.contains(&vx_id) {
                continue;
            }

            let neighbors = graph.vertex_spokes(vx_id).collect::<Vec<_>>();

            *neighbor_count.entry(neighbors.len()).or_default() += 1usize;

            all_neighbors.push((vx_id, neighbors));
        }

        // all_neighbors.sort_by_key(|(_, n)| n.len());

        for (vx_id, neighbors) in all_neighbors {
            print!("{vx_id:?}\t");
            // print!("\t");

            for (node, dst_vx) in neighbors {
                let c = ('a' as u8 + node.node().ix() as u8) as char;
                let o = if node.is_reverse() { "-" } else { "+" };
                let dst = dst_vx.0;
                print!("{c}{o}[{dst}], ");
            }
            println!();
        }

        println!();
        println!("visited segments:");

        let mut visited = walked.into_iter().collect::<Vec<_>>();
        visited.sort();

        for seg in visited {
            let c = ('a' as u8 + seg.ix() as u8) as char;
            println!("  {c}");
        }

        println!();
        println!("remaining segments:");

        let mut remaining = remaining_segments.into_iter().collect::<Vec<_>>();
        remaining.sort();

        for seg in remaining {
            let c = ('a' as u8 + seg.ix() as u8) as char;
            println!("  {c}");
        }
    }
}
