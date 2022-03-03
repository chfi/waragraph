use gfa::gfa::GFA;
use thunderdome::{Arena, Index};

use sprs::{CsMat, CsVec, TriMat};

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node(u32);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Strand(u32);

pub struct Waragraph {
    node_count: usize,

    // adj: CsMatI<u8, Strand>,
    // adj: CsMatI<u8, Node>,
    pub adj_n_n: CsMat<u8>,
}

impl Waragraph {
    pub fn from_gfa(gfa: &GFA<usize, ()>) -> Result<Self> {
        let node_count = gfa.segments.len();

        let mut tris = TriMat::new((node_count, node_count));

        for edge in gfa.links.iter() {
            let from = edge.from_segment;
            let to = edge.to_segment;

            tris.add_triplet(from, to, 1);
        }

        let adj_n_n = tris.to_csc();

        Ok(Self {
            node_count,
            adj_n_n,
        })
    }
}
