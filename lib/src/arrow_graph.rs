use arrow2::{
    array::{BinaryArray, PrimitiveArray, StructArray, Utf8Array},
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

    links: StructArray,

    path_names: Utf8Array<i32>,
}

pub fn arrow_graph_from_gfa<S: BufRead + Seek>(
    mut gfa_lines_reader: S,
    // gfa_lines: impl Iterator<Item = &'a str>,
) -> std::io::Result<ArrowGFA> {
    let mut line_buf = Vec::new();

    // let mut seg_sequences: Vec<Vec<u8>> = Vec::new();
    // let mut seg_names: Vec<String> = Vec::new();

    // let mut seg_seq_offsets: OffsetsBuffer<i64> = OffsetsBuffer::new();
    let mut seg_seq_offsets: Vec<i32> = Vec::new();
    let mut seg_seq_bytes: Vec<u8> = Vec::new();

    let mut seg_name_offsets: Vec<i32> = Vec::new();
    let mut seg_name_str: Vec<u8> = Vec::new();

    gfa_lines_reader.rewind()?;

    loop {
        line_buf.clear();

        let len = gfa_lines_reader.read_until(0xA, &mut line_buf)?;
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

        let offset = seg_seq_bytes.len();
        // the first offset is always 0 and implicit
        if offset != 0 {
            seg_seq_offsets.push(offset as i32);
        }
        seg_seq_bytes.extend(seq);

        let name_offset = seg_name_str.len();
        if name_offset != 0 {
            seg_name_offsets.push(name_offset as i32);
        }
        seg_name_str.extend(name);

        let Some(opt_fields) = opt
        else { continue; };
    }

    todo!();
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

    use anyhow::Result;

    use super::*;

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
}
