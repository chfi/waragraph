use crate::graph::{Edge, Node, OrientedNode};
use roaring::RoaringBitmap;
use sprs::{CsMat, CsMatBase, CsVec};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

// use super::hyper::{Cycle, HyperSpokeGraph, VertexId};

#[derive(Debug, Clone)]
pub struct MatGraph<VData, EData> {
    pub vertex_count: usize,
    pub edge_count: usize,

    // |V|x|V|
    pub adj: CsMat<u8>,

    // |V|x|E|
    pub inc: CsMat<u8>,

    pub vertex: Vec<VData>,
    pub edge: Vec<EData>,
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
