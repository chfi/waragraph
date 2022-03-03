use std::num::NonZeroU32;

use gfa::gfa::GFA;
use thunderdome::{Arena, Index};

use sprs::{CsMat, CsMatI, CsVec, CsVecView, TriMat, TriMatI};

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node(u32);

impl From<NonZeroU32> for Node {
    fn from(u: NonZeroU32) -> Node {
        let v = u.get();
        Node(v - 1)
    }
}

impl From<u32> for Node {
    fn from(u: u32) -> Node {
        Node(u)
    }
}

impl Into<NonZeroU32> for Node {
    fn into(self) -> NonZeroU32 {
        if let Some(u) = NonZeroU32::new(self.0 + 1) {
            u
        } else {
            unreachable!();
        }
    }
}

impl Into<u32> for Node {
    fn into(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0 + 1)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Strand(u32);

impl std::fmt::Display for Strand {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let i = self.0 >> 1;
        let rev = (self.0 & 1) == 1;
        if rev {
            write!(f, "{}-", i + 1)?;
        } else {
            write!(f, "{}+", i + 1)?;
        }
        Ok(())
    }
}

pub struct Waragraph {
    node_count: usize,

    // adj: CsMatI<u8, Strand>,
    // adj: CsMatI<u8, Node>,
    pub adj_n_n: CsMatI<u8, u32>,
}

impl Waragraph {
    pub fn from_gfa(gfa: &GFA<usize, ()>) -> Result<Self> {
        let node_count = gfa.segments.len();

        let nodes = node_count as u32;
        let mut tris: TriMatI<u8, u32> = TriMatI::new((node_count, node_count));

        for edge in gfa.links.iter() {
            let from = edge.from_segment - 1;
            let to = edge.to_segment - 1;

            tris.add_triplet(to, from, 1);
        }

        let adj_n_n = tris.to_csc();

        Ok(Self {
            node_count,
            adj_n_n,
        })
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    // pub fn neighbors_fwd(&self, node: Node) -> Option<CsVecView<'_, u8>> {
    //     let i = node.0 as usize;
    //     self.adj_n_n.outer_view(i)
    // }

    // pub fn neighbors_fwd(&self, node: Node) -> Option<CsVecView<'_, u8>> {
    pub fn neighbors_fwd<'a>(&'a self, node: Node) -> Option<&'a [Node]> {
        let i = node.0 as usize;
        let range = self.adj_n_n.indptr().outer_inds_sz(i);
        let indices = &self.adj_n_n.indices()[range];

        // TODO do this with bytemuck instead
        let slice = unsafe {
            let ptr = indices.as_ptr();
            let slice: &'a [Node] =
                std::slice::from_raw_parts(ptr as _, indices.len());
            slice
        };
        Some(slice)
    }
}
