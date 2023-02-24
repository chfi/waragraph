use roaring::RoaringBitmap;
use sprs::{CsMat, CsMatBase, CsVec};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};
use waragraph_core::graph::{Edge, Node, OrientedNode};

use super::hyper::{Cycle, HyperSpokeGraph, VertexId};

#[derive(Debug, Clone)]
pub struct MatGraph<VData, EData> {
    vertex_count: usize,
    edge_count: usize,

    // |V|x|V|
    adj: CsMat<u8>,

    // |V|x|E|
    inc: CsMat<u8>,

    vertex: Vec<VData>,
    edge: Vec<EData>,
}

impl<V, E> MatGraph<V, E> {
    pub fn print_adj(&self) {
        sprs::visu::print_nnz_pattern(self.adj.view());
    }

    pub fn print_inc(&self) {
        sprs::visu::print_nnz_pattern(self.inc.view());
    }

    pub fn from_edges(
        // vertex_count: usize,
        // edge_count: usize,
        edges: impl IntoIterator<Item = (u32, u32)>,
        vert_data: impl IntoIterator<Item = V>,
        edge_data: impl IntoIterator<Item = E>,
    ) -> Self {
        use sprs::TriMat;

        let vertex = vert_data.into_iter().collect::<Vec<_>>();
        let edge = edge_data.into_iter().collect::<Vec<_>>();

        let vertex_count = vertex.len();

        // let mut adj = TriMat::new(

        // let mut adj

        todo!();
    }

    pub fn neighbors(&self, vertex: usize) -> Vec<usize> {
        let col = self.adj.outer_view(vertex).unwrap();
        col.indices().to_vec()
    }

