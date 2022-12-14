use roaring::{RoaringBitmap, RoaringTreemap};
use std::collections::BTreeMap;
use std::io::prelude::*;
use std::io::BufReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Node(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct OrientedNode(u32);

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

impl OrientedNode {
    #[inline]
    pub fn new(id: u32, reverse: bool) -> Self {
        OrientedNode((id << 1) | reverse as u32)
    }

    #[inline]
    pub fn node(&self) -> Node {
        Node(self.0 >> 1)
    }

    #[inline]
    pub fn is_reverse(&self) -> bool {
        (self.0 & 1) == 1
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

#[derive(Debug, Clone)]
pub struct PathIndex {
    pub segment_offsets: roaring::RoaringTreemap,
    pub sequence_total_len: usize,
    pub segment_id_range: (u32, u32),

    pub path_names: BTreeMap<String, usize>,
    pub path_steps: Vec<Vec<OrientedNode>>,

    pub path_step_offsets: Vec<roaring::RoaringTreemap>,
}

pub struct PathStepRangeIter<'a> {
    path_id: usize,
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
    pub fn pangenome_len(&self) -> usize {
        self.sequence_total_len
    }

    pub fn path_steps<'a>(
        &'a self,
        path_name: &str,
    ) -> Option<&'a [OrientedNode]> {
        let ix = self.path_names.get(path_name)?;
        self.path_steps.get(*ix).map(|s| s.as_slice())
    }

    pub fn step_at_pos(
        &self,
        path_name: &str,
        pos: usize,
    ) -> Option<OrientedNode> {
        let path_id = *self.path_names.get(path_name)?;
        let offsets = self.path_step_offsets.get(path_id)?;
        let steps = self.path_steps.get(path_id)?;
        let pos_rank = offsets.rank(pos as u64) as usize;
        steps.get(pos_rank).copied()
    }

    pub fn path_step_range_iter<'a>(
        &'a self,
        path_name: &str,
        pos_range: std::ops::Range<u64>,
    ) -> Option<PathStepRangeIter<'a>> {
        let path_id = *self.path_names.get(path_name)?;
        let offsets = self.path_step_offsets.get(path_id)?;

        let start = pos_range.start;
        let end = pos_range.end;
        let start_rank = offsets.rank(start);
        let end_rank = offsets.rank(end);

        let steps = {
            let path_steps = self.path_steps.get(path_id)?;

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

            sequence_total_len += len;
            segment_offsets.push(sequence_total_len as u64);
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

        let mut path_names = BTreeMap::default();

        let mut path_steps: Vec<Vec<OrientedNode>> = Vec::new();
        let mut path_step_offsets: Vec<RoaringTreemap> = Vec::new();
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
            path_names.insert(name.to_string(), path_steps.len());

            let mut pos = 0;

            let mut parsed_steps = Vec::new();

            let mut offsets = RoaringTreemap::new();

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

                pos += len;
            }

            path_steps.push(parsed_steps);
            path_step_offsets.push(offsets);
        }

        Ok(Self {
            path_names,
            path_steps,
            path_step_offsets,

            segment_offsets,
            segment_id_range: seg_id_range,
            sequence_total_len,
        })
    }
}
