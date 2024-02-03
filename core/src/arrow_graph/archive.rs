use arrow2::{
    array::{BinaryArray, Int32Array, StructArray, UInt32Array, Utf8Array},
    chunk::Chunk,
    datatypes::{DataType, Field, Metadata, Schema},
    io::ipc::{
        read::{FileMetadata, FileReader},
        write::{FileWriter, WriteOptions},
    },
    offset::Offsets,
};
use tar::Archive;

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor, SeekFrom},
    sync::Arc,
};
use std::{io::prelude::*, path::PathBuf};

use super::*;

fn find_field_by_name<'a>(
    metadata: &'a FileMetadata,
    name: &str,
) -> std::io::Result<(usize, &'a Field)> {
    metadata
        .schema
        .fields
        .iter()
        .enumerate()
        .find(|(_, f)| f.name == name)
        .ok_or_else(|| std::io::Error::other(format!("Missing field {name}")))
}

fn deserialize_segments<R: Read + Seek>(
    reader: FileReader<R>,
) -> Result<(BinaryArray<i32>, Utf8Array<i32>), arrow2::error::Error> {
    let metadata = reader.metadata().clone();

    let seg_sequences_field =
        find_field_by_name(&metadata, "segment_sequences")?;
    let seg_names_field = find_field_by_name(&metadata, "segment_names")?;

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

    let sequences = sequences.as_any().downcast_ref::<BinaryArray<i32>>();
    let names = names.as_any().downcast_ref::<Utf8Array<i32>>();

    Ok((sequences.unwrap().clone(), names.unwrap().clone()))
}

fn deserialize_links<R: Read + Seek>(
    reader: FileReader<R>,
) -> Result<(UInt32Array, UInt32Array), arrow2::error::Error> {
    let metadata = reader.metadata().clone();
    let (from_ix, _from) = find_field_by_name(&metadata, "from")?;
    let (to_ix, _to) = find_field_by_name(&metadata, "to")?;

    let mut batches = Vec::new();
    for chunk in reader {
        batches.push(chunk?);
    }

    if batches.len() > 1 {
        eprintln!("Ignoring batches after the first");
    }

    let arrays = &batches[0].arrays();

    let from = arrays[from_ix]
        .as_any()
        .downcast_ref::<UInt32Array>()
        .cloned();
    let to = arrays[to_ix]
        .as_any()
        .downcast_ref::<UInt32Array>()
        .cloned();

    Ok((from.unwrap(), to.unwrap()))
}

fn deserialize_path_names<R: Read + Seek>(
    reader: FileReader<R>,
) -> Result<Utf8Array<i32>, arrow2::error::Error> {
    let mut batches = Vec::new();
    for chunk in reader {
        batches.push(chunk?);
    }

    if batches.len() > 1 {
        eprintln!("Ignoring batches after the first");
    }

    let arrays = &batches[0].arrays();

    let names = arrays[0].as_any().downcast_ref::<Utf8Array<i32>>().cloned();

    Ok(names.unwrap())
}

fn deserialize_path<R: Read + Seek>(
    reader: FileReader<R>,
) -> Result<UInt32Array, arrow2::error::Error> {
    let mut batches = Vec::new();
    for chunk in reader {
        batches.push(chunk?);
    }

    if batches.len() > 1 {
        eprintln!("Ignoring batches after the first");
    }

    let arrays = &batches[0].arrays();

    let steps = arrays[0].as_any().downcast_ref::<UInt32Array>().cloned();

    Ok(steps.unwrap())
}

impl ArrowGFA {
    // NB: if the Arc<memmap2::Mmap> is dropped, the `ArrowGFA` is invalidated...
    pub unsafe fn mmap_archive(
        path: impl AsRef<std::path::Path>,
    ) -> Result<(Self, Arc<memmap2::Mmap>), arrow2::error::Error> {
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

            if let Ok(ix_str) = path.strip_prefix("path/") {
                let ix_str = ix_str.file_name().and_then(|s| s.to_str());
                if let Some(ix) = ix_str.and_then(|s| s.parse::<u32>().ok()) {
                    path_arrays_index.insert(ix, offset..end);
                } else {
                    eprintln!("Error parsing path index from `{path:?}`");
                }
            } else {
                field_index.insert(path.to_path_buf(), offset..end);
            }
        }

        let path_arrays_index = {
            let mut index = path_arrays_index.into_iter().collect::<Vec<_>>();
            index.sort_by_key(|(i, _)| *i);
            index
        };

        let file = File::open(&path)?;

        let mmap = std::sync::Arc::new(unsafe { memmap2::Mmap::map(&file)? });

