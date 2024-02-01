use arrow2::{
    array::{BinaryArray, StructArray, UInt32Array, Utf8Array},
    chunk::Chunk,
    datatypes::{DataType, Field, Metadata, Schema},
    offset::Offsets,
};
use smallvec::SmallVec;

// use crate::types::{Bp, Edge, Node, OrientedNode, PathId};

pub mod parser;

pub use parser::arrow_graph_from_gfa;

#[derive(Debug, Clone)]
pub struct ArrowGFA {
    // using 32-bit IDs & indices, even for sequence, for now; since
    // wasm is limited to 32 bits for the foreseeable future (and
    // single memories), it's probably better to implement a kind of
    // paging/chunking system so that we can load in only the relevant
    // parts of the graph into the wasm linear memory
    //
    // each page would only need to hold 64 bit offsets at the most,
    // with the array data being 0-offset, so this also provides some
    // compression (especially for e.g. paths)
    pub segment_sequences: BinaryArray<i32>,
    pub segment_names: Option<Utf8Array<i32>>,
    // pub segment_id_offset: Option<i32>, // TODO handle this maybe
    pub link_from: UInt32Array,
    pub link_to: UInt32Array,

    pub path_names: Utf8Array<i32>,
    // TODO: path_steps should be a ListArray
    path_steps: Vec<UInt32Array>,
    // TODO: finish this!!
    // path_step_list: ListArray<i32>,
}

#[derive(Debug, Clone)]
pub struct PathIndex {
    pub segment_path_matrix: SegmentPathMatrix,
}

// pub struct PathsOnSegmentIter<'a> {
//     bitmap_iter:
// }

impl PathIndex {
    pub fn from_arrow_gfa(gfa: &ArrowGFA) -> Self {
        let segment_path_matrix = SegmentPathMatrix::from_arrow_gfa(gfa);
        Self {
            segment_path_matrix,
        }
    }

    pub fn paths_on_segment<'a>(
        &'a self,
        segment: u32,
    ) -> Option<impl Iterator<Item = u32> + 'a> {
        // TODO
        let bitmap = self.segment_path_matrix.paths_on_segment(segment)?;

        // bitmap.iter().flat_map(|(ix, &bits)| {
        //     // bits.
        //     // bits.
        //     todo!();
        // })

        None
        /*
        if let Some(bitmap) =
            self.0.segment_path_matrix.paths_on_segment(segment)
        {
            let paths = Vec::new();

            for (ix, &bits) in bitmap.iter() {
                log::warn!("{ix} - {:b}", bits);
            }

            paths
        } else {
            log::error!("segment out of bounds");

            Vec::new()
        }
        */
    }
}

#[derive(Debug, Clone)]
pub struct SegmentPathMatrix {
    storage: sprs::CompressedStorage,
    shape: (usize, usize),
    indptr_array: UInt32Array,
    ind_array: UInt32Array,
    data_array: UInt32Array,
}

impl SegmentPathMatrix {
    pub fn from_arrow_gfa(gfa: &ArrowGFA) -> Self {
        // use ahash::AHashMap;
        // let mut segment_paths: Vec<AHashMap<u32, u32>> =
        //     vec![AHashMap::default(); gfa.segment_count()];

        let mut segment_paths: Vec<SmallVec<[(u32, u32); 4]>> =
            vec![SmallVec::default(); gfa.segment_count()];

        let insert_segment = |map: &mut SmallVec<[(u32, u32); 4]>,
                              path_id: u32| {
            let bits_ix = path_id / 32;
            let modulo = path_id % 32;
            let val = 1 << modulo;

            let result = map.binary_search_by_key(&bits_ix, |(k, _v)| *k);

            match result {
                Ok(ix) => map[ix].1 |= val,
                Err(ix) => map.insert(ix, (bits_ix, val)),
            }
        };

        for (path_i, steps) in gfa.path_steps.iter().enumerate() {
            let path_id = path_i as u32;
            // let bitset_index = path_id / 32;

            for step_handle in steps.values_iter() {
                let segment = step_handle >> 1;
                let bitmap = &mut segment_paths[segment as usize];
                insert_segment(bitmap, path_id);
            }
        }

        let rows = (gfa.path_steps.len() as f32 / 32.0).ceil() as usize;
        let cols = gfa.segment_count();

        let mut tri: sprs::TriMatI<u32, u32> = sprs::TriMatI::new((rows, cols));

        let mut bitmap_vec = Vec::new();

        for (segment, bitmap) in segment_paths.into_iter().enumerate() {
            bitmap_vec.clear();
            bitmap_vec.extend(bitmap.into_iter());
            bitmap_vec.sort();
            let col = segment;
            for &(bitset_index, value) in bitmap_vec.iter() {
                let row = bitset_index as usize;
                tri.add_triplet(row, col, value);
            }
        }
        let _t2 = instant::now();
        // log::warn!("took {} ms to construct sparse matrix", t2 - t1);

        let mat = tri.to_csc::<u32>();

        let storage = mat.storage();
        let shape = mat.shape();

        let (indptr, ind, data) = mat.into_raw_storage();

        let indptr_array = UInt32Array::from_vec(indptr);
        let ind_array = UInt32Array::from_vec(ind);
        let data_array = UInt32Array::from_vec(data);

        let result = SegmentPathMatrix {
            storage,
            shape,
            indptr_array,
            ind_array,
            data_array,
        };

        result
    }

