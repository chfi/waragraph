use anyhow::Result;
use roaring::RoaringBitmap;
use std::collections::BTreeMap;
use std::io::prelude::*;
use std::io::BufReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathStep {
    pub node: u32,
    pub reverse: bool,
}

pub struct PathIndex {
    pub segment_id_range: (usize, usize),
    pub segment_lens: Vec<usize>,

    path_names: BTreeMap<String, usize>,
    // path_names: Vec<String>,
    path_steps: Vec<Vec<PathStep>>,

    // path_step_offsets: Vec<Vec<usize>>,
    path_step_offsets: Vec<roaring::RoaringBitmap>,
}

pub struct PathStepRangeIter<'a> {
    path_id: usize,
    pos_range: std::ops::Range<u32>,
    // start_pos: usize,
    // end_pos: usize,
    steps: Box<dyn Iterator<Item = (usize, &'a PathStep)> + 'a>,
    // first_step_start_pos: u32,
    // last_step_end_pos: u32,
}

impl<'a> Iterator for PathStepRangeIter<'a> {
    type Item = (usize, &'a PathStep);

    fn next(&mut self) -> Option<Self::Item> {
        self.steps.next()
    }
}

impl PathIndex {
    pub fn path_steps<'a>(&'a self, path_name: &str) -> Option<&'a [PathStep]> {
        let ix = self.path_names.get(path_name)?;
        self.path_steps.get(*ix).map(|s| s.as_slice())
    }

    pub fn path_step_range_iter<'a>(
        &'a self,
        path_name: &str,
        pos_range: std::ops::Range<u32>,
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

    pub fn from_gfa(gfa_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let gfa = std::fs::File::open(&gfa_path)?;
        let mut gfa_reader = BufReader::new(gfa);

        let mut line_buf = Vec::new();

        let mut seg_lens = Vec::new();

        let mut seg_id_range = (std::usize::MAX, 0usize);
        // dbg!();

        loop {
            line_buf.clear();

            let len = gfa_reader.read_until(0xA, &mut line_buf)?;
            if len == 0 {
                break;
            }

            let line = &line_buf[..len];
            let line_str = std::str::from_utf8(&line)?;
            // println!("{line_str}");

            if !matches!(line.first(), Some(b'S')) {
                continue;
            }

            let mut fields = line_str.split(|c| c == '\t');

            let Some((name, seq)) = fields.next().and_then(|_type| {
                let name = fields.next()?.trim();
                let seq = fields.next()?.trim();
                Some((name, seq))
            }) else {
                continue;
            };
            let seg_id = name.parse::<usize>()?;

            seg_id_range.0 = seg_id_range.0.min(seg_id);
            seg_id_range.1 = seg_id_range.1.max(seg_id);

            let len = seq.len();
            seg_lens.push(len);
        }

        assert!(
        seg_id_range.1 - seg_id_range.0 == seg_lens.len() - 1,
        "GFA segments must be tightly packed: min ID {}, max ID {}, node count {}, was {}",
        seg_id_range.0, seg_id_range.1, seg_lens.len(),
        seg_id_range.1 - seg_id_range.0,
        );

        let gfa = std::fs::File::open(&gfa_path)?;
        let mut gfa_reader = BufReader::new(gfa);

        let mut path_names = BTreeMap::default();

        let mut path_steps: Vec<Vec<PathStep>> = Vec::new();
        let mut path_step_offsets: Vec<RoaringBitmap> = Vec::new();
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

            let name = std::str::from_utf8(name)?;
            path_names.insert(name.to_string(), path_steps.len());

            let mut pos = 0;

            let mut parsed_steps = Vec::new();

            let mut offsets = RoaringBitmap::new();

            let steps = steps.split(|&c| c == b',');

            for step in steps {
                let (seg, orient) = step.split_at(step.len() - 1);
                let seg_id = btoi::btou::<usize>(seg)?;
                let seg_ix = seg_id - seg_id_range.0;
                let len = seg_lens[seg_ix];

                let is_rev = orient == b"-";

                let step = PathStep {
                    node: seg_ix as u32,
                    reverse: is_rev,
                };
                parsed_steps.push(step);
                offsets.push(pos as u32);

                pos += len;
            }

            path_steps.push(parsed_steps);
            path_step_offsets.push(offsets);
        }

        Ok(Self {
            path_names,
            path_steps,
            path_step_offsets,

            segment_id_range: seg_id_range,
            segment_lens: seg_lens,
        })
    }
}
