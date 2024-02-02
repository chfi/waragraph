use arrow2::{
    array::{BinaryArray, Int32Array, StructArray, UInt32Array, Utf8Array},
    chunk::Chunk,
    datatypes::{DataType, Field, Metadata, Schema},
    io::ipc::write::{FileWriter, WriteOptions},
    offset::Offsets,
};
use tar::Archive;

use std::fs::File;
use std::io::prelude::*;

use super::*;

impl ArrowGFA {
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
