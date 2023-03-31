use anyhow::{anyhow, Result};
use std::collections::HashMap;

use waragraph_core::graph::{Bp, PathId, PathIndex};

pub struct AnnotationSet {
    annotations: Vec<(std::ops::Range<Bp>, String)>,
    path_annotations: HashMap<PathId, Vec<usize>>,
}

impl AnnotationSet {
    pub fn from_gff(
        graph: &PathIndex,
        path_name_map: impl Fn(&str) -> String,
        record_label: impl Fn(&noodles::gff::Record) -> Option<String>,
        gff_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        use noodles::gff;
        use std::fs::File;
        use std::io::BufReader;

        let mut reader = File::open(gff_path)
            .map(BufReader::new)
            .map(gff::Reader::new)?;

        let mut annotations = Vec::new();
        let mut path_annotations: HashMap<_, Vec<_>> = HashMap::new();

        for result in reader.records() {
            let record = result?;

            if let Some(label) = record_label(&record) {
                let seqid = &record.reference_sequence_name();

                let start = record.start().get();
                let end = record.end().get();

                let start_bp = Bp(start as u64 - 1);
                let end_bp = Bp(end as u64);
                let range = start_bp..end_bp;

                let path_name = path_name_map(seqid);
                let path_id = *graph
                    .path_names
                    .get_by_right(&path_name)
                    .ok_or_else(|| anyhow!("Path not found: {path_name}"))?;

                let a_id = annotations.len();
                annotations.push((range, label));
                path_annotations.entry(path_id).or_default().push(a_id);
            }
        }

        Ok(Self {
            annotations,
            path_annotations,
        })
    }
}

pub struct AnnotationStore {
    pub(crate) annotation_sets: HashMap<String, AnnotationSet>,
}
