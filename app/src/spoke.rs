use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use reunion::UnionFind;
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
    graph: Arc<PathIndex>,

    hub_adj: Vec<(HubId, Vec<OrientedNode>)>,
    endpoint_partitions: UnionFind<OrientedNode>,
}

impl Graph {
    pub fn new(graph: Arc<PathIndex>) -> Self {
        let mut end_ufind = UnionFind::<OrientedNode>::new();

        for &(from, to) in graph.edges_iter() {
            let (a, b) = match (from.is_reverse(), to.is_reverse()) {
                (false, false) => (from.node_end(), to.node_start()),
                (false, true) => (from.node_end(), to.node_end()),
                (true, false) => (from.node_start(), to.node_start()),
                (true, true) => (from.node_start(), to.node_end()),
            };
        }

        todo!();
    }
}

pub struct SpokeGraph {
    // graph: Arc<PathIndex>,
    pub node_hub_map: BTreeMap<Node, (Option<HubId>, Option<HubId>)>,

    pub hubs: Vec<Hub>,
}

#[derive(Default, Clone)]
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
        use reunion::{UnionFind, UnionFindTrait};

        let edges = edges.into_iter().collect::<Vec<_>>();

        let mut ufind = UnionFind::<OrientedNode>::new();

        for &Edge { from, to } in edges.iter() {
            let (a, b) = match (from.is_reverse(), to.is_reverse()) {
                (false, false) => (from.node_end(), to.node_start()),
                (false, true) => (from.node_end(), to.node_end()),
                (true, false) => (from.node_start(), to.node_start()),
                (true, true) => (from.node_start(), to.node_end()),
            };

            ufind.union(a, b);
        }

        // for each representative in the disjoint set, create a hub
        let mut hubs: BTreeMap<HubId, Hub> = BTreeMap::new();
        let mut hub_ids: BTreeMap<OrientedNode, HubId> = BTreeMap::new();

        for &Edge { from, to } in edges.iter() {
            let (from, to) = match (from.is_reverse(), to.is_reverse()) {
                (false, false) => (from, to.flip()),
                (false, true) => (from, to),
                (true, false) => (from.flip(), to.flip()),
                (true, true) => (from.flip(), to),
            };
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

        /*
        (a+, b+)
        (a+, c+)
        (b+, d+)
        (c+, d+)
        (d+, e+)
        (d+, f+)
        (e+, g+)
        (f+, g+)
        (f+, h+)
        (g+, k+)
        (g+, l+)
        (h+, i+)
        (h+, j+)
        (i+, j+)
        (j+, l+)
        (l+, m+)
        (m+, n+)
        (m+, o+)
        (n+, p+)
        (o+, p+)
        (p+, m+)
        (p+, q+)
        (p+, r+)
        */
        todo!();
    }

    #[test]
    fn spoke_graph_construction() {
        todo!();
    }

    #[test]
    fn spoke_graph_projections() {
        todo!();
    }
}