    pub fn neighbors_old(&self, vertex: usize) -> Vec<usize> {
        let n = self.vertex_count;
        let v = CsVec::new(n, vec![vertex as usize], vec![1]);

        let ns = &self.adj * &v;
        let (indices, _) = ns.into_raw_storage();
        indices
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CacTreeVx {
    Net { vertex: VertexId },
    Chain { cycle: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CacTreeEdge {
    Net {
        segment: Node,
        from: VertexId,
        to: VertexId,
    },
    Chain {
        net: VertexId,
        cycle: usize,
        prev_step: OrientedNode,
        this_step: OrientedNode,
    },
}

#[derive(Debug, Clone)]
pub struct CactusTree {
    cactus_graph: HyperSpokeGraph,
    graph: MatGraph<CacTreeVx, CacTreeEdge>,

    vertex_cycle_map: HashMap<VertexId, BTreeSet<usize>>,
    cycles: Vec<Cycle>,

    net_vertices: usize,
    net_edges: usize,

    chain_vertices: usize,
    chain_edges: usize,
}

impl CactusTree {
    pub fn from_cactus_graph(cactus: HyperSpokeGraph) -> Self {
        let cycles = super::hyper::find_cactus_graph_cycles(&cactus);

        // we have the VertexIds from the cactus graph,
        // plus the chain vertices, one for each cycle

        let net_vx_count = cactus.vertex_count();
        let chain_vx_count = cycles.len();
        let vertex_count = net_vx_count + chain_vx_count;

        let mut vertex: Vec<CacTreeVx> = Vec::new();

        for (vxid, _) in cactus.vertices() {
            vertex.push(CacTreeVx::Net { vertex: vxid });
        }

        let mut edges: Vec<CacTreeEdge> = Vec::new();

        let seg_count = cactus.spoke_graph.max_endpoint.ix() / 2;

        let mut remaining_segments = RoaringBitmap::default();
        remaining_segments.insert_range(0..=seg_count as u32);

        let mut cycle_vertices: Vec<Vec<([OrientedNode; 2], VertexId)>> =
            Vec::new();

        for (cix, cycle) in cycles.iter().enumerate() {
            vertex.push(CacTreeVx::Chain { cycle: cix });

            let mut vertices = Vec::new();

            for (step_ix, step) in cycle.steps.iter().enumerate() {
                let prev_step = if step_ix == 0 {
                    let i = cycle.steps.len() - 1;
                    cycle.steps[i]
                } else {
                    cycle.steps[step_ix - 1]
                };
                remaining_segments.remove(step.node().ix() as u32);
                vertices
                    .push(([prev_step, *step], cactus.endpoint_vertex(*step)));
            }

            cycle_vertices.push(vertices);
        }

        for i in remaining_segments.iter() {
            let node = Node::from(i);

            let from_hub =
                cactus.spoke_graph.node_endpoint_hub(node.as_reverse());
            let from_vx = cactus.hub_vertex_map[from_hub.ix()];

            let to_hub =
                cactus.spoke_graph.node_endpoint_hub(node.as_forward());
            let to_vx = cactus.hub_vertex_map[to_hub.ix()];

            edges.push(CacTreeEdge::Net {
                segment: node,
                from: from_vx,
                to: to_vx,
            });

            let c = ('a' as u8 + node.ix() as u8) as char;
        }

        let net_edges = edges.len();

        let mut vertex_cycle_map: HashMap<_, BTreeSet<_>> = HashMap::new();

        for (cix, vertices) in cycle_vertices.into_iter().enumerate() {
            for ([prev_step, this_step], vertex) in vertices {
                vertex_cycle_map.entry(vertex).or_default().insert(cix);
                edges.push(CacTreeEdge::Chain {
                    cycle: cix,
                    net: vertex,
                    prev_step,
                    this_step,
                });
            }
        }

        use sprs::TriMat;

        let v_n = vertex.len();
        let e_n = edges.len();

        let mut adj: TriMat<u8> = TriMat::new((v_n, v_n));
        let mut inc: TriMat<u8> = TriMat::new((v_n, e_n));

        edges.sort();
        edges.dedup();

        for (i, edge) in edges.iter().enumerate() {
            match edge {
                CacTreeEdge::Net { from, to, .. } => {
                    adj.add_triplet(from.ix(), to.ix(), 1);
                    adj.add_triplet(to.ix(), from.ix(), 1);
                    inc.add_triplet(from.ix(), i, 1);
                    inc.add_triplet(to.ix(), i, 1);
                }
                CacTreeEdge::Chain { net, cycle, .. } => {
                    let c_i = net_vx_count + *cycle;
                    adj.add_triplet(net.ix(), c_i, 1);
                    adj.add_triplet(c_i, net.ix(), 1);
                    inc.add_triplet(net.ix(), i, 1);
                    inc.add_triplet(c_i, i, 1);
                }
            }
        }

        let adj: CsMat<u8> = adj.to_csc();
        let inc: CsMat<u8> = inc.to_csr();

        let chain_edges = edges.len() - net_edges;

        let graph = MatGraph {
            vertex_count,
            edge_count: edges.len(),
            adj,
            inc,
            vertex,
            edge: edges,
        };

        Self {
            cactus_graph: cactus,
            graph,
            cycles,
            vertex_cycle_map,
            net_vertices: net_vx_count,
            net_edges,
            chain_vertices: chain_vx_count,
            chain_edges,
        }
    }
}

impl CactusTree {
    pub fn rooted_cactus_forest(&self) -> Vec<Vec<usize>> {
        let mut forest = Vec::new();

        let mut visited: HashSet<usize> = HashSet::new();

        let mut stack = Vec::new();

        // iterate through all chain vertices
        let chain_vertex_ixs =
            self.net_vertices..(self.net_vertices + self.chain_vertices);

        for ix in chain_vertex_ixs {
            let mut order = Vec::new();

            stack.push((None, ix));

            while let Some((parent, vi)) = stack.pop() {
                if !visited.contains(&vi) {
                    visited.insert(vi);
                    order.push(vi);

                    let on_chain = vi >= self.net_vertices;

                    let neighbors =
                        self.graph.neighbors(vi).into_iter().filter(|&vj| {
                            if on_chain {
                                vj < self.net_vertices
                            } else {
                                vj >= self.net_vertices
                            }
                        });

                    for vj in neighbors {
                        println!("pushing ({vi} -> {vj})");
                        stack.push((Some(vi), vj));
                    }
                }
            }

            if !order.is_empty() {
                forest.push(order);
            }
        }

        forest
    }

    // vg_adj is an 2Nx2N adjacency matrix where N is the number of
    // segments in the variation graph; it lacks the connectivity
    // "within" segments (the black edges in the biedged repr.)
    fn chain_edge_net_graph(
        &self,
        vg_adj: &CsMat<u8>,
        chain_pair: (OrientedNode, OrientedNode),
        chain_ix: usize,
    ) -> Option<CsMat<u8>> {
        use sprs::TriMat;

        // chain pairs only have the one edge with one net vertex,
        // so that's the only vertex we need to project from
        let (net, _cycle_ix) =
            if let CacTreeEdge::Chain { net, cycle, .. } =
                self.graph.edge[self.net_edges + chain_ix]
            {
                (net, cycle)
            } else {
                unreachable!();
            };

        let endpoints = self.net_vertex_endpoints(net).collect::<BTreeSet<_>>();
        // let endpoints_vec = endpoints.iter().copied().

        let mut net_adj: TriMat<u8> = TriMat::new(vg_adj.shape());

        // find the edges between segments among the endpoints
        for &si in endpoints.iter() {
            if let Some(column) = vg_adj.outer_view(si.ix()) {
                for (isj, _) in column.iter() {
                    let sj = OrientedNode::from(isj as u32);
                    if endpoints.contains(&sj) {
                        net_adj.add_triplet(isj, si.ix(), 1);
                    }
                }
            }
        }

        // the gray edges are the subset of vg edges that connect endpoints
        // in the subgraph

        // the black edges are created from the cycles that the
        // endpoints are in the subgraph; since this is a chain pair,
        // there's just one contained cycle

        let mut black_edges: Vec<(OrientedNode, OrientedNode)> = Vec::new();

        let cycles = self.vertex_cycle_map.get(&net).unwrap();

        fn print_step(step: OrientedNode) {
            let c = ('a' as u8 + step.node().ix() as u8) as char;
            let o = if step.is_reverse() { "-" } else { "+" };
            print!("{c}{o}")
        }

        println!("----------------------");

        println!("present endpoints:");
        for &step in &endpoints {
            print_step(step);
            print!(", ");
        }

        println!("\n");

        for &cycle_ix in cycles {
            println!("  ---- Cycle {cycle_ix} ----");
            let cycle = &self.cycles[cycle_ix];

            // start by "flattening" the cycle, so that both segment endpoints
            // are present. iterating the flattened cycle produces

            // e.g. [b+, c-] => [b-, b
            let mut steps = cycle
                .steps
                .iter()
                .flat_map(|s| [*s, s.flip()])
                .filter(|s| endpoints.contains(s))
                // .flat_map(|s| [s.flip(), *s])
                .collect::<Vec<_>>();

            println!("steps:");
            for &step in &steps {
                if endpoints.contains(&step) {
                    print_step(step);
                    print!(", ");
                }
            }
            println!("\n");

            // if there's just two endpoints, there's just one step,
            // meaning one edge to add, so we're done
            if let &[a, b] = steps.as_slice() {
                let (ca, cb) = chain_pair;
                // if a.node() == b.node()
                let a_chain = a == ca || a == cb;
                let b_chain = b == ca || b == cb;
                if endpoints.contains(&a)
                    && endpoints.contains(&b)
                    && !(a_chain || b_chain)
                {
                    println!("pushing segment");
                    black_edges.push((a, b));
                    continue;
                }
            }

            let mut edge_start: Option<OrientedNode> = None;

            for (i, w) in steps.windows(2).enumerate() {
                let a_in = endpoints.contains(&w[0]);
                let b_in = endpoints.contains(&w[1]);

                print!("  on step [");
                print_step(w[0]);
                print!(", ");
                print_step(w[1]);
                println!("]");

                if w[0].node() == w[1].node() {
                    // traversing a segment
                    if edge_start.is_none() && a_in {
                        print!(" >> opening black edge: ");
                        print_step(w[0]);
                        println!();
                        edge_start = Some(w[0]);
                    } else if let Some(start) = edge_start {
                        // the chain endpoints should have no black edges
                        let start_chain =
                            start == chain_pair.0 || start == chain_pair.1;
                        let end_chain =
                            w[1] == chain_pair.0 || w[1] == chain_pair.1;

                        if b_in && !start_chain && !end_chain {
                            edge_start = None;
                            black_edges.push((start, w[1]));
                        }
                    }
                } else {
                    // traversing an edge
                }
            }
        }

        if black_edges.is_empty() {
            return None;
        }

        for (a, b) in black_edges {
            net_adj.add_triplet(b.ix(), a.ix(), 1);
        }

        Some(net_adj.to_csc())
    }

    pub fn project_segment_end(&self, end: OrientedNode) -> usize {
        let vx = self.cactus_graph.endpoint_vertex(end);
        vx.ix()
    }

    pub fn net_vertex_endpoints(
        &self,
        net: VertexId,
    ) -> impl Iterator<Item = OrientedNode> + '_ {
        let vx = self.cactus_graph.get_vertex(net);
        vx.hubs.iter().flat_map(|h| {
            self.cactus_graph.spoke_graph.hub_endpoints[h.ix()]
                .iter()
                .copied()
        })
    }

    pub fn enumerate_chain_pairs(
        &self,
    ) -> Vec<((OrientedNode, OrientedNode), usize)> {
        // let chain_range = (self.net_edges..(self.net_edges + self.chain_edges));

        let mut chain_pairs = Vec::with_capacity(self.chain_edges);

        let chain_range = 0..self.chain_edges;

        // for (cycle_ix, cycle) in self.cycles.iter().enumerate() {
        //
        // }

        fn print_step(step: OrientedNode) {
            let c = ('a' as u8 + step.node().ix() as u8) as char;
            let o = if step.is_reverse() { "-" } else { "+" };
            print!("{c}{o}")
        }

        for chain_ix in chain_range {
            let edge_ix = self.net_edges + chain_ix;

            let (net, cycle_ix, steps) = if let CacTreeEdge::Chain {
                net,
                cycle,
                prev_step,
                this_step,
            } = self.graph.edge[edge_ix]
            {
                (net, cycle, [prev_step, this_step])
            } else {
                unreachable!();
            };

            let prev = steps[0].flip();
            let this = steps[1];

            let cycle = &self.cycles[cycle_ix];

            println!();
            println!("_______________________");
            print!("cycle!!  ");
            for (i, step) in cycle.steps.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }

                print_step(*step);
            }
            println!();

            let net_spokes = self
                .cactus_graph
                .vertex_spokes(net)
                .filter(|(s, _)| {
                    let n = s.node();
                    cycle.steps.iter().any(|is| {
                        let ns = is.node();
                        n == ns
                    })
                })
                .collect::<Vec<_>>();
            if net_spokes.len() == 2 {
                let (a, _) = net_spokes[0];
                let (b, _) = net_spokes[1];
                if a.node() != b.node() {
                    chain_pairs.push(((a, b), chain_ix));
                }
            }
        }

        // output with chain pairs in shortest cycles first
        chain_pairs.sort_by_cached_key(|((_, _), ci)| {
            let edge_i = self.net_edges + *ci;
            let edge = &self.graph.edge[edge_i];
            if let CacTreeEdge::Chain { cycle, .. } = edge {
                self.cycles[*cycle].steps.len()
            } else {
                unreachable!();
            }
        });

        chain_pairs
    }
}

#[cfg(test)]
mod tests {
    use sprs::vec::SparseIterTools;
    use waragraph_core::graph::PathIndex;

