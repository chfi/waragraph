use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use crate::graph::{Edge, Node, OrientedNode, PathIndex};
use reunion::{UnionFind, UnionFindTrait};

pub mod hyper;
// pub mod matrix;

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

#[derive(Debug)]
pub struct SpokeGraph {
    // implicitly indexed by HubId
    hub_adj: Vec<BTreeMap<HubId, Vec<OrientedNode>>>,
    hub_endpoints: Vec<BTreeSet<OrientedNode>>,

    // implicitly indexed by OrientedNode
    endpoint_hubs: Vec<HubId>,

    max_endpoint: OrientedNode,
}

impl SpokeGraph {
    pub fn hub_count(&self) -> usize {
        self.hub_adj.len()
    }

    pub fn node_endpoint_hub(&self, node_endpoint: OrientedNode) -> HubId {
        self.endpoint_hubs[node_endpoint.ix()]
    }

    pub fn new_from_graph(graph: &PathIndex) -> Self {
        let seg_count = graph.node_count;
        Self::new(seg_count, graph.edges_iter().copied())
    }

    pub fn map_edge(&self, edge: Edge) -> HubId {
        let (l, _r) = edge.endpoints();
        let hub = self.endpoint_hubs[l.ix()];
        hub
    }

    pub fn new(
        segment_count: usize,
        edges: impl IntoIterator<Item = Edge>,
    ) -> Self {
        let mut end_ufind = UnionFind::<OrientedNode>::new();

        for edge in edges {
            let (a, b) = edge.endpoints();
            end_ufind.union(a, b);
        }

        let mut next_hub = 0u32;

        let mut rep_end_hub_map: HashMap<OrientedNode, HubId> =
            HashMap::default();

        for segment in 0..segment_count {
            let node = Node::from(segment);
            let fwd = node.as_forward();
            let f_rep = end_ufind.find(fwd);
            if !rep_end_hub_map.contains_key(&f_rep) {
                let hub_id = HubId(next_hub);
                rep_end_hub_map.insert(f_rep, hub_id);
                next_hub += 1;
            }

            let rev = node.as_reverse();
            let r_rep = end_ufind.find(rev);
            if !rep_end_hub_map.contains_key(&r_rep) {
                let hub_id = HubId(next_hub);
                rep_end_hub_map.insert(r_rep, hub_id);
                next_hub += 1;
            }
        }

        let hub_count = next_hub as usize;

        let mut hub_adj: Vec<BTreeMap<HubId, Vec<OrientedNode>>> =
            vec![BTreeMap::new(); hub_count];
        let mut hub_endpoints: Vec<BTreeSet<OrientedNode>> =
            vec![BTreeSet::new(); hub_count];

        let endpoints = (0..segment_count).flat_map(|seg| {
            let node = Node::from(seg);
            let fwd = node.as_forward();
            let rev = node.as_reverse();
            [fwd, rev]
        });

        let mut endpoint_hubs = Vec::with_capacity(segment_count * 2);

        let mut max_endpoint = OrientedNode::from(0);

        // for segment in 0..segment_count {
        for endpoint in endpoints {
            max_endpoint = max_endpoint.max(endpoint);

            let rep = end_ufind.find(endpoint);
            let hub = rep_end_hub_map.get(&rep).unwrap();

            endpoint_hubs.push(*hub);

            let adj = &mut hub_adj[hub.ix()];
            let endpoint_set = &mut hub_endpoints[hub.ix()];

            // add this endpoint to the hub
            endpoint_set.insert(endpoint);

            // add an edge for each segment
            let op_rep = end_ufind.find(endpoint.flip());
            let op_hub = rep_end_hub_map.get(&op_rep).unwrap();

            adj.entry(*op_hub).or_default().push(endpoint);
        }

        // shouldn't be necessary, but~~
        for adj in hub_adj.iter_mut() {
            for ends in adj.values_mut() {
                ends.sort();
                ends.dedup();
            }
        }

        Self {
            hub_adj,
            hub_endpoints,
            endpoint_hubs,
            max_endpoint,
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

#[cfg(test)]
mod tests {

    use super::*;

    use waragraph_core::graph::Edge;

    // corresponds to the graph in fig 3A in the paper
    pub(crate) fn example_graph_edges() -> Vec<Edge> {
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

    // corresponds to the graph in fig 5A in the paper
    pub(crate) fn alt_paper_graph_edges() -> Vec<Edge> {
        let oriented_node = |c: char, rev: bool| -> OrientedNode {
            let node = (c as u32) - 'a' as u32;
            OrientedNode::new(node, rev)
        };

        let edge = |s: &str| -> Edge {
            let chars = s.chars().collect::<Vec<_>>();
            let a = chars[0];
            let a_rev = chars[1] == '-';
            let b = chars[2];
            let b_rev = chars[3] == '-';

            Edge::new(oriented_node(a, a_rev), oriented_node(b, b_rev))
        };

        let edges = [
            "a+n+", //
            "a+b+", //
            "b+c+", "b+d+", "c+e+", "d+e+", //
            "e+f+", "e+g+", "f+h+", "g+h+", //
            "h+m+", "h+i+", //
            "i+j+", "i+k+", "j+l+", "k+l+", //
            "l+m+", //
            "m+n+",
        ]
        .into_iter()
        .map(edge)
        .collect::<Vec<_>>();

        /*
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
        */

        edges
    }

    #[test]
    fn spoke_graph_construction() {
        // TODO: test this more thoroughly; the current implementation
        // produces different internal IDs each run, so right now the
        // only thing this tests is that all node endpoints are mapped

        let edges = example_graph_edges();
        let node_count = 18;
        let graph = SpokeGraph::new(node_count, edges);

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

        let node_count = 18;
        let graph = SpokeGraph::new(node_count, edges);

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
