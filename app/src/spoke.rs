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

pub struct Graph {
    hub_adj: Vec<BTreeMap<HubId, Vec<OrientedNode>>>,

    // endpoint_hub_map: HashMap<OrientedNode, HubId>,
    // implicitly indexed by OrientedNode
    endpoint_hubs: Vec<HubId>,
}

impl Graph {
    pub fn node_endpoint_hub(&self, node_endpoint: OrientedNode) -> HubId {
        self.endpoint_hubs[node_endpoint.ix()]
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

            // let neighbors = this_set
            //     .iter()
            //     .map(|end| end.flip())
            //     .filter_map(|other_end| {
            //         let rep = end_ufind.find(other_end);
            //         let hub_id = *rep_end_hub_map.get(&rep)?;
            //         Some((other_end, hub_id))
            //     })
            //     .collect::<Vec<_>>();

            for (other_end, other_hub) in neighbors {
                hub_map.entry(other_hub).or_default().push(other_end);
            }
        }

        for (other_hub_ix, this_end) in to_add {
            let hub_id = HubId(hub_adj.len() as u32);

            let other_hub = HubId(other_hub_ix as u32);

            let mut this_hub = BTreeMap::default();

            this_hub.insert(other_hub, vec![this_end]);
            hub_adj.push(this_hub);

            hub_adj[other_hub_ix]
                .entry(hub_id)
                .or_default()
                .push(this_end.flip());

            rep_end_hub_map.insert(this_end, hub_id);
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
            // endpoint_hub_map: rep_end_hub_map,
            endpoint_hubs,
        }
    }
}

pub struct SpokeGraph {
    // graph: Arc<PathIndex>,
    pub node_hub_map: BTreeMap<Node, (Option<HubId>, Option<HubId>)>,

    pub hubs: Vec<Hub>,
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

impl SpokeGraph {
    pub fn from_edges(
        node_count: usize,
        edges: impl IntoIterator<Item = Edge>,
    ) -> Self {
        let edges = edges.into_iter().collect::<Vec<_>>();

        let mut ufind = UnionFind::<OrientedNode>::new();

        for edge in edges.iter() {
            let (a, b) = edge.endpoints();

            ufind.union(a, b);
        }

        // for each representative in the disjoint set, create a hub
        let mut hubs: BTreeMap<HubId, Hub> = BTreeMap::new();
        let mut hub_ids: BTreeMap<OrientedNode, HubId> = BTreeMap::new();

        for edge in edges.iter() {
            let (from, to) = edge.endpoints();
            let rep = ufind.find(from);
            assert_eq!(ufind.find(to), rep);

            let hub_id = if let Some(id) = hub_ids.get(&rep).copied() {
                id
            } else {
                let id = HubId(hubs.len() as u32);
                hub_ids.insert(rep, id);
                id
            };

            let hub = hubs.entry(hub_id).or_default();
            hub.edges.insert((from, to));

            let (a, b) = match (from.is_reverse(), to.is_reverse()) {
                (false, false) => (from, to.flip()),
                (false, true) => (from, to),
                (true, false) => (from.flip(), to.flip()),
                (true, true) => (from.flip(), to),
            };

            hub.spokes.push(a);
            hub.spokes.push(b);
        }

        for (hub_id, hub) in hubs.iter_mut() {
            let mut adj_hubs = hub
                .edges
                .iter()
                .flat_map(|&(from, to)| [from, to])
                .filter_map(|onode| {
                    let proj_hub = hub_ids.get(&onode)?;
                    (proj_hub != hub_id).then_some(*proj_hub)
                })
                .collect::<Vec<_>>();

            adj_hubs.sort();
        }

        // fill node_hub_map

        let mut node_hub_map = BTreeMap::default();
        for node in (0..node_count as u32).map(Node::from) {
            let l = node.as_reverse();
            let r = node.as_forward();

            let l_hub = hub_ids.get(&l).copied();
            let r_hub = hub_ids.get(&r).copied();

            node_hub_map.insert(node, (l_hub, r_hub));
        }

        let hubs = hubs.into_values().collect::<Vec<_>>();

        println!("spoke graph contains {} hubs", hubs.len());

        Self { node_hub_map, hubs }
    }

    pub fn new(graph: &PathIndex) -> Self {
        Self::from_edges(
            graph.node_count,
            graph.edges_iter().map(|&(a, b)| Edge::new(a, b)),
        )
    }

    pub fn node_endpoint_hub(
        &self,
        node_endpoint: OrientedNode,
    ) -> Option<HubId> {
        let (f_hub_l, f_hub_r) =
            *self.node_hub_map.get(&node_endpoint.node())?;

        if node_endpoint.is_reverse() {
            f_hub_l
        } else {
            f_hub_r
        }
    }

    pub fn find_hub_from_edge(
        &self,
        (from, to): (OrientedNode, OrientedNode),
    ) -> Option<HubId> {
        let (f_hub_l, f_hub_r) = *self.node_hub_map.get(&from.node())?;
        // let (t_hub_l, t_hub_r) = *self.node_hub_map.get(&to.node())?;

        if from.is_reverse() {
            f_hub_l
        } else {
            f_hub_r
        }
    }

    pub fn neighbors<'a>(
        &'a self,
        hub: HubId,
    ) -> Option<impl Iterator<Item = HubId> + 'a> {
        let hub = self.hubs.get(hub.0 as usize)?;
        let iter = hub.adj_hubs.iter().copied();

        Some(iter)
    }

    // pub fn incoming_segments<'a>(
    //     &'a self,
    //     hub: HubId,
    // ) -> Option<impl Iterator<Item = OrientedNode> + 'a> {
    //     let hub = self.hubs.get(hub.0 as usize)?;
    //     let iter = hub.edges.iter().map(|&(from, to)| {
    //         match (from.is_reverse(), to.is_reverse()) {
    //             (false, false) => from,
    //             (false, true) => from,
    //             (true, false) => from.flip(),
    //             (true, true) => from.flip(),
    //         }
    //     });

    //     Some(iter)
    // }
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

    fn example_graph() -> SpokeGraph {
        let edges = example_graph_edges();

        let graph = SpokeGraph::from_edges(18, edges);

        graph
    }

    #[test]
    fn new_graph_ctr() {
        let edges = example_graph_edges();
        let graph = Graph::new(edges);

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
            // println!("{hub_ix}\n\t{hub:?}");
        }

        println!();

        // println!("node -> (hub, hub) map");
        // for node_ix in 0..18 {
        //     let node = Node::from(node_ix as u32);
        //     let left = graph.endpoint_hub_map.get(&node.as_reverse());
        //     let right = graph.endpoint_hub_map.get(&node.as_forward());
        //     println!("{node_ix:2} => {left:?}\t{right:?}");
        // }
    }

    #[test]
    fn spoke_graph_construction() {
        let graph = example_graph();

        println!("hubs");
        for (hub_ix, hub) in graph.hubs.iter().enumerate() {
            println!("{hub_ix}\n\t{hub:?}");
        }

        println!();

        println!("node -> (hub, hub) map");
        for (node, (left, right)) in graph.node_hub_map.iter() {
            let n = node.ix();
            println!("{n:2} -> {left:?}\t{right:?}");
        }
    }

    #[test]
    fn spoke_graph_projections() {
        // todo!();
    }
}