    use super::*;

    fn print_step(step: OrientedNode) {
        let c = ('a' as u8 + step.node().ix() as u8) as char;
        let o = if step.is_reverse() { "-" } else { "+" };
        print!("{c}{o}")
    }

    #[test]
    fn paper_fig3_cactus_tree() {
        let cactus_graph = super::super::hyper::tests::paper_cactus_graph();
        let edges = super::super::tests::example_graph_edges();

        let cactus_tree = CactusTree::from_cactus_graph(cactus_graph);

        println!("vertex_count: {}", cactus_tree.graph.vertex_count);
        println!("edge_count: {}", cactus_tree.graph.edge_count);

        assert_eq!(cactus_tree.graph.vertex_count, 19);
        assert_eq!(cactus_tree.graph.edge_count, 18);

        println!("-----------------------");
        cactus_tree.graph.print_adj();
        println!("-----------------------");
        cactus_tree.graph.print_inc();

        println!("---");

        println!("enumerating chain pairs!");
        println!("---");
        println!();
        let mut chain_pairs = cactus_tree.enumerate_chain_pairs();

        println!("{chain_pairs:?}");

        println!("\n\n------------\n\n");

        for (i, cycle) in cactus_tree.cycles.iter().enumerate() {
            // println!("{i}\t{cycle:?}");

            print!("cycle {i} steps\t[");

            for (i, step) in cycle.steps.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                print_step(*step);
            }
            println!("]");

            /////////////

            print!("cycle {i} step endpoints\t[");

            for (i, &net_vx) in cycle.step_endpoints.iter().enumerate() {
                if i > 0 {
                    print!(";  ");
                }
                print!("[{net_vx:?}] ");
                let endpoints = cactus_tree.net_vertex_endpoints(net_vx);

                for (j, s) in endpoints.into_iter().enumerate() {
                    if j > 0 {
                        print!(", ");
                    }
                    print_step(s);
                }
            }
            println!("]");
        }

