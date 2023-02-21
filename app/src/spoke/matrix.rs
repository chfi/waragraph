use roaring::RoaringBitmap;
use sprs::{CsMat, CsMatBase, CsVec};
use waragraph_core::graph::Node;

use super::hyper::{HyperSpokeGraph, VertexId};

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
    },
}

impl MatGraph<CacTreeVx, CacTreeEdge> {
    pub fn build_cactus_tree(cactus: &HyperSpokeGraph) -> Self {
        let cycles = super::hyper::find_cactus_graph_cycles(cactus);

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
        remaining_segments.insert_range(0..seg_count as u32);

        let mut cycle_vertices: Vec<Vec<VertexId>> = Vec::new();

        for (cix, cycle) in cycles.iter().enumerate() {
            vertex.push(CacTreeVx::Chain { cycle: cix });

            let mut vertices = Vec::new();
            vertices.push(cycle.endpoint);

            for step in cycle.steps.iter() {
                remaining_segments.remove(step.node().ix() as u32);
                vertices.push(cactus.endpoint_vertex(*step));
            }

            cycle_vertices.push(vertices);
        }

        println!("remaining segments: {}", remaining_segments.len());

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
        }

        for (cix, vertices) in cycle_vertices.into_iter().enumerate() {
            for vertex in vertices {
                edges.push(CacTreeEdge::Chain {
                    cycle: cix,
                    net: vertex,
                });
            }
        }

        use sprs::TriMat;

        let v_n = vertex.len();
        let e_n = edges.len();

        let mut adj: TriMat<u8> = TriMat::new((v_n, v_n));
        let mut inc: TriMat<u8> = TriMat::new((v_n, e_n));

        edges.sort();

        for (i, edge) in edges.iter().enumerate() {
            match edge {
                CacTreeEdge::Net { from, to, .. } => {
                    adj.add_triplet(from.ix(), to.ix(), 1);
                    inc.add_triplet(from.ix(), i, 1);
                    inc.add_triplet(to.ix(), i, 1);
                }
                CacTreeEdge::Chain { net, cycle } => {
                    let c_i = net_vx_count + *cycle;
                    adj.add_triplet(net.ix(), c_i, 1);
                    inc.add_triplet(net.ix(), i, 1);
                    inc.add_triplet(c_i, i, 1);
                }
            }
        }

        let adj: CsMat<u8> = adj.to_csc();
        let inc: CsMat<u8> = inc.to_csc();

        Self {
            vertex_count,
            edge_count: edges.len(),
            adj,
            inc,
            vertex,
            edge: edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cactus_tree() {
        let cactus_graph = super::super::hyper::tests::paper_cactus_graph();

        let cactus_tree = MatGraph::build_cactus_tree(&cactus_graph);

        println!("vertex_count: {}", cactus_tree.vertex_count);
        println!("edge_count: {}", cactus_tree.edge_count);
    }
}
