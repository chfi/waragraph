use bimap::BiBTreeMap;
use roaring::{RoaringBitmap, RoaringTreemap};
use std::collections::BTreeMap;
use std::io::prelude::*;
use std::io::BufReader;

use self::iter::PangenomeNodePosRangeIter;
use self::iter::PangenomePathDataPosRangeIter;

pub mod iter;
pub mod sampling;

pub mod spoke;

pub mod matrix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Node(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct OrientedNode(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Edge {
    pub from: OrientedNode,
    pub to: OrientedNode,
}

impl From<(OrientedNode, OrientedNode)> for Edge {
    fn from((from, to): (OrientedNode, OrientedNode)) -> Self {
        Self { from, to }
    }
}

impl Edge {
    pub fn new(from: OrientedNode, to: OrientedNode) -> Self {
        Self { from, to }
    }

    pub fn endpoints(&self) -> (OrientedNode, OrientedNode) {
        let Edge { from, to } = self;
        match (from.is_reverse(), to.is_reverse()) {
            (false, false) => (from.node_end(), to.node_start()),
            (false, true) => (from.node_end(), to.node_end()),
            (true, false) => (from.node_start(), to.node_start()),
            (true, true) => (from.node_start(), to.node_end()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Bp(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct PathId(u32);

impl Node {
    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn as_forward(&self) -> OrientedNode {
        OrientedNode::new(self.0, false)
    }

    #[inline]
    pub fn as_reverse(&self) -> OrientedNode {
        OrientedNode::new(self.0, true)
    }
}

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

impl From<u32> for OrientedNode {
    fn from(u: u32) -> OrientedNode {
        OrientedNode(u)
    }
}

impl OrientedNode {
    #[inline]
    pub fn new(id: u32, reverse: bool) -> Self {
        OrientedNode((id << 1) | reverse as u32)
    }

    #[inline]
    pub fn node_start(&self) -> OrientedNode {
        let i = self.node().0;
        Self::new(i, true)
    }

    #[inline]
    pub fn node_end(&self) -> OrientedNode {
        let i = self.node().0;
        Self::new(i, false)
    }

    #[inline]
    pub fn node(&self) -> Node {
        Node(self.0 >> 1)
    }

    #[inline]
    pub fn is_reverse(&self) -> bool {
        (self.0 & 1) == 1
    }

    #[inline]
    pub fn flip(self) -> Self {
        Self::new(self.node().0, !self.is_reverse())
    }

    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }
}

impl From<u64> for Bp {
    fn from(u: u64) -> Bp {
        Bp(u)
    }
}

impl From<Bp> for u64 {
    fn from(bp: Bp) -> u64 {
        bp.0
    }
}

impl PathId {
    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for PathId {
    fn from(u: u32) -> PathId {
        PathId(u)
    }
}

impl From<usize> for PathId {
    fn from(u: usize) -> PathId {
        PathId(u as u32)
    }
}

#[derive(Debug, Clone)]
pub struct Waragraph {
    pub path_index: PathIndex,
    pub path_node_sets: Vec<RoaringBitmap>,
}

impl Waragraph {
    pub fn from_gfa(
        gfa_path: impl AsRef<std::path::Path>,
    ) -> std::io::Result<Self> {
        let path_index = PathIndex::from_gfa(gfa_path)?;

        let mut path_node_sets = Vec::new();

        for steps in path_index.path_steps.iter() {
            let set =
                steps.iter().map(|s| s.node().0).collect::<RoaringBitmap>();

            path_node_sets.push(set);
        }

        Ok(Waragraph {
            path_index,
            path_node_sets,
        })
    }
}

#[derive(Default, Clone)]
pub struct NodeSet {
    set: roaring::RoaringBitmap,
}

#[derive(Debug, Clone)]
pub struct PathIndex {
    pub segment_offsets: roaring::RoaringTreemap,
    pub node_count: usize,
    pub sequence_total_len: Bp,
    pub segment_id_range: (u32, u32),

    edges: Vec<Edge>,

    pub path_names: BiBTreeMap<PathId, String>,
    // pub path_names: BTreeMap<String, usize>,
    pub path_steps: Vec<Vec<OrientedNode>>,

    pub path_step_offsets: Vec<roaring::RoaringTreemap>,
    pub path_node_sets: Vec<roaring::RoaringBitmap>,
}

pub struct PathStepRangeIter<'a> {
    path_id: PathId,
    pos_range: std::ops::Range<u64>,
    // start_pos: usize,
    // end_pos: usize,
    steps: Box<dyn Iterator<Item = (usize, &'a OrientedNode)> + 'a>,
    // first_step_start_pos: u32,
    // last_step_end_pos: u32,
}

impl<'a> Iterator for PathStepRangeIter<'a> {
    type Item = (usize, &'a OrientedNode);

    fn next(&mut self) -> Option<Self::Item> {
        self.steps.next()
    }
}

impl PathIndex {
    pub fn directed_adjacency_matrix(
        node_count: usize,
        edges: impl Iterator<Item = Edge>,
    ) -> sprs::CsMat<u8> {
        use sprs::TriMat;

        // TODO build the compressed matrix directly

        // node endpoint space
        let n = node_count * 2;

        let mut mat: TriMat<u8> = TriMat::new((n, n));

        for edge in edges {
            let (from, to) = edge.endpoints();
            let fi = from.ix();
            let ti = to.ix();

            // entry in row i, col j is 1 when there's an edge j->i
            mat.add_triplet(ti, fi, 1);
        }

        mat.to_csc()
    }

    pub fn edges_iter<'a>(&'a self) -> impl Iterator<Item = &'a Edge> {
        self.edges.iter()
    }

    pub fn pangenome_len(&self) -> Bp {
        self.sequence_total_len
    }

    #[inline]
    pub fn node_offset_length(&self, node: Node) -> (Bp, Bp) {
        let i = node.0 as u64;
        let i = node.0 as u64;
        let offset = self.segment_offsets.select(i).unwrap_or_default();
        let next = self
            .segment_offsets
            .select(i + 1)
            .unwrap_or(self.pangenome_len().0);

        let length = Bp(next - offset);
        let offset = Bp(offset);

        (offset, length)
    }

    #[inline]
    pub fn node_offset(&self, node: Node) -> Bp {
        self.node_offset_length(node).0
    }

    pub fn node_at_pangenome_pos(&self, pos: Bp) -> Option<Node> {
        if pos > self.sequence_total_len {
            return None;
        }
        let rank = self.segment_offsets.rank(pos.0);
        Some(Node::from(rank as u32))
    }

    #[inline]
    pub fn node_length(&self, node: Node) -> Bp {
        self.node_offset_length(node).1
    }

    pub fn pos_range_nodes(
        &self,
        pos_range: std::ops::Range<u64>,
    ) -> std::ops::RangeInclusive<Node> {
        let s = pos_range.start;
        let e = pos_range.end;

        let start_rank = self.segment_offsets.rank(s);
        let end_rank = self.segment_offsets.rank(e);

        let first = Node::from(start_rank as usize - 1);
        let last = Node::from(end_rank as usize - 1);

        first..=last
    }

    pub fn nodes_pan_range_iter<'i>(
        &'i self,
        pan_range: std::ops::Range<u64>,
    ) -> PangenomeNodePosRangeIter<'i> {
        PangenomeNodePosRangeIter::new_pos_range(self, pan_range)
    }

    pub fn path_data_pan_range_iter<'index, 'data, T>(
        &'index self,
        pan_range: std::ops::Range<u64>,
        path_id: PathId,
        data: &'data [T],
    ) -> PangenomePathDataPosRangeIter<'index, 'data, T> {
        PangenomePathDataPosRangeIter::new_pos_range(
            self, pan_range, path_id, data,
        )
    }

    pub fn path_steps<'a>(
        &'a self,
        path_name: &str,
    ) -> Option<&'a [OrientedNode]> {
        let id = self.path_names.get_by_right(path_name)?;
        self.path_steps.get(id.ix()).map(|s| s.as_slice())
    }

    pub fn step_at_pos(
        &self,
        path_name: &str,
        pos: usize,
    ) -> Option<OrientedNode> {
        let path_id = self.path_names.get_by_right(path_name)?;
        let offsets = self.path_step_offsets.get(path_id.ix())?;
        let steps = self.path_steps.get(path_id.ix())?;
        let pos_rank = offsets.rank(pos as u64) as usize;
        steps.get(pos_rank).copied()
    }

    pub fn path_step_range_iter<'a>(
        &'a self,
        path_name: &str,
        pos_range: std::ops::Range<u64>,
    ) -> Option<PathStepRangeIter<'a>> {
        let path_id = *self.path_names.get_by_right(path_name)?;
        let offsets = self.path_step_offsets.get(path_id.ix())?;

        let start = pos_range.start;
        let end = pos_range.end;
        let start_rank = offsets.rank(start);
        let end_rank = offsets.rank(end);

        let steps = {
            let path_steps = self.path_steps.get(path_id.ix())?;

            let skip = (start_rank as usize).checked_sub(1).unwrap_or_default();
            let take = end_rank as usize - skip;
            let iter = path_steps
                .iter()
                .skip(skip)
                .take(take)
                .enumerate()
                .map(move |(ix, step)| (skip + ix, step))
                .fuse();

            Box::new(iter) as Box<dyn Iterator<Item = _>>
        };

        Some(PathStepRangeIter {
            path_id,
            pos_range,
            steps,
            // first_step_start_pos,
            // last_step_end_pos,
        })
    }

    pub fn from_gfa(
        gfa_path: impl AsRef<std::path::Path>,
    ) -> std::io::Result<Self> {
        let gfa = std::fs::File::open(&gfa_path)?;
        let mut gfa_reader = BufReader::new(gfa);

        let mut line_buf = Vec::new();

        let mut segment_offsets = roaring::RoaringTreemap::new();
        let mut seg_lens = Vec::new();
        let mut sequence_total_len = 0;

        let mut seg_id_range = (std::u32::MAX, 0u32);

        loop {
            line_buf.clear();

            let len = gfa_reader.read_until(0xA, &mut line_buf)?;
            if len == 0 {
                break;
            }

            let line = &line_buf[..len - 1];

            if !matches!(line.first(), Some(b'S')) {
                continue;
            }

            let mut fields = line.split(|&c| c == b'\t');

            let Some((name, seq)) = fields.next().and_then(|_type| {
                let name = fields.next()?;
                let seq = fields.next()?;
                Some((name, seq))
            }) else {
                continue;
            };

            let seg_id = btoi::btou::<u32>(name).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;

            seg_id_range.0 = seg_id_range.0.min(seg_id);
            seg_id_range.1 = seg_id_range.1.max(seg_id);

            let len = seq.len();

            segment_offsets.push(sequence_total_len as u64);
            sequence_total_len += len;
            seg_lens.push(len);
        }

        assert!(
        seg_id_range.1 - seg_id_range.0 == (seg_lens.len() as u32) - 1,
        "GFA segments must be tightly packed: min ID {}, max ID {}, node count {}, was {}",
        seg_id_range.0, seg_id_range.1, seg_lens.len(),
        seg_id_range.1 - seg_id_range.0,
        );

        let gfa = std::fs::File::open(&gfa_path)?;
        let mut gfa_reader = BufReader::new(gfa);

        let mut edges = Vec::new();

        loop {
            line_buf.clear();

            let len = gfa_reader.read_until(0xA, &mut line_buf)?;
            if len == 0 {
                break;
            }

            let line = &line_buf[..len - 1];

            if !matches!(line.first(), Some(b'L')) {
                continue;
            }

            let fields = line.split(|&c| c == b'\t');
            let edge = Self::parse_gfa_link(seg_id_range.0, fields)?;
            edges.push(edge);
        }
        println!("parsed {} edges", edges.len());

        edges.sort();

        let node_count = seg_lens.len();

        let gfa = std::fs::File::open(&gfa_path)?;
        let mut gfa_reader = BufReader::new(gfa);

        let mut path_names = BiBTreeMap::default();

        let mut path_steps: Vec<Vec<OrientedNode>> = Vec::new();
        let mut path_step_offsets: Vec<RoaringTreemap> = Vec::new();
        let mut path_node_sets: Vec<RoaringBitmap> = Vec::new();
        // let mut path_pos: Vec<Vec<usize>> = Vec::new();

        loop {
            line_buf.clear();

            let len = gfa_reader.read_until(b'\n', &mut line_buf)?;
            if len == 0 {
                break;
            }

            let line = &line_buf[..len];
            if !matches!(line.first(), Some(b'P')) {
                continue;
            }

            let mut fields = line.split(|&c| c == b'\t');

            let Some((name, steps)) = fields.next().and_then(|_type| {
                let name = fields.next()?;
                let steps = fields.next()?;
                Some((name, steps))
            }) else {
                continue;
            };

            let name = std::str::from_utf8(name).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;
            path_names.insert(PathId::from(path_steps.len()), name.to_string());

            let mut pos = 0;

            let mut parsed_steps = Vec::new();

            let mut offsets = RoaringTreemap::new();
            let mut path_nodes = RoaringBitmap::new();

            let steps = steps.split(|&c| c == b',');

            for step in steps {
                let (seg, orient) = step.split_at(step.len() - 1);
                let seg_id = btoi::btou::<u32>(seg).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                })?;
                let seg_ix = seg_id - seg_id_range.0;
                let len = seg_lens[seg_ix as usize];

                let is_rev = orient == b"-";

                let step = OrientedNode::new(seg_ix as u32, is_rev);

                parsed_steps.push(step);
                offsets.push(pos as u64);
                path_nodes.insert(seg_ix);

                pos += len;
            }

            path_steps.push(parsed_steps);
            path_step_offsets.push(offsets);
            path_node_sets.push(path_nodes);
        }

        Ok(Self {
            path_names,
            path_steps,
            path_step_offsets,
            path_node_sets,

            segment_offsets,
            node_count,
            segment_id_range: seg_id_range,
            sequence_total_len: Bp(sequence_total_len as u64),

            edges,
        })
    }
}