        println!();
        // println!(");

        for net_ix in 0..cactus_tree.net_vertices {
            let net_vx = VertexId(net_ix as u32);
            let endpoints = cactus_tree.net_vertex_endpoints(net_vx);

            print!("net vertex {net_ix}\t[");

            for s in endpoints {
                print_step(s);
            }
            println!("]");
        }

        println!();

        /*
        for seg in 0u32..18 {
            let node = Node::from(seg);
            let r = node.as_reverse();
            let f = node.as_forward();

            let pr = cactus_tree.project_segment_end(r);
            let pf = cactus_tree.project_segment_end(f);

            print!("{seg:2} - ");
            print_step(r);
            println!(" : Vertex {pr}");

            print!("{seg:2} - ");
            print_step(f);
            println!(" : Vertex {pf}");
        }
        */

        println!("chain pair count: {}", chain_pairs.len());
        chain_pairs.reverse();

        for ((a, b), chain_ix) in chain_pairs {
            print!("chain {chain_ix}: (");
            print_step(a);
            print!(", ");
            print_step(b);
            println!(")");
        }
    }

    #[test]
    fn paper_fig5_cactus_tree() {
        let cactus_graph = super::super::hyper::tests::alt_paper_cactus_graph();
        let edges = super::super::tests::alt_paper_graph_edges();

        println!("cactus graph vertex count: {}", cactus_graph.vertex_count());

        let cactus_tree = CactusTree::from_cactus_graph(cactus_graph);

        println!("vertex_count: {}", cactus_tree.graph.vertex_count);
        println!("edge_count: {}", cactus_tree.graph.edge_count);

        assert_eq!(cactus_tree.graph.vertex_count, 15);
        assert_eq!(cactus_tree.graph.edge_count, 14);

        println!("enumerating chain pairs!");
        let mut chain_pairs = cactus_tree.enumerate_chain_pairs();

        for net_ix in 0..cactus_tree.net_vertices {
            let net_vx = VertexId(net_ix as u32);
            let endpoints = cactus_tree.net_vertex_endpoints(net_vx);

            print!("net vertex {net_ix}\t[");

            for s in endpoints {
                print_step(s);
            }
            println!("]");
        }

        println!();

        println!("chain pair count: {}", chain_pairs.len());
        chain_pairs.reverse();

        for ((a, b), chain_ix) in chain_pairs {
            print!("chain {chain_ix}: (");
            print_step(a);
            print!(", ");
            print_step(b);
            println!(")");
        }
    }