        let memory_map_chunk =
            |range: &Range<usize>| -> Result<_, arrow2::error::Error> {
                let file_slice = &mmap[range.clone()];
                let mut cursor = Cursor::new(file_slice);
                let data = Arc::new(&mmap[range.clone()]);
                let metadata =
                    arrow2::io::ipc::read::read_file_metadata(&mut cursor)?;

                let dicts = unsafe {
                    arrow2::mmap::mmap_dictionaries_unchecked(
                        &metadata,
                        data.clone(),
                    )?
                };

                let chunk = unsafe {
                    arrow2::mmap::mmap_unchecked(&metadata, &dicts, data, 0)?
                };

                Ok((metadata, chunk))
            };

        // segments
        let segments_range = field_index
            .get("segments".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other("`segments` not found in archive"))?;

        let (_seg_meta, seg_chunk) = memory_map_chunk(segments_range)?;

        let arrays = &seg_chunk.arrays();

        let segment_sequences =
            arrays[0].as_any().downcast_ref::<BinaryArray<i32>>();

        let segment_names = arrays[1].as_any().downcast_ref::<Utf8Array<i32>>();

        // links

        let links_range = field_index
            .get("links".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other("`links` not found in archive"))?;

        let (_link_meta, link_chunk) = memory_map_chunk(links_range)?;
        let arrays = &link_chunk.arrays();
        let link_from = arrays[0].as_any().downcast_ref::<UInt32Array>();
        let link_to = arrays[1].as_any().downcast_ref::<UInt32Array>();

        // path names

        let path_names_range = field_index
            .get("path_names".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other(
                "`path_names` not found in archive",
            ))?;

        let (_names_meta, names_chunk) = memory_map_chunk(path_names_range)?;
        let arrays = &names_chunk.arrays();
        let path_names = arrays[0].as_any().downcast_ref::<Utf8Array<i32>>();

        // path steps
        let mut path_steps = Vec::new();

        for (_path_ix, range) in path_arrays_index {
            let (_meta, steps_chunk) = memory_map_chunk(&range)?;

            let arrays = &steps_chunk.arrays();
            let steps = arrays[0].as_any().downcast_ref::<UInt32Array>();

            path_steps.push(steps.cloned().unwrap());
        }

        Ok((
            ArrowGFA {
                segment_sequences: segment_sequences.cloned().unwrap(),
                segment_names: segment_names.cloned(),
                link_from: link_from.cloned().unwrap(),
                link_to: link_to.cloned().unwrap(),
                path_names: path_names.cloned().unwrap(),
                path_steps,
            },
            mmap,
        ))
    }

    pub fn read_archive(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, arrow2::error::Error> {
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

            if let Ok(ix_str) = path.strip_prefix("path/") {
                let ix_str = ix_str.file_name().and_then(|s| s.to_str());
                if let Some(ix) = ix_str.and_then(|s| s.parse::<u32>().ok()) {
                    path_arrays_index.insert(ix, offset..end);
                } else {
                    eprintln!("Error parsing path index from `{path:?}`");
                }
            } else {
                field_index.insert(path.to_path_buf(), offset..end);
            }
        }

        let path_arrays_index = {
            let mut index = path_arrays_index.into_iter().collect::<Vec<_>>();
            index.sort_by_key(|(i, _)| *i);
            index
        };

        let file = File::open(&path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        let create_reader_for_range =
            |range: Range<usize>| -> Result<_, arrow2::error::Error> {
                let file_slice = &mmap[range];
                let mut cursor = Cursor::new(file_slice);
                let metadata =
                    arrow2::io::ipc::read::read_file_metadata(&mut cursor)?;
                Ok(FileReader::new(cursor, metadata.clone(), None, None))
            };

        // segments
        let segments_range = field_index
            .get("segments".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other("`segments` not found in archive"))?;
        let segments_reader = create_reader_for_range(segments_range.clone())?;

        let (segment_sequences, segment_names) =
            deserialize_segments(segments_reader)
                .map_err(|e| std::io::Error::other(e))?;

        // links
        let links_range = field_index
            .get("links".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other("`links` not found in archive"))?;
        let links_reader = create_reader_for_range(links_range.clone())?;

        let (link_from, link_to) = deserialize_links(links_reader)?;

        // path names
        let path_names_range = field_index
            .get("path_names".as_ref() as &std::path::Path)
            .ok_or(std::io::Error::other(
                "`path_names` not found in archive",
            ))?;

        let names_reader = create_reader_for_range(path_names_range.clone())?;
        let path_names = deserialize_path_names(names_reader)?;

        let mut path_steps = Vec::new();
        // path steps
        for (_path_ix, range) in path_arrays_index {
            let steps_reader = create_reader_for_range(range)?;
            let steps = deserialize_path(steps_reader)?;
            path_steps.push(steps);
        }

        Ok(ArrowGFA {
            segment_sequences,
            segment_names: Some(segment_names),
            link_from,
            link_to,
            path_names,
            path_steps,
        })
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

                buf.clear();

                Ok(())
            };

            for (path_ix, steps) in self.path_steps.iter().enumerate() {
                write_steps(path_ix as u32, steps)?;
            }
        }

        archive_builder.finish()
    }
}
