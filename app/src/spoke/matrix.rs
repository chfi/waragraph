use roaring::RoaringBitmap;
use sprs::{CsMat, CsMatBase, CsVec};
use std::sync::Arc;
use waragraph_core::graph::{Node, OrientedNode};

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

        println!("net_vx_count: {net_vx_count}");
        println!("chain_vx_count: {chain_vx_count}");

        let mut vertex: Vec<CacTreeVx> = Vec::new();

        for (vxid, _) in cactus.vertices() {
            vertex.push(CacTreeVx::Net { vertex: vxid });
        }

        let mut edges: Vec<CacTreeEdge> = Vec::new();

        let seg_count = cactus.spoke_graph.max_endpoint.ix() / 2;
        println!("seg_count: {seg_count}");

        let mut remaining_segments = RoaringBitmap::default();
        remaining_segments.insert_range(0..=seg_count as u32);

        println!("remaining segments: {}", remaining_segments.len());
        let mut cycle_vertices: Vec<Vec<([OrientedNode; 2], VertexId)>> =
            Vec::new();

        for (cix, cycle) in cycles.iter().enumerate() {
            vertex.push(CacTreeVx::Chain { cycle: cix });

            let mut vertices = Vec::new();
            // vertices.push(cycle.endpoint);

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

            println!("vertices: {vertices:?}");
            cycle_vertices.push(vertices);
        }

        println!("remaining segments: {}", remaining_segments.len());
        println!("cycle vertices: {}", cycle_vertices.len());

        for i in remaining_segments.iter() {
            let node = Node::from(i);
            println!("remaining: {node:?}");

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

            println!("pushing net edge `{c}` {from_vx:?}, {to_vx:?}");
        }

        let net_edges = edges.len();
        println!("net edge count: {}", edges.len());

        for (cix, vertices) in cycle_vertices.into_iter().enumerate() {
            for ([prev_step, this_step], vertex) in vertices {
                edges.push(CacTreeEdge::Chain {
                    cycle: cix,
                    net: vertex,
                    prev_step,
                    this_step,
                });
                println!("pushing chain edge {cix}, {vertex:?}");
            }
        }

        println!("chain edge count: {}", edges.len() - net_edges);

        use sprs::TriMat;

        let v_n = vertex.len();
        let e_n = edges.len();

        let mut adj: TriMat<u8> = TriMat::new((v_n, v_n));
        let mut inc: TriMat<u8> = TriMat::new((v_n, e_n));

        edges.sort();
        println!("edges len: {}", edges.len());
        edges.dedup();
        println!("edges dedup len: {}", edges.len());

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
        let inc: CsMat<u8> = inc.to_csc();

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
            net_vertices: net_vx_count,
            net_edges,
            chain_vertices: chain_vx_count,
            chain_edges,
        }
    }
}

impl CactusTree {
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
                    // cycle.steps.iter().any(|is| is.node()
                    // let n = s.node();
                    // steps.iter().any(|s| s.node() == n)
                })
                .collect::<Vec<_>>();
            for (s, v) in net_spokes {
                print!("vertex {v:?} - ");
                print_step(s);
                println!();
            }
            println!();
            // println!(" --- {net_spokes:?}");
            // println!(" steps: [{prev:?}, {this:?}]");
            print!(" steps: [");
            print_step(prev);
            print!(", ");
            print_step(this);
            println!("]");

            let prev_vx = self.project_segment_end(steps[0].flip());
            let this_vx = self.project_segment_end(steps[1]);

            print!(" {chain_ix}:\t");
            print_step(steps[0].flip());
            print!("  -  ");
            print_step(steps[1]);
            println!("\tprev: {prev_vx} - {net:?} - {this_vx}");
            println!();
        }

        // chain_pairs.sort();
        // chain_pairs.dedup();

        chain_pairs
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn print_step(step: OrientedNode) {
        let c = ('a' as u8 + step.node().ix() as u8) as char;
        let o = if step.is_reverse() { "-" } else { "+" };
        print!("{c}{o}")
    }

    #[test]
    fn test_cactus_tree() {
        let cactus_graph = super::super::hyper::tests::paper_cactus_graph();

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
        let out = cactus_tree.enumerate_chain_pairs();

        println!("{out:?}");

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
    }
}