    #[test]
    fn test_chain_pair_net_graph() {
        // let cactus_graph = super::super::hyper::tests::paper_cactus_graph();
        // let edges = super::super::tests::example_graph_edges();

        // let cactus_tree = CactusTree::from_cactus_graph(cactus_graph);

        // let vg_adj =
        //     PathIndex::directed_adjacency_matrix(18, edges.iter().copied());

        let cactus_graph = super::super::hyper::tests::alt_paper_cactus_graph();
        let edges = super::super::tests::alt_paper_graph_edges();

        let cactus_tree = CactusTree::from_cactus_graph(cactus_graph);

        let vg_adj =
            PathIndex::directed_adjacency_matrix(14, edges.iter().copied());

        println!();

        let chain_pairs = cactus_tree.enumerate_chain_pairs();

        println!("---\nchain pair net graphs\n----\n");

        let mut zero_bes = 0;

        for &((a, b), chain_ix) in chain_pairs.iter() {
            println!("\n--------\n");
            print!("Chain pair: ");
            print_step(a);
            print!(", ");
            print_step(b);
            println!(", chain: {chain_ix}");
            if let Some(net_graph) =
                cactus_tree.chain_edge_net_graph(&vg_adj, (a, b), chain_ix)
            {
                sprs::visu::print_nnz_pattern(net_graph.view());
            } else {
                zero_bes += 1;
            }

            println!("\n--------\n");
        }

        println!("number of net graphs with zero black edges: {zero_bes}");
    }

