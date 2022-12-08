use anyhow::Result;
use std::collections::HashMap;
use ultraviolet::Vec2;

use crate::{GfaLayout, PathIndex};

#[derive(Default, Clone)]
pub struct AnnotationStore {
    // path name -> list of (range, text) pairs
    pub path_annotations:
        HashMap<String, Vec<(std::ops::Range<usize>, String)>>,
}

impl AnnotationStore {
    pub fn layout_positions(
        &self,
        path_index: &PathIndex,
        layout: &GfaLayout,
    ) -> Vec<(Vec2, String)> {
        let mut out = Vec::new();

        let world_pos_for_offset = |path: &str, pos: usize| {
            path_index
                .step_at_pos(path, pos)
                .and_then(|s| layout.pos_for_node(s.node as usize))
        };

        for (path, annots) in self.path_annotations.iter() {
            for (range, text) in annots.iter() {
                let (s0, s1) = world_pos_for_offset(path, range.start).unwrap();
                let (e0, e1) = world_pos_for_offset(path, range.end).unwrap();

                let start = s0 + (s1 - s0) / 2.0;
                let end = e0 + (e1 - e0) / 2.0;
                let mid = start + (end - start) * 0.5;

                out.push((mid, text.to_string()));
            }
        }

        out
    }

    pub fn fill_from_bed(
        &mut self,
        bed_path: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        let mut reader = std::fs::File::open(bed_path)
            .map(std::io::BufReader::new)
            .map(noodles::bed::Reader::new)?;

        let records = reader.records::<4>();

        for record in records {
            let record = record?;
            let path = record.reference_sequence_name();
            let start = record.start_position().get();
            let end = record.end_position().get();

            if let Some(name) = record.name() {
                self.path_annotations
                    .entry(path.to_string())
                    .or_default()
                    .push((start..end, name.to_string()))
            }
        }

        Ok(())
    }
}
