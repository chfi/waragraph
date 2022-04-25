use std::collections::{BTreeMap, HashMap};

use ash::vk;
use bimap::BiBTreeMap;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use raving::vk::{context::VkContext, BufferIx, GpuResources, VkEngine};
use rustc_hash::FxHashMap;

use sled::IVec;
use thunderdome::{Arena, Index};

use sprs::{CsMatI, CsVecI, TriMatI};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use rhai::plugin::*;

pub mod script;

use crate::viewer::ViewDiscrete1D;

#[repr(transparent)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, zerocopy::AsBytes,
)]
pub struct Node(u32);

impl From<u32> for Node {
    fn from(u: u32) -> Node {
        Node(u)
    }
}

impl From<usize> for Node {
    fn from(u: usize) -> Node {
        Node(u as u32)
    }
}

impl From<Node> for u32 {
    fn from(n: Node) -> u32 {
        n.0
    }
}

impl From<Node> for usize {
    fn from(n: Node) -> usize {
        n.0 as usize
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0 + 1)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Strand(u32);

impl Strand {
    pub fn new(node: Node, rev: bool) -> Self {
        let i = node.0 << 1;
        if rev {
            Strand(i & 1)
        } else {
            Strand(i)
        }
    }

    pub const fn node(&self) -> Node {
        Node(self.0 >> 1)
    }

    pub const fn is_reverse(&self) -> bool {
        (self.0 & 1) == 1
    }
}

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

#[repr(transparent)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, zerocopy::AsBytes,
)]
pub struct Path(usize);

impl Path {
    pub fn ix(&self) -> usize {
        self.0
    }
}

impl From<usize> for Path {
    fn from(u: usize) -> Path {
        Path(u)
    }
}

impl From<Path> for usize {
    fn from(p: Path) -> usize {
        p.0
    }
}

impl From<u32> for Path {
    fn from(u: u32) -> Path {
        Path(u as usize)
    }
}

impl From<Path> for u32 {
    fn from(p: Path) -> u32 {
        p.0 as u32
    }
}

// this should probably be `TryFrom`, with an Error type that
// implements `Into` for the rhai error type
// impl From<i64> for Path {
//     fn from(u: i64) -> Path {
//         Path(u as usize)
//     }
// }

// impl Into<i64> for Path {
//     fn into(self) -> i64 {
//         self.0 as i64
//     }
// }

// impl std::fmt::Display for Path {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

pub struct Waragraph {
    node_count: usize,
    total_len: usize,

    pub node_sum_lens: Vec<usize>,
    pub node_lens: Vec<u32>,
    pub sequences: Vec<Vec<u8>>,

    edges: FxHashMap<(Node, Node), u32>,

    // adj: CsMatI<u8, Strand>,
    // adj: CsMatI<u8, Node>,
    pub adj_n_n: CsMatI<u8, u32>,
    pub d0: CsMatI<i8, u32>,

    // pub paths: Vec<CsVecI<Strand, u32>>,
    pub path_lens: Vec<usize>,
    pub path_sum_lens: Arc<Vec<Vec<(Node, usize)>>>,
    pub paths: Vec<CsVecI<u32, u32>>,

    // pub path_names: BiBTreeMap<IVec, usize>,
    pub path_names: BiBTreeMap<Path, Vec<u8>>,
    pub path_names_prefixes: BTreeMap<Vec<u8>, Path>,
    // pub path_names: HashMap<usize, Arc<Vec<u8>>>,
    // pub path_indices: BTreeMap<Arc<Vec<u8>>, usize>,
    // pub path_names: Vec<Vec<u8>>,
    pub path_nodes: Vec<roaring::RoaringBitmap>,
    pub path_invert: Vec<roaring::RoaringBitmap>,

    pub path_offsets: Vec<usize>,
}

