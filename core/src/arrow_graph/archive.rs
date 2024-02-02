use arrow2::{
    array::{BinaryArray, Int32Array, StructArray, UInt32Array, Utf8Array},
    chunk::Chunk,
    datatypes::{DataType, Field, Metadata, Schema},
    io::ipc::{
        read::FileReader,
        write::{FileWriter, WriteOptions},
    },
    offset::Offsets,
};
use tar::Archive;

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor, SeekFrom},
};
use std::{io::prelude::*, path::PathBuf};

use super::*;

fn deserialize_segments<R: Read + Seek>(
    mut reader: FileReader<R>,
) -> Result<(BinaryArray<i32>, Utf8Array<i32>), arrow2::error::Error> {
    let metadata = reader.metadata().clone();
    let get_field = |name: &str| {
        metadata
            .schema
            .fields
            .iter()
            .enumerate()
            .find(|(_, f)| f.name == name)
            .ok_or_else(|| {
                std::io::Error::other(format!("Missing field {name}"))
            })
    };

    let seg_sequences_field = get_field("segment_sequences")?;
    let seg_names_field = get_field("segment_names")?;

    let segment_sequences: BinaryArray<i32>;
    let segment_names: Utf8Array<i32>;

    // NB: right now there's only ever one chunk per message
    let mut batches = Vec::new();
    for chunk in reader {
        batches.push(chunk?);
    }

    if batches.len() > 1 {
        eprintln!("Ignoring batches after the first");
    }

    let arrays = &batches[0].arrays();

    let sequences = &arrays[seg_sequences_field.0];
    let names = &arrays[seg_names_field.0];

    println!("sequences datatype: {:?}", sequences.data_type());
    println!("names datatype: {:?}", names.data_type());

    let sequences = sequences.as_any().downcast_ref::<BinaryArray<i32>>();
    let names = names.as_any().downcast_ref::<Utf8Array<i32>>();

    // println!("{sequences:?}");
    // println!("{names:?}");

    /*
    for (i, c) in reader.enumerate() {
        println!("chunk {i}");
        if let Ok(c) = c {
            println!("    length {}", c.len());
            let count = c.arrays().len();
            println!("    arrays count {}", count);
            segment_chunks.push(c);
        }
    }

    println!("{} chunks", segment_chunks.len());
    */

    Ok((sequences.unwrap().clone(), names.unwrap().clone()))
}

