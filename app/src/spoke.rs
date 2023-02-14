use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use reunion::{UnionFind, UnionFindTrait};
use waragraph_core::graph::{Edge, Node, OrientedNode, PathIndex};

pub mod app;

pub mod hyper;

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

impl HubId {
    pub fn ix(&self) -> usize {
        self.0 as usize
    }
}

pub struct SpokeGraph {
    // implicitly indexed by HubId
    hub_adj: Vec<BTreeMap<HubId, Vec<OrientedNode>>>,
    hub_endpoints: Vec<HashSet<OrientedNode>>,

    // implicitly indexed by OrientedNode
    endpoint_hubs: Vec<HubId>,
}

impl SpokeGraph {
    pub fn hub_count(&self) -> usize {
        self.hub_adj.len()
    }

    pub fn node_endpoint_hub(&self, node_endpoint: OrientedNode) -> HubId {
        self.endpoint_hubs[node_endpoint.ix()]
    }

    pub fn new_from_graph(graph: &PathIndex) -> Self {
        Self::new(graph.edges_iter().map(|&(a, b)| Edge::new(a, b)))
    }

    pub fn map_edge(&self, edge: Edge) -> HubId {
        let (l, _r) = edge.endpoints();
        let hub = self.endpoint_hubs[l.ix()];
        hub
    }

    pub fn new(edges: impl IntoIterator<Item = Edge>) -> Self {
        let mut end_ufind = UnionFind::<OrientedNode>::new();

        let mut max_end = usize::MIN;

        for edge in edges {
            let (a, b) = edge.endpoints();
            max_end = max_end.max(a.node().ix().max(b.node().ix()));
            end_ufind.union(a, b);
        }

        let mut rep_end_hub_map: HashMap<OrientedNode, HubId> =
            HashMap::default();

        let mut hub_adj: Vec<BTreeMap<HubId, Vec<OrientedNode>>> = Vec::new();

        let partitions = end_ufind.subsets();

        for (hub_ix, set) in partitions.iter().enumerate() {
            let hub_id = HubId(hub_ix as u32);

            for &node_end in set.iter() {
                rep_end_hub_map.insert(node_end, hub_id);
            }

            let hub = BTreeMap::default();
            hub_adj.push(hub);
        }

        let mut to_add = Vec::new();

        for (hub_ix, hub_map) in hub_adj.iter_mut().enumerate() {
            let this_set = &partitions[hub_ix];

            let mut neighbors = Vec::new();

            for &endpoint in this_set.iter() {
                //
                let other_end = endpoint.flip();
                let rep = end_ufind.find(other_end);

                if let Some(hub_id) = rep_end_hub_map.get(&rep) {
                    neighbors.push((other_end, *hub_id));
                } else {
                    to_add.push((hub_ix, other_end));
                }
            }

            for (other_end, other_hub) in neighbors {
                hub_map.entry(other_hub).or_default().push(other_end);
            }
        }

        let mut hub_endpoints = partitions;

        for (other_hub_ix, this_end) in to_add {
            let hub_id = HubId(hub_adj.len() as u32);

            let other_hub = HubId(other_hub_ix as u32);

            let mut this_hub = BTreeMap::default();

            this_hub.insert(other_hub, vec![this_end.flip()]);
            hub_adj.push(this_hub);

            hub_adj[other_hub_ix]
                .entry(hub_id)
                .or_default()
                .push(this_end);

            rep_end_hub_map.insert(this_end, hub_id);

            hub_endpoints.push(HashSet::from_iter([this_end]));
        }

        for hub_map in hub_adj.iter_mut() {
            for nodes in hub_map.values_mut() {
                nodes.sort();
            }
        }

        let mut endpoint_hubs: Vec<(OrientedNode, HubId)> =
            rep_end_hub_map.into_iter().collect();
        endpoint_hubs.sort_by_key(|(n, _)| *n);

        let endpoint_hubs =
            endpoint_hubs.into_iter().map(|(_node, hub)| hub).collect();

        Self {
            hub_adj,
            hub_endpoints,
            endpoint_hubs,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Hub {
    // the set of edges that is mapped to this hub
    pub edges: BTreeSet<(OrientedNode, OrientedNode)>,

    // [(a+, b+)] would map to the spokes [a+, b-]
    pub spokes: Vec<OrientedNode>,
    // node_spoke_map: BTreeMap<Node, usize>,
    pub adj_hubs: Vec<HubId>,
}

impl Hub {
    pub fn edges<'a>(
        &'a self,
    ) -> impl Iterator<Item = (OrientedNode, OrientedNode)> + 'a {
        self.edges.iter().copied()
    }
}

pub struct SpokeLayout {
    graph: SpokeGraph,
    geometry: HubSpokeGeometry,
}

type HubSpokeGeometry = HubSpokeData<f32, f32>;

struct HubSpokeData<Node, Edge> {
    // implicitly indexed by NodeId
    node_data: Vec<Node>,