impl Waragraph {
    pub fn from_gfa(gfa: &GFA<usize, ()>) -> Result<Self> {
        let node_count = gfa.segments.len();
        let edge_count = gfa.links.len();

        let mut node_sum_lens = Vec::with_capacity(node_count);
        let mut node_lens = Vec::with_capacity(node_count);
        let mut sequences = Vec::with_capacity(node_count);
        let mut sum = 0;

        for seg in gfa.segments.iter() {
            sequences.push(seg.sequence.clone());
            let len = seg.sequence.len();
            node_sum_lens.push(sum);
            node_lens.push(len as u32);
            sum += len;
        }

        let total_len = sum;

        let mut adj_tris: TriMatI<u8, u32> =
            TriMatI::new((node_count, node_count));
        let mut d0_tris: TriMatI<i8, u32> =
            TriMatI::new((edge_count, node_count));

        let mut edges: FxHashMap<(Node, Node), u32> = FxHashMap::default();

        for edge in gfa.links.iter() {
            let ei = edges.len();

            let from = edge.from_segment - 1;
            let to = edge.to_segment - 1;

            adj_tris.add_triplet(to, from, 1);

            d0_tris.add_triplet(ei, from, -1);
            d0_tris.add_triplet(ei, to, 1);

            let n_f = Node::from(from as u32);
            let n_t = Node::from(to as u32);

            edges.insert((n_f, n_t), ei as u32);
        }

        let adj_n_n = adj_tris.to_csc();
        let d0 = d0_tris.to_csc();

        let mut path_names = BiBTreeMap::default();
        let mut path_names_prefixes = BTreeMap::default();
        // let mut path_names = HashMap::default();

        let mut path_lens = Vec::new();
        let mut path_offsets = Vec::new();

        let mut path_nodes = Vec::new();
        let mut path_invert = Vec::new();

        dbg!();
        let paths = gfa
            .paths
            .iter()
            .enumerate()
            .map(|(ix, path)| {
                dbg!(ix);
                let path_ix = Path::from(ix);
                let name = path.path_name.as_bstr();

                path_names.insert(path_ix, name.as_bytes().into());

                {
                    fn parse_usize(bs: &[u8]) -> Option<usize> {
                        let s = bs.to_str().ok()?;
                        s.parse::<usize>().ok()
                    }

                    let mut split = name.splitn_str(2, ":");
                    let name = split.next();
                    let range = split.next().and_then(|s| {
                        let mut split = s.splitn_str(2, "-");
                        let from = split.next().and_then(parse_usize)?;
                        let to = split.next().and_then(parse_usize)?;
                        Some((from, to))
                    });

                    match (name, range) {
                        (Some(name), Some((from, _to))) => {
                            path_offsets.push(from);
                            path_names_prefixes.insert(name.to_vec(), path_ix);
                        }
                        _ => {
                            path_offsets.push(0);
                        }
                    }
                }

                let mut loop_count = FxHashMap::default();

                let mut nodeset = roaring::RoaringBitmap::default();
                let mut inv_set = roaring::RoaringBitmap::default();

                let mut len = 0;

                for (seg, orient) in path.iter() {
                    // let node = Node::from((seg - 1) as u32);
                    // let strand = Strand::new(node, orient.is_reverse());

                    let i = (seg - 1) as u32;
                    // let v = if orient.is_reverse() { -1 } else { 1 };

                    len += node_lens[i as usize] as usize;

                    if orient.is_reverse() {
                        inv_set.insert(i);
                    }
                    nodeset.insert(i);

                    *loop_count.entry(i).or_default() += 1u32;
                }

                path_nodes.push(nodeset);
                path_lens.push(len);

                let mut ids: Vec<(u32, u32)> = loop_count.into_iter().collect();

                ids.sort_by_key(|(i, _)| *i);
                ids.dedup_by_key(|(i, _)| *i);

                let (indices, data) = ids.into_iter().unzip();

                CsVecI::new(node_count, indices, data)
            })
            .collect::<Vec<_>>();

        let mut path_sum_lens: Vec<Vec<(Node, usize)>> = Vec::new();

        for path in paths.iter() {
            let mut sum = 0usize;
            let mut cache = Vec::new();

            for (node_ix, _val) in path.iter() {
                let len = node_lens[node_ix] as usize;
                let val = len;
                cache.push(((node_ix as u32).into(), sum));
                sum += val;
            }

            path_sum_lens.push(cache);
        }
        let path_sum_lens = Arc::new(path_sum_lens);

        Ok(Self {
            node_count,
            sequences,
            total_len,
            node_sum_lens,
            node_lens,

            edges,

            adj_n_n,
            d0,

            path_names,
            path_names_prefixes,
            path_lens,
            path_sum_lens,
            paths,

            // path_indices,
            path_offsets,

            path_nodes,
            path_invert,
        })
    }

    pub fn path_count(&self) -> usize {
        self.path_lens.len()
    }

    // pub fn path_name(&self, path: usize) -> Option<&IVec> {
    pub fn path_name(&self, path: Path) -> Option<&Vec<u8>> {
        self.path_names.get_by_left(&path)
    }

    pub fn path_index(&self, name: &[u8]) -> Option<Path> {
        self.path_names
            .get_by_right(name)
            .or_else(|| self.path_names_prefixes.get(name))
            .copied()
    }

