use ahash::{AHashMap, HashMap};
use arrow2::{
    array::{
        BinaryArray, UInt32Array, Utf8Array,
    },
    buffer::Buffer,
    datatypes::{DataType},
    offset::{OffsetsBuffer},
};
use smallvec::SmallVec;

use std::io::prelude::*;

// use crate::{Bp, Edge, Node, OrientedNode, PathId};

use super::ArrowGFA;

pub struct ChunkParser {
    segment_id_map: HashMap<SmallVec<[u8; 12]>, u32>,

    sequences: Vec<u8>,
    sequence_offsets: Vec<i32>,
    links_from: Vec<u32>,
    links_to: Vec<u32>,

    path_names: Vec<Vec<u8>>,
    path_steps: Vec<Vec<u32>>,
}

impl ChunkParser {
    pub fn finish(self) -> ArrowGFA {
        todo!();
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

        let Some(_opt_fields) = opt else {
            continue;
        };
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

        let Some((from_handle, to_handle, _overlap, opt)) =
            fields.next().and_then(|_type| {
                let from_name = fields.next()?;
                let from_is_rev = fields.next()? == b"-";
                let from_h = seg_step_to_handle(from_name, from_is_rev);

                let to_name = fields.next()?;
                let to_is_rev = fields.next()? == b"-";
                let to_h = seg_step_to_handle(to_name, to_is_rev);

                let overlap = fields.next()?;
                let opt = fields.next();
                Some((from_h, to_h, overlap, opt))
            })
        else {
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

    let mut path_step_offsets: Vec<i32> = Vec::new();
    let mut path_step_offset = 0;
    // each step as handle, per path
    let mut path_step_arrs: Vec<UInt32Array> = Vec::new();
    // let mut path_step_array: Vec<u32> = Vec::new();

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

        let Some((path_name, seg_names, _overlaps, _opt)) =
            fields.next().and_then(|_type| {
                let path_name = fields.next()?;
                let seg_names = fields.next()?;
                let overlaps = fields.next()?;
                let opt = fields.next();

                Some((path_name, seg_names, overlaps, opt))
            })
        else {
            continue;
        };

        let name_offset = path_name_str.len();
        path_name_offsets.push(name_offset as i32);
        path_name_str.extend(path_name);

        let mut step_vec = Vec::new();

        for (_step_index, step_str) in
            seg_names.split(|&c| c == b',').enumerate()
        {
            let (seg_name, seg_orient) = step_str.split_at(step_str.len() - 1);
            let is_rev = seg_orient == b"-";

            let seg_i = seg_step_to_handle(seg_name, is_rev);
            step_vec.push(seg_i);
            // path_step_array.push(seg_i);
        }

        path_step_offsets.push(path_step_offset);
        path_step_offset += step_vec.len() as i32;
        path_step_arrs.push(UInt32Array::from_vec(step_vec));
    }

    let name_offset = path_name_str.len();
    path_name_offsets.push(name_offset as i32);
    let name_offsets = OffsetsBuffer::try_from(path_name_offsets).unwrap();

    // path_step_offsets.push(path_step_offset);
    // let path_step_offsets = OffsetsBuffer::try_from(path_step_offsets).unwrap();

    // let path_step_list = ListArray::new(
    //     DataType::List(Box::new(Field::new("steps", DataType::UInt32, false))),
    //     path_step_offsets,
    //     UInt32Array::from_vec(path_step_array).boxed(),
    //     None,
    // );

    // let arr: MutablePrimitiveArray<u32> = MutablePrimitiveArray::default();
    // let mut steps_list = MutableListArray::new_with_field(arr, "steps", false);

    // for steps in path_step_arrs.iter() {
    //     let array = steps.clone().boxed();
    //     steps_list.try_push(array);
    // }

    // steps_list.try_push(

    // let path_step_list = ListArray::new(DataType::List(Box::new(Field::new("steps", DataType::UInt32, false))),
    //                                     path_step_offsets,

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
        // path_step_list,
    })
}