    pub fn matrix(&self) -> sprs::CsMatViewI<'_, u32, u32, u32> {
        // safe since we know it's a correct sparse matrix
        unsafe {
            let indptr = self.indptr_array.values();
            let ind = self.ind_array.values();
            let data = self.data_array.values();
            sprs::CsMatViewI::new_unchecked(
                self.storage,
                self.shape,
                indptr,
                ind,
                data,
            )
        }
    }

    pub fn paths_on_segment(
        &self,
        segment: u32,
    ) -> Option<sprs::CsVecI<u32, u32>> {
        let matrix = self.matrix();

        if segment as usize >= matrix.cols() {
            return None;
        }

        let mut rhs: sprs::CsVecI<u32, u32> =
            sprs::CsVecI::empty(matrix.cols());
        rhs.append(segment as usize, 1);

        // log::warn!("rhs: {:?}", rhs);

        let result = &matrix * &rhs;

        // log::warn!("result: {result:?}");

        Some(result)
    }
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

    pub fn path_steps(&self, path_id: u32) -> &UInt32Array {
        &self.path_steps[path_id as usize]
    }

    pub fn segment_sequence(&self, segment_index: u32) -> &[u8] {
        self.segment_sequences.get(segment_index as usize).unwrap()
    }

    pub fn segment_len(&self, segment_index: u32) -> usize {
        self.segment_sequence(segment_index).len()
    }

    pub fn total_sequence_len(&self) -> usize {
        *self.segment_sequences.offsets().last() as usize
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
            .find(|&(_i, name)| name == segment_name)?;
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
    pub fn path_name_id(&self, path_name: &str) -> Option<u32> {
        let (path_ix, _) = self
            .path_names
            .iter()
            .enumerate()
            .find(|(_ix, name)| name.is_some_and(|n| n == path_name))?;
        Some(path_ix as u32)
    }

    pub fn path_name(&self, path_id: u32) -> Option<&str> {
        self.path_names.get(path_id as usize)
    }

    pub fn path_step_len(&self, path_id: u32) -> usize {
        self.path_steps[path_id as usize].len()
    }

    pub fn path_steps_iter<'a>(
        &'a self,
        path_id: u32,
    ) -> impl Iterator<Item = u32> + 'a {
        let steps = &self.path_steps[path_id as usize];
        steps.values_iter().copied()
    }

    pub fn path_slice(
        &self,
        path_id: u32,
        start_step: usize,
        length: usize,
    ) -> UInt32Array {
        let steps = &self.path_steps[path_id as usize];
        let slice = steps.clone().sliced(start_step, length);
        slice
    }

    /// Returns a CSC matrix that maps handles to links.
    pub fn handle_link_adj_mat(&self) -> sprs::CsMatI<u8, u32> {
        // rows
        let link_count = self.link_from.len();

        // columns
        let handle_count = self.segment_count() * 2;

        // |E| x |V|
        let mut adj: sprs::TriMatI<u8, u32> =
            sprs::TriMatI::new((link_count, handle_count));

        let from = self.link_from.values_iter();
        let to = self.link_to.values_iter();

        for (link_i, (&from, &to)) in from.zip(to).enumerate() {
            adj.add_triplet(link_i, from as usize, 1);
            adj.add_triplet(link_i, to as usize, 1);
        }

        adj.to_csc()
    }

    // pub fn path_vector_offsets(
    //     &self,
    //     path_id: u32,
    // ) -> sprs::CsVecI<u32, u32> {
    //     let dim = self.segment_sequences.len();
    //     //
    // }

    pub fn path_vector_sparse_u32(
        &self,
        path_id: u32,
    ) -> sprs::CsVecI<u32, u32> {
        let dim = self.segment_sequences.len();

        let mut data = vec![0u32; dim];

        let steps = &self.path_steps[path_id as usize];

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

    pub fn path_step_offsets(
        &self,
        path_id: u32,
    ) -> arrow2::offset::Offsets<i32> {
        let steps = &self.path_steps[path_id as usize];
        arrow2::offset::Offsets::try_from_lengths(steps.values_iter().map(
            |step_handle| {
                let i = step_handle >> 1;
                self.segment_len(i)
            },
        ))
        .unwrap()
    }
}

impl ArrowGFA {
    pub fn graph_depth_vector(&self) -> Vec<u32> {
        let mut seg_counts = vec![0u32; self.segment_count()];

        for steps in &self.path_steps {
            for step in steps.values_iter() {
                let seg = step >> 1;
                seg_counts[seg as usize] += 1;
            }
        }

        seg_counts
    }
}