    #[test]
    fn test_rooted_forest() {
        let graph_fig3 = super::super::hyper::tests::paper_cactus_graph();
        let tree_fig3 = CactusTree::from_cactus_graph(graph_fig3);
        let forest_fig3 = tree_fig3.rooted_cactus_forest();

        let graph_fig5 = super::super::hyper::tests::alt_paper_cactus_graph();
        let tree_fig5 = CactusTree::from_cactus_graph(graph_fig5);
        let forest_fig5 = tree_fig5.rooted_cactus_forest();

        // TODO better tests once bridges are in

        assert_eq!(forest_fig3.len(), 3);
        assert_eq!(forest_fig3[0].len(), 8);
        assert_eq!(forest_fig3[1].len(), 5);
        assert_eq!(forest_fig3[2].len(), 3);

        assert_eq!(forest_fig5.len(), 1);
        assert_eq!(forest_fig5[0].len(), 13);

        println!("\n");

        let fig3_expected = vec![(6, 1), (3, 1), (1, 1)];

        // this looks correct (chain edges only, missing edges when
        // backtracking)
        for (i, (tree, expected)) in
            forest_fig3.iter().zip(fig3_expected).enumerate()
        {
            let inc = &tree_fig3.graph.inc;

            let (edges, missing): (Vec<_>, Vec<_>) = tree
                .windows(2)
                .map(|ixs| {
                    let i_row = inc.outer_view(ixs[0]).unwrap();
                    let j_row = inc.outer_view(ixs[1]).unwrap();

                    let edge = i_row
                        .iter()
                        .nnz_zip(j_row.iter())
                        .map(|(i, _a, _b)| i)
                        .next()
                        .map(|edge_ix| tree_fig3.graph.edge[edge_ix]);

                    edge
                })
                .partition(|edge| edge.is_some());

            let (exp_edges, exp_missing) = expected;

            assert_eq!(edges.len(), exp_edges);
            assert_eq!(missing.len(), exp_missing);
        }

        println!("\n");

        let (edges, missing): (Vec<_>, Vec<_>) = forest_fig5[0]
            .windows(2)
            .map(|ixs| {
                let inc = &tree_fig5.graph.inc;
                let i_row = inc.outer_view(ixs[0]).unwrap();
                let j_row = inc.outer_view(ixs[1]).unwrap();

                let edge = i_row
                    .iter()
                    .nnz_zip(j_row.iter())
                    .map(|(i, _a, _b)| i)
                    .next()
                    .map(|edge_ix| tree_fig5.graph.edge[edge_ix]);

                edge
            })
            .partition(|edge| edge.is_some());

        assert_eq!(edges.len(), 7);
        assert_eq!(missing.len(), 5);
    }
}