    // outer Vec implicitly indexed by HubId,
    // inner Vec corresponds to `Hub`s `spokes` field
    hub_spoke_data: Vec<Vec<Edge>>,
}

#[cfg(test)]
mod tests {

    use super::*;

    use waragraph_core::graph::Edge;

    // corresponds to the graph in fig 3A in the paper
    fn example_graph_edges() -> Vec<Edge> {
        let oriented_node = |c: char, rev: bool| -> OrientedNode {
            let node = (c as u32) - 'a' as u32;
            OrientedNode::new(node, rev)
        };

        let edge = |a: char, a_r: bool, b: char, b_r: bool| -> Edge {
            let a = oriented_node(a, a_r);
            let b = oriented_node(b, b_r);
            Edge::new(a, b)
        };

        let edges = [
            ('a', 'b'),
            ('a', 'c'),
            ('b', 'd'),
            ('c', 'd'),
            ('d', 'e'),
            ('d', 'f'),
            ('e', 'g'),
            ('f', 'g'),
            ('f', 'h'),
            ('g', 'k'),
            ('g', 'l'),
            ('h', 'i'),
            ('h', 'j'),
            ('i', 'j'),
            ('j', 'l'),
            ('k', 'l'),
            ('l', 'm'),
            ('m', 'n'),
            ('m', 'o'),
            ('n', 'p'),
            ('o', 'p'),
            ('p', 'm'),
            ('p', 'q'),
            ('p', 'r'),
        ]
        .into_iter()
        .map(|(a, b)| edge(a, false, b, false))
        .collect::<Vec<_>>();

        edges
    }

    #[test]
    fn spoke_graph_construction() {
        // TODO: test this more thoroughly; the current implementation
        // produces different internal IDs each run, so right now the
        // only thing this tests is that all node endpoints are mapped

        let edges = example_graph_edges();
        let graph = SpokeGraph::new(edges);

        println!("hub adj");
        for (hub_ix, hub) in graph.hub_adj.iter().enumerate() {
            println!("{hub_ix}");
            for (o_id, nodes) in hub {
                print!("{o_id:?}: ");
                for node in nodes {
                    let c = ('a' as u8 + node.node().ix() as u8) as char;
                    let o = if node.is_reverse() { "-" } else { "+" };
                    print!("{c}{o}, ");
                }
                println!();
            }
        }

        println!();

        println!("node -> (hub, hub) map");
        for node_ix in 0..18 {
            let node = Node::from(node_ix as u32);

            let c = ('a' as u8 + node_ix as u8) as char;

            let left = graph.node_endpoint_hub(node.as_reverse());
            let right = graph.node_endpoint_hub(node.as_forward());

            println!("{c} => {left:?}\t{right:?}");
        }
    }

    #[test]
    fn spoke_graph_3ec() {
        let edges = example_graph_edges();
        let graph = SpokeGraph::new(edges);

        let node_count = 18;

        println!("hub adj");
        for (hub_ix, hub) in graph.hub_adj.iter().enumerate() {
            println!("{hub_ix}");
            for (o_id, nodes) in hub {
                print!("{o_id:?}: ");
                for node in nodes {
                    let c = ('a' as u8 + node.node().ix() as u8) as char;
                    let o = if node.is_reverse() { "-" } else { "+" };
                    print!("{c}{o}, ");
                }
                println!();
            }
        }

        println!();

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

        // rs-3-edge maps the vertex IDs to a compact usize range
        // (since vertex IDs can be any type), even with the
        // (usize, usize) iterator from_edges() constructor, so
        // the output from `find_components` can't be used directly
        // `invert_components` also filters out all singleton components
        assert_eq!(inverted_comps.len(), 1);
        assert_eq!(inverted_comps[0].len(), 2);
    }
}