    pub fn path_offset(&self, path: Path) -> usize {
        self.path_offsets[usize::from(path)]
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn total_len(&self) -> usize {
        self.total_len
    }

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

    pub fn sample_node_lengths_db(
        &self,
        nsamples: usize,
        // pos_offset: usize,
        // len: usize,
        view: &ViewDiscrete1D,
        // out: &mut Vec<(Node, usize)>,
        out: &mut Vec<[u32; 2]>,
        // tree: &sled::Db
    ) {
        out.clear();

        let range = view.range();
        let pos_offset = range.start;
        let len = range.end - range.start;

        let pos_end = pos_offset + len;

        let slice = &self.node_sum_lens;
        let sample_width = len / nsamples;

        let sample_point = |p| match slice.binary_search(&p) {
            Ok(i) => i,
            Err(i) => {
                if i == 0 {
                    i
                } else {
                    i - 1
                }
            }
        };

        let p0 = pos_offset;

        for i in 0..=nsamples {
            let p = p0 + i * sample_width;
            let ix = sample_point(p);
            let offset = self.node_sum_lens[ix];

            let node = Node(ix as u32);
            let rem = p - offset;

            out.push([node.0, rem as u32]);
        }
    }

    pub fn sample_node_lengths(
        &self,
        nsamples: usize,
        // pos_offset: usize,
        // len: usize,
        view: &ViewDiscrete1D,
        out: &mut Vec<(Node, usize)>,
    ) {
        out.clear();

        let range = view.range();
        let pos_offset = range.start;
        let len = range.end - range.start;

        let pos_end = pos_offset + len;

        let slice = &self.node_sum_lens;
        let sample_width = len / nsamples;

        let sample_point = |p| match slice.binary_search(&p) {
            Ok(i) => i,
            Err(i) => {
                if i == 0 {
                    i
                } else {
                    i - 1
                }
            }
        };

        // let p0 = pos_offset + sample_width / 2;
        let p0 = pos_offset;

        for i in 0..=nsamples {
            let p = p0 + i * sample_width;
            let ix = sample_point(p);
            let offset = self.node_sum_lens[ix];

            let node = Node(ix as u32);
            let rem = p - offset;

            out.push((node, rem));
        }
    }

    pub fn alloc_node_length_buf(
        &self,
        engine: &mut VkEngine,
        name: Option<&str>,
        usage: vk::BufferUsageFlags,
    ) -> Result<BufferIx> {
        let mut sum = 0;
        self.alloc_with_nodes(engine, name, usage, |node| {
            let len = self.node_lens[node.0 as usize];

            let lb = (len as u32).to_ne_bytes();
            let is = (sum as u32).to_ne_bytes();

            sum += len;

            // TODO use bytemuck
            [is[0], is[1], is[2], is[3], lb[0], lb[1], lb[2], lb[3]]
        })
    }

    pub fn alloc_with_nodes<F, const N: usize>(
        &self,
        engine: &mut VkEngine,
        name: Option<&str>,
        usage: vk::BufferUsageFlags,
        mut f: F,
    ) -> Result<BufferIx>
    where
        F: FnMut(Node) -> [u8; N],
    {
        let data = (0..self.node_count)
            .map(|ni| {
                let node = Node(ni as u32);
                f(node)
            })
            .flatten()
            .collect::<Vec<u8>>();

        let buf_ix = engine.with_allocators(|ctx, res, alloc| {
            let buf = res.allocate_buffer(
                ctx,
                alloc,
                gpu_allocator::MemoryLocation::GpuOnly,
                N,
                self.node_count,
                usage,
                name,
            )?;

            let ix = res.insert_buffer(buf);
            Ok(ix)
        })?;

        let staging_buf = Arc::new(AtomicCell::new(None));

        let arc = staging_buf.clone();

        let fill_buf_batch =
            move |ctx: &VkContext,
                  res: &mut GpuResources,
                  alloc: &mut Allocator,
                  cmd: vk::CommandBuffer| {
                let buf = &mut res[buf_ix];

                let staging = buf.upload_to_self_bytes(
                    ctx,
                    alloc,
                    bytemuck::cast_slice(&data),
                    cmd,
                )?;

                arc.store(Some(staging));

                Ok(())
            };

        let batches = vec![&fill_buf_batch as &_];

        let fence = engine.submit_batches_fence_alt(batches.as_slice())?;

        engine.block_on_fence(fence)?;

        for buf in staging_buf.take() {
            buf.cleanup(&engine.context, &mut engine.allocator).ok();
        }

        Ok(buf_ix)
    }
}
