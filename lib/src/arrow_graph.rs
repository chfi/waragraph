use arrow2::{
    array::PrimitiveArray,
    datatypes::{DataType, Field, Schema},
    offset::OffsetsBuffer,
};

use crate::graph::{Bp, Edge, Node, OrientedNode, PathId};

// waragraph file
// pub struct WGF {
//     schema: Schema,
// }

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