impl ArrowGFA {
    pub fn read_archive(
        path: impl AsRef<std::path::Path>,
    ) -> std::io::Result<Self> {
        use std::ops::Range;

        let file = File::open(&path)?;
        let mut archive = tar::Archive::new(file);

        let entries = archive.entries_with_seek()?;

        let mut field_index: HashMap<PathBuf, Range<usize>> =
            HashMap::default();
        let mut path_arrays_index: HashMap<u32, Range<usize>> =
            HashMap::default();

        for entry in entries {
            let entry = entry?;
            let offset = entry.raw_file_position() as usize;
            let end = offset + entry.size() as usize;

            let path = entry.path()?;

            println!("{path:?}");

            if let Ok(ix_str) = path.strip_prefix("path/") {
                let ix_str = ix_str.file_name().and_then(|s| s.to_str());
                if let Some(ix) = ix_str.and_then(|s| s.parse::<u32>().ok()) {
                    println!("Path {ix} -- {}..{}", offset, end);
                    path_arrays_index.insert(ix, offset..end);
                } else {
                    eprintln!("Error parsing path index from `{path:?}`");
                }
            } else {
                println!("Field {path:?} -- {}..{}", offset, end);
                field_index.insert(path.to_path_buf(), offset..end);
            }
        }

        let path_arrays_index = {
            let mut index = path_arrays_index
                .into_iter()
                .enumerate()
                .collect::<Vec<_>>();
            index.sort_by_key(|(i, _)| *i);
            index.into_iter().map(|(_, v)| v).collect::<Vec<_>>()
        };

        let mut file = File::open(&path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        // segments
        let segments_range = field_index
            .get("segments".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other("`segments` not found in archive"))?;

        let segments_slice = &mmap[segments_range.clone()];
        let mut segments_cursor = Cursor::new(segments_slice);
        let metadata =
            arrow2::io::ipc::read::read_file_metadata(&mut segments_cursor)
                .map_err(|e| std::io::Error::other(e))?;

        println!("{metadata:#?}");

        let arrow_reader =
            FileReader::new(segments_cursor, metadata.clone(), None, None);

        let (segment_sequences, segment_names) =
            deserialize_segments(arrow_reader)
                .map_err(|e| std::io::Error::other(e))?;

        println!("sequence count: {}", segment_sequences.len());

        /*

        // links
        let links_range = field_index
            .get("links".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other("`links` not found in archive"))?;

        // path names
        let links_range = field_index
            .get("path_names".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other(
                "`path_names` not found in archive",
            ))?;

        // path steps
        */

        todo!();
    }

    pub fn write_archive(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> std::io::Result<()> {
        // create file
        let file = File::create(path)?;
        let mut archive_builder = tar::Builder::new(file);

        // buffer to hold the serialized representation;
        // can probably be avoided
        let mut buf = Vec::<u8>::new();

        // segments
        {
            // serialize segment names and sequences
            let schema = ArrowGFA::segment_schema();

            let mut writer = FileWriter::new(
                &mut buf,
                schema,
                None,
                WriteOptions { compression: None },
            );

            let chunk = Chunk::new(vec![
                self.segment_sequences.clone().boxed(),
                self.segment_names.clone().unwrap().boxed(),
            ]);

            writer.start().unwrap();
            writer.write(&chunk, None).unwrap();
            writer.finish().unwrap();

            let mut header = tar::Header::new_old();
            header.set_size(buf.len() as u64);
            header.set_cksum();

            archive_builder.append_data(
                &mut header,
                "segments",
                buf.as_slice(),
            )?;
        }

        buf.clear();

        // links
        {
            let schema = Schema {
                fields: vec![
                    Field::new("from", DataType::UInt32, false),
                    Field::new("to", DataType::UInt32, false),
                ],
                metadata: Metadata::default(),
            };

            let mut writer = FileWriter::new(
                &mut buf,
                schema,
                None,
                WriteOptions { compression: None },
            );

            let chunk = Chunk::new(vec![
                self.link_from.clone().boxed(),
                self.link_to.clone().boxed(),
            ]);

            writer.start().unwrap();
            writer.write(&chunk, None).unwrap();
            writer.finish().unwrap();

            let mut header = tar::Header::new_old();
            header.set_size(buf.len() as u64);
            header.set_cksum();

            archive_builder.append_data(
                &mut header,
                "links",
                buf.as_slice(),
            )?;
        }

        buf.clear();

        // path names
        {
            let schema = Schema {
                fields: vec![Field::new("path_name", DataType::Utf8, false)],
                metadata: Metadata::default(),
            };

            let mut writer = FileWriter::new(
                &mut buf,
                schema,
                None,
                WriteOptions { compression: None },
            );

            let chunk = Chunk::new(vec![self.path_names.clone().boxed()]);

            writer.start().unwrap();
            writer.write(&chunk, None).unwrap();
            writer.finish().unwrap();

            let mut header = tar::Header::new_old();
            header.set_size(buf.len() as u64);
            header.set_cksum();

            archive_builder.append_data(
                &mut header,
                "path_names",
                buf.as_slice(),
            )?;
        }

        buf.clear();

        // path steps
        {
            let mut write_steps = |path_ix: u32,
                                   steps: &UInt32Array|
             -> std::io::Result<()> {
                let schema = Schema {
                    fields: vec![Field::new("steps", DataType::UInt32, false)],
                    metadata: Metadata::default(),
                };
                let mut writer = FileWriter::new(
                    &mut buf,
                    schema,
                    None,
                    WriteOptions { compression: None },
                );

                let chunk = Chunk::new(vec![steps.clone().boxed()]);

                writer.start().unwrap();
                writer.write(&chunk, None).unwrap();
                writer.finish().unwrap();

                let mut header = tar::Header::new_old();
                header.set_size(buf.len() as u64);
                header.set_cksum();

                archive_builder.append_data(
                    &mut header,
                    format!("path/{path_ix}"),
                    buf.as_slice(),
                )?;

                Ok(())
            };

            for (path_ix, steps) in self.path_steps.iter().enumerate() {
                write_steps(path_ix as u32, steps)?;
            }
        }

        archive_builder.finish()
    }
}
