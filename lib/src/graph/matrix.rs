use sprs::CsMat;

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

    pub fn neighbors(&self, vertex: usize) -> Vec<usize> {
        let col = self.adj.outer_view(vertex).unwrap();
        col.indices().to_vec()
    }
}
