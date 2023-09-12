use ahash::{AHashMap, HashMap};
use arrow2::{
    array::{BinaryArray, PrimitiveArray, StructArray, UInt32Array, Utf8Array},
    buffer::Buffer,
    datatypes::{DataType, Field, Schema},
    offset::OffsetsBuffer,
};

use std::io::prelude::*;

use crate::graph::{Bp, Edge, Node, OrientedNode, PathId};

pub struct ArrowGFA {
    // using 32-bit IDs & indices, even for sequence, for now; since
    // wasm is limited to 32 bits for the forseeable future (and
    // single memories), it's probably better to implement a kind of
    // paging/chunking system so that we can load in only the relevant
    // parts of the graph into the wasm linear memory
    //
    // each page would only need to hold 64 bit offsets at the most,
    // with the array data being 0-offset, so this also provides some
    // compression (especially for e.g. paths)
    segment_sequences: BinaryArray<i32>,
    segment_names: Option<Utf8Array<i32>>,

    link_from: UInt32Array,
    link_to: UInt32Array,

    path_names: Utf8Array<i32>,
    path_steps: Vec<UInt32Array>,
}

pub struct PathMetadata<'a> {
    name: &'a str,
    step_count: usize,
    unique_segments: usize,
}

impl ArrowGFA {
    pub fn segment_count(&self) -> usize {
        self.segment_sequences.len()
    }

    pub fn link_count(&self) -> usize {
        self.link_from.len()
    }

    pub fn path_count(&self) -> usize {
        self.path_names.len()
    }

    pub fn segment_sequence(&self, segment_index: u32) -> &[u8] {
        self.segment_sequences.get(segment_index as usize).unwrap()
    }

    pub fn segment_len(&self, segment_index: u32) -> usize {
        self.segment_sequence(segment_index).len()
    }

    pub fn segment_name(&self, segment_index: u32) -> Option<&str> {
        self.segment_names.as_ref()?.get(segment_index as usize)
    }

    pub fn segment_index(&self, segment_name: &str) -> Option<u32> {
        let names = self.segment_names.as_ref()?;
        let (i, _) = names
            .iter()
            .filter_map(|s| s)
            .enumerate()
            .find(|&(i, name)| name == segment_name)?;
        Some(i as u32)
    }

