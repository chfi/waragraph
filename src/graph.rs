use thunderdome::{Arena, Index};

use nalgebra_sparse::CscMatrix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vertex(Index);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge(Index);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path(Index);

#[derive(Clone)]
pub struct VertexData {
    //
}

#[derive(Clone)]
pub struct EdgeData {
    //
}

#[derive(Clone)]
pub struct PathData {
    //
}

pub struct Waragraph {
    vertices: Arena<VertexData>,
    edges: Arena<EdgeData>,
    paths: Arena<PathData>,
}

impl std::ops::Index<Vertex> for Waragraph {
    type Output = VertexData;

    fn index(&self, v: Vertex) -> &Self::Output {
        &self.vertices[v.0]
    }
}

impl std::ops::Index<Edge> for Waragraph {
    type Output = EdgeData;

    fn index(&self, e: Edge) -> &Self::Output {
        &self.edges[e.0]
    }
}

impl std::ops::Index<Path> for Waragraph {
    type Output = PathData;

    fn index(&self, p: Path) -> &Self::Output {
        &self.paths[p.0]
    }
}