impl PathIndex {
    fn parse_gfa_link<'a>(
        min_id: u32,
        mut fields: impl Iterator<Item = &'a [u8]>,
    ) -> std::io::Result<Edge> {
        let fields_missing =
            || std::io::Error::new(std::io::ErrorKind::Other, "Fields missing");

        let parse_id = |bs: &[u8]| {
            btoi::btou::<u32>(bs).map(|i| i - min_id).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })
        };

        let parse_orient = |bs: &[u8]| match bs {
            b"+" => Ok(false),
            b"-" => Ok(true),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Could not parse link orientation",
            )),
        };

        let _type = fields.next().ok_or_else(&fields_missing)?;

        let from = fields.next().ok_or_else(&fields_missing)?;
        let from_orient = fields.next().ok_or_else(&fields_missing)?;

        let to = fields.next().ok_or_else(&fields_missing)?;
        let to_orient = fields.next().ok_or_else(&fields_missing)?;

        let from_id = parse_id(from)?;
        let from_rev = parse_orient(from_orient)?;

        let to_id = parse_id(to)?;
        let to_rev = parse_orient(to_orient)?;

        let from = OrientedNode::new(from_id, from_rev);
        let to = OrientedNode::new(to_id, to_rev);

        Ok(Edge::new(from, to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const GFA_PATH: &'static str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test/data/",
        "A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa"
    );

    #[test]
    fn node_lengths() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();

        let node_lengths = (0..10)
            .map(|i| index.node_length(Node(i)).0)
            .collect::<Vec<_>>();

        let expected = vec![44, 12, 19, 1, 1, 13, 1, 1, 1, 2];

        assert_eq!(node_lengths, expected);

        let n = index.node_count as u32;
        let node_lengths = ((n - 10)..n)
            .map(|i| index.node_length(Node(i)).0)
            .collect::<Vec<_>>();

        let expected = vec![1, 1, 1, 3, 1, 1, 2, 1, 1, 12];
        assert_eq!(node_lengths, expected);
    }

    #[test]
    fn pangenome_nodes_range() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();
        let total_len = index.pangenome_len();

        let pos_range = 44..55;
        let range0 = index.pos_range_nodes(pos_range);

        let mut last_start = (total_len.0 - 12);
        last_start -= 1;

        let pos_range = last_start..total_len.0;
        let range1 = index.pos_range_nodes(pos_range);

        assert_eq!(range0, Node(1)..=Node(1));
        assert_eq!(range1, Node(4964)..=Node(4965));
    }
}