    pub fn segment_sequences_iter(
        &self,
    ) -> arrow2::array::BinaryValueIter<'_, i32> {
        self.segment_sequences.values_iter()
    }

    pub fn segment_sequences_array(&self) -> &BinaryArray<i32> {
        &self.segment_sequences
    }

    /// O(n) in number of paths
    pub fn path_name_index(&self, path_name: &str) -> Option<u32> {
        let (path_ix, _) = self
            .path_names
            .iter()
            .enumerate()
            .find(|(_ix, name)| name.is_some_and(|n| n == path_name))?;
        Some(path_ix as u32)
    }

    // pub fn path_vector_offsets(
    //     &self,
    //     path_index: u32,
    // ) -> sprs::CsVecI<u32, u32> {
    //     let dim = self.segment_sequences.len();
    //     //
    // }

    pub fn path_vector_sparse_u32(
        &self,
        path_index: u32,
    ) -> sprs::CsVecI<u32, u32> {
        let dim = self.segment_sequences.len();

        let mut data = vec![0u32; dim];

        let steps = &self.path_steps[path_index as usize];

        // step vectors are dense so can use values() here
        for step_h in steps.values_iter() {
            let _is_rev = (step_h & 1) == 1;
            let segment_index = step_h >> 1;
            data[segment_index as usize] += 1;
        }

        let mut indices: Vec<u32> = Vec::new();
        let mut data = data
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| {
                if v > 0 {
                    indices.push(i as u32);
                    Some(v)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        indices.shrink_to_fit();
        data.shrink_to_fit();

        let vector = sprs::CsVecI::new(dim, indices, data);

        vector
    }
}

pub fn arrow_graph_from_gfa<S: BufRead + Seek>(
    mut gfa_reader: S,
) -> std::io::Result<ArrowGFA> {
    let mut line_buf = Vec::new();

    // let mut seg_sequences: Vec<Vec<u8>> = Vec::new();
    // let mut seg_names: Vec<String> = Vec::new();

    // let mut seg_seq_offsets: OffsetsBuffer<i64> = OffsetsBuffer::new();
    let mut seg_seq_offsets: Vec<i32> = Vec::new();
    let mut seg_seq_bytes: Vec<u8> = Vec::new();

    let mut seg_name_offsets: Vec<i32> = Vec::new();
    let mut seg_name_str: Vec<u8> = Vec::new();

    gfa_reader.rewind()?;

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

        let Some((name, seq, opt)) = fields.next().and_then(|_type| {
                let name = fields.next()?;
                let seq = fields.next()?;
                let opt = fields.next();
                Some((name, seq, opt))
            }) else {
                continue;
            };

        let _opt = opt;

        // let seg_index = seg_name_index_map.len();

        let offset = seg_seq_bytes.len();
        seg_seq_offsets.push(offset as i32);
        seg_seq_bytes.extend(seq);

        let name_offset = seg_name_str.len();
        seg_name_offsets.push(name_offset as i32);
        seg_name_str.extend(name);

        let Some(opt_fields) = opt
        else { continue; };
    }

    let offset = seg_seq_bytes.len();
    seg_seq_offsets.push(offset as i32);
    let name_offset = seg_name_str.len();
    seg_name_offsets.push(name_offset as i32);

    let name_offsets = OffsetsBuffer::try_from(seg_name_offsets).unwrap();

    let seg_name_arr = Utf8Array::new(
        DataType::Utf8,
        name_offsets,
        Buffer::from(seg_name_str),
        None,
    );

    let seq_offsets = OffsetsBuffer::try_from(seg_seq_offsets).unwrap();

    let seg_seq_arr = BinaryArray::new(
        DataType::Binary,
        seq_offsets,
        Buffer::from(seg_seq_bytes),
        None,
    );

    let seg_name_index_map = seg_name_arr
        .iter()
        .enumerate()
        .filter_map(|(i, v)| Some((v?.as_bytes(), i as u32)))
        .collect::<AHashMap<_, _>>();

    let seg_step_to_handle = |seg_name: &[u8], is_reverse: bool| -> u32 {
        let index = *seg_name_index_map.get(seg_name).unwrap();

        let mut handle = index << 1;

        if is_reverse {
            handle |= 1;
        }

        handle
    };

    let mut link_from_arr: Vec<u32> = Vec::new();
    let mut link_to_arr: Vec<u32> = Vec::new();

    gfa_reader.rewind()?;

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

        let mut fields = line.split(|&c| c == b'\t');

        let Some((from_handle, to_handle, overlap, opt)) = fields.next().and_then(|_type| {
            let from_name = fields.next()?;
            let from_is_rev = fields.next()? == b"-";
            let from_h = seg_step_to_handle(from_name, from_is_rev);

            let to_name = fields.next()?;
            let to_is_rev = fields.next()? == b"-";
            let to_h = seg_step_to_handle(to_name, to_is_rev);

            let overlap = fields.next()?;
            let opt = fields.next();
            Some((from_h, to_h, overlap, opt))
        }) else {
            continue;
        };

        link_from_arr.push(from_handle);
        link_to_arr.push(to_handle);

        // TODO store overlap

        let _opt = opt;
    }

    let link_from_arr = UInt32Array::from_vec(link_from_arr);
    let link_to_arr = UInt32Array::from_vec(link_to_arr);

    /*
    let link_array = StructArray::new(
        DataType::Struct(vec![
            Field::new("from", DataType::UInt32, false),
            Field::new("to", DataType::UInt32, false),
        ]),
        vec![link_from_arr.boxed(), link_to_arr.boxed()],
        None,
    );
    */

    let mut path_name_offsets: Vec<i32> = Vec::new();
    let mut path_name_str: Vec<u8> = Vec::new();

    // each step as handle, per path
    let mut path_step_arrs: Vec<UInt32Array> = Vec::new();

    gfa_reader.rewind()?;

    loop {
        line_buf.clear();

        let len = gfa_reader.read_until(0xA, &mut line_buf)?;
        if len == 0 {
            break;
        }

        let line = &line_buf[..len - 1];

        if !matches!(line.first(), Some(b'P')) {
            continue;
        }

        let mut fields = line.split(|&c| c == b'\t');

        let Some((path_name, seg_names, overlaps, opt)) = fields.next().and_then(|_type| {
            let path_name = fields.next()?;
            let seg_names = fields.next()?;
            let overlaps = fields.next()?;
            let opt = fields.next();


            Some((path_name, seg_names, overlaps, opt))
        }) else {
            continue;
        };

        let name_offset = path_name_str.len();
        path_name_offsets.push(name_offset as i32);
        path_name_str.extend(path_name);

        let mut step_vec = Vec::new();

        for (step_index, step_str) in
            seg_names.split(|&c| c == b',').enumerate()
        {
            let (seg_name, seg_orient) = step_str.split_at(step_str.len() - 1);
            let is_rev = seg_orient == b"-";

            let seg_i = seg_step_to_handle(seg_name, is_rev);
            step_vec.push(seg_i);
        }

        path_step_arrs.push(UInt32Array::from_vec(step_vec));
    }

    let name_offset = path_name_str.len();
    path_name_offsets.push(name_offset as i32);
    let name_offsets = OffsetsBuffer::try_from(path_name_offsets).unwrap();

    let path_name_arr = Utf8Array::new(
        DataType::Utf8,
        name_offsets,
        Buffer::from(path_name_str),
        None,
    );

    Ok(ArrowGFA {
        segment_sequences: seg_seq_arr,
        segment_names: Some(seg_name_arr),
        link_from: link_from_arr,
        link_to: link_to_arr,
        path_names: path_name_arr,
        path_steps: path_step_arrs,
    })
}

