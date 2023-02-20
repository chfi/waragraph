use sprs::{CsMat, CsMatBase, CsVec};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum CacTreeVx {
    Net { vertex: VertexId },
    Chain { cycle: usize },
}

impl<E> MatGraph<CacTreeVx, E> {
    pub fn build_cactus_tree(cactus: &HyperSpokeGraph) -> Self {
        let cycles = super::hyper::find_cactus_graph_cycles(cactus);

        // we have the VertexIds from the cactus graph,
        // plus the chain vertices, one for each cycle

        let vertex_count = cactus.vertex_count() + cycles.len();

        let mut vertex: Vec<CacTreeVx> = Vec::new();

        // the edges are more complicated

        let mut edges = Vec::new();

        todo!();
    }
}