impl ArrowGFA {
    // need one schema for segments, one for links, etc., as each will need
    // its own set of record batches

    fn segment_schema() -> Schema {
        let mut fields = vec![];
        fields.push(Field::new("segment_sequences", DataType::Binary, false));
        fields.push(Field::new("segment_names", DataType::Utf8, true));
        Schema {
            fields,
            metadata: Metadata::default(),
        }
    }

    fn links_schema() -> Schema {
        let links = DataType::Struct(vec![
            Field::new("from", DataType::UInt32, false),
            Field::new("to", DataType::UInt32, false),
        ]);
        Schema {
            fields: vec![Field::new("links", links, false)],
            metadata: Metadata::default(),
        }
    }

    fn paths_schema() -> Schema {
        let mut fields = vec![];
        fields.push(Field::new("path_names", DataType::Utf8, true));
        fields.push(Field::new(
            "path_steps",
            DataType::List(Box::new(Field::new(
                "steps",
                DataType::UInt32,
                false,
            ))),
            true,
        ));
        Schema {
            fields,
            metadata: Metadata::default(),
        }
    }

    pub fn write_arrow_ipc<W: std::io::Write>(
        &self,
        mut writer: W,
    ) -> arrow2::error::Result<()> {
        macro_rules! create_writer {
            ($schema:expr) => {
                arrow2::io::ipc::write::FileWriter::try_new(
                    &mut writer,
                    $schema,
                    None,
                    arrow2::io::ipc::write::WriteOptions { compression: None },
                )?
                // .map_err(|e| {
                //     std::io::Error::new(
                //         std::io::ErrorKind::Other,
                //         format!("Error creating Arrow IPC writer: {e:?}"),
                //     )
                // })?
            };
        }
        // let create_writer = |writer, schema| {
        //     arrow2::io::ipc::write::FileWriter::try_new(
        //         writer,
        //         schema,
        //         None,
        //         arrow2::io::ipc::write::WriteOptions { compression: None },
        //     )
        //     .map_err(|e| {
        //         std::io::Error::new(
        //             std::io::ErrorKind::Other,
        //             format!("Error creating Arrow IPC writer: {e:?}"),
        //         )
        //     })
        // };

        {
            let schema = Self::segment_schema();
            let mut msg_writer = create_writer!(schema);

            let seqs = self.segment_sequences.clone().boxed();
            if let Some(names) = self.segment_names.clone() {
                // this might not even be ecorrect
                msg_writer
                    .write(&Chunk::new(vec![seqs, names.boxed()]), None)?;
            } else {
                msg_writer.write(&Chunk::new(vec![seqs]), None)?;
            }
        }

        {
            let schema = Self::links_schema();

            let from = self.link_from.clone().boxed();
            let to = self.link_to.clone().boxed();

            let links = StructArray::new(
                DataType::Struct(schema.fields.clone()),
                vec![from, to],
                None,
            );

            let mut msg_writer = create_writer!(schema);

            msg_writer.write(&Chunk::new(vec![links.boxed()]), None)?;
        }

        {
            let _schema = Self::paths_schema();

            let _names = self.path_names.clone().boxed();
            // let steps = ListArray (use arrow2::array::List
            let _steps_list_offsets: Offsets<i32> = Offsets::try_from_lengths(
                self.path_steps.iter().map(|s| s.len()),
            )
            .unwrap();
            todo!();
            // let steps = ListArray::new(
            //     DataType::List(Box::new(Field::new("steps", DataType::UInt32, false))),
            //     OffsetsBuffer::from(steps_list_offsets),
        }

        /*
        let mut writer = arrow2::io::ipc::write::FileWriter::try_new(
            writer,
            schema.clone(),
            None,
            arrow2::io::ipc::write::WriteOptions { compression: None },
        )
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Error creating Arrow IPC writer: {e:?}"),
            )
        });
        */

        //
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

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

        let seq_lens = arrow_gfa
            .segment_sequences_iter()
            .map(|seq| seq.len())
            .collect::<Vec<_>>();

        let total_seq_len_iter: usize = seq_lens.iter().sum();

        let total_seq_len = arrow_gfa.total_sequence_len();

        let seq_offsets = arrow_gfa.segment_sequences.offsets();

        println!("total seq len (iter): {total_seq_len_iter}");
        println!("total seq len:        {total_seq_len}");

        println!("node count: {nodes}");
        println!("node iter count: {nodes_iter_count}");
        println!("link count: {links}");
        println!("path count: {paths}");

        assert_eq!(45897, total_seq_len_iter);
        assert_eq!(total_seq_len_iter, total_seq_len);

        assert_eq!(4966, nodes);
        assert_eq!(nodes, nodes_iter_count);

        assert_eq!(6793, links);
        assert_eq!(11, paths);

        Ok(())
    }
}