pub struct Path {
    steps: arrow2::array::PrimitiveArray<u32>,
    // path: arrow2::array::
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct NodeSpan {
    from: Bp,
    to: Bp,
}

#[derive(Debug, Clone)]
pub struct NodeSequences {
    sequences: arrow2::array::BinaryArray<i64>,
    // node_spans: arrow2::array::ListArray<i32>,
}

impl NodeSequences {
    fn from_segments<'a>(
        segments: impl IntoIterator<Item = &'a str>,
    ) -> NodeSequences {
        let mut sequence: Vec<u8> = vec![];
        let mut offsets: Vec<i64> = vec![];

        let mut offset = 0;

        for (seg_id, seq) in segments.into_iter().enumerate() {
            let len = seq.len();
            offsets.push(offset as i64);
            sequence.extend(seq.as_bytes());

            offset += len;
        }
        offsets.push(offset as i64);

        let sequences = arrow2::array::BinaryArray::try_new(
            DataType::LargeBinary,
            offsets.try_into().unwrap(),
            sequence.into(),
            None,
        )
        .unwrap();

        // let sequences = arrow2::array::BinaryArray::from_iter_values(
        //     segments.into_iter().map(|s| s.as_bytes()),
        // );

        NodeSequences { sequences }
    }

    pub fn node_sequence(&self, node: Node) -> Option<&[u8]> {
        self.sequences.get(node.ix())
    }

    pub fn node_span(&self, node: Node) -> NodeSpan {
        let (start, end) = self.sequences.offsets().start_end(node.ix());
        let from = Bp(start as u64);
        let to = Bp(end as u64);
        NodeSpan { from, to }
    }
}

pub struct Waragraph {
    seqs: NodeSequences,

    edges: arrow2::array::StructArray,
}

fn arrow_edges(
    edges: impl IntoIterator<Item = (OrientedNode, OrientedNode)>,
) -> arrow2::array::StructArray {
    use arrow2::array::UInt32Array;

    let (from, to): (Vec<_>, Vec<_>) =
        edges.into_iter().map(|(from, to)| (from.0, to.0)).unzip();

    let from = UInt32Array::from_slice(&from).boxed();
    let to = UInt32Array::from_slice(&to).boxed();

    arrow2::array::StructArray::new(
        DataType::Struct(vec![
            Field::new("from", DataType::UInt32, false),
            Field::new("to", DataType::UInt32, false),
        ]),
        vec![from, to],
        None,
    )
}

#[cfg(test)]
mod tests {

    use super::*;

    /*
    #[test]
    fn node_sequences_test() -> Result<()> {
        // let seqs = ["GCGC", "TT", "TGTTGTGT", "A", "TGT", "AAAA"];
        let seqs = ["GCGC", "TT", "TGTTGTGT", "A", "TGT", "AAAA", "T"];

        println!("input len: {}", seqs.len());

        let nodes = NodeSequences::from_segments(seqs);

        println!("{nodes:#?}");

        println!("seq len: {}", nodes.sequences.len());

        for (i, seq) in nodes.sequences.iter().enumerate() {
            if let Some(seq) = seq {
                let seqstr = std::str::from_utf8(seq).unwrap();
                println!("{i} - {seqstr}");
                //
            }
        }

        let buf = nodes.sequences.values();
        println!("{:?}", buf);

        let offsets = nodes.sequences.offsets();
        println!("{offsets:?}");

        Ok(())
    }
    */

    use std::io::BufReader;

    #[test]
    fn test_arrow_gfa() -> std::io::Result<()> {
        use std::fs::File;

        let gfa_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../test/data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa"
        );

        let gfa_file = File::open(gfa_path)?;
        let reader = BufReader::new(gfa_file);

        let arrow_gfa = arrow_graph_from_gfa(reader)?;

        let nodes = arrow_gfa.segment_count();
        let links = arrow_gfa.link_count();
        let paths = arrow_gfa.path_count();

        let nodes_iter_count: usize =
            arrow_gfa.segment_sequences_iter().count();

        assert_eq!(4966, nodes);
        assert_eq!(nodes, nodes_iter_count);

        assert_eq!(6793, links);
        assert_eq!(11, paths);

        println!("node count: {nodes}");
        println!("node iter count: {nodes_iter_count}");
        println!("link count: {links}");
        println!("path count: {paths}");

        Ok(())
    }
}
