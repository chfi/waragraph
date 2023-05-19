use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use waragraph_core::graph::{Bp, PathId, PathIndex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub path: PathId,
    pub range: std::ops::Range<Bp>,
    pub label: Arc<String>,
}

pub struct AnnotationSet {
    pub name: String,
    pub annotations: Vec<Annotation>,
    pub path_annotations: HashMap<PathId, Vec<usize>>,
}

fn annotation_set_name(
    file_path: impl AsRef<std::path::Path>,
    name: Option<&str>,
) -> String {
    if let Some(name) = name {
        name.to_string()
    } else {
        file_path
            .as_ref()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| "<ERROR>")
            .to_string()
    }
}

impl AnnotationSet {
    pub fn get(&self, annot_id: AnnotationId) -> Option<&Annotation> {
        self.annotations.get(annot_id.0)
    }

    pub fn from_bed(
        graph: &PathIndex,
        name: Option<&str>,
        path_name_map: impl Fn(&str) -> String,
        bed_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        use noodles::bed;
        use std::fs::File;
        use std::io::BufReader;

        let name = annotation_set_name(&bed_path, name);

        let mut reader = File::open(bed_path)
            .map(BufReader::new)
            .map(bed::Reader::new)?;

        let mut annotations = Vec::new();
        let mut path_annotations: HashMap<_, Vec<_>> = HashMap::new();

        for result in reader.records::<4>() {
            match result {
                Ok(record) => {
                    if let Some(name) = record.name() {
                        let seqid = &record.reference_sequence_name();

                        let start = record.start_position().get();
                        let end = record.end_position().get();

                        let start_bp = Bp(start as u64);
                        let end_bp = Bp(end as u64);
                        let range = start_bp..end_bp;

                        let path_name = path_name_map(seqid);
                        let path_id = *graph
                            .path_names
                            .get_by_right(&path_name)
                            .ok_or_else(|| {
                                anyhow!("Path not found: {path_name}")
                            })?;

                        let a_id = annotations.len();

                        let annot = Annotation {
                            path: path_id,
                            range,
                            label: Arc::new(name.to_string()),
                        };

                        annotations.push(annot);
                        path_annotations.entry(path_id).or_default().push(a_id);
                    }
                }
                Err(err) => {
                    log::error!("Error parsing GFF record: {err}");
                }
            }
        }

        Ok(Self {
            name,
            annotations,
            path_annotations,
        })
    }

    pub fn from_gff(
        graph: &PathIndex,
        name: Option<&str>,
        path_name_map: impl Fn(&str) -> String,
        record_label: impl Fn(&noodles::gff::Record) -> Option<String>,
        gff_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        use noodles::gff;
        use std::fs::File;
        use std::io::BufReader;

        let name = annotation_set_name(&gff_path, name);

        let mut reader = File::open(gff_path)
            .map(BufReader::new)
            .map(gff::Reader::new)?;

        let mut annotations = Vec::new();
        let mut path_annotations: HashMap<_, Vec<_>> = HashMap::new();

        for result in reader.records() {
            match result {
                Ok(record) => {
                    if let Some(label) = record_label(&record) {
                        dbg!();
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
                            .ok_or_else(|| {
                                anyhow!("Path not found: {path_name}")
                            })?;

                        let a_id = annotations.len();

                        let annot = Annotation {
                            path: path_id,
                            range,
                            label: Arc::new(label.to_string()),
                        };

                        annotations.push(annot);
                        path_annotations.entry(path_id).or_default().push(a_id);
                    }
                }
                Err(err) => {
                    log::error!("Error parsing GFF record: {err}");
                }
            }
        }

        Ok(Self {
            name,
            annotations,
            path_annotations,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotationSetId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotationId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalAnnotationId {
    pub set_id: AnnotationSetId,
    pub annot_id: AnnotationId,
}

pub struct AnnotationStore {
    pub annotation_sets: BTreeMap<AnnotationSetId, Arc<AnnotationSet>>,
    next_set_id: AnnotationSetId,
}

impl std::default::Default for AnnotationStore {
    fn default() -> Self {
        Self {
            annotation_sets: BTreeMap::default(),
            next_set_id: AnnotationSetId(0),
        }
    }
}

impl AnnotationStore {
    pub fn insert_set(&mut self, set: AnnotationSet) -> AnnotationSetId {
        let set_id = self.next_set_id;
        self.next_set_id = AnnotationSetId(set_id.0 + 1);
        self.annotation_sets.insert(set_id, Arc::new(set));
        set_id
    }

    pub fn get_sets_for_path<'a>(
        &'a self,
        path: PathId,
    ) -> impl Iterator<Item = (AnnotationSetId, &'a Arc<AnnotationSet>)> {
        self.annotation_sets
            .iter()
            .filter_map(move |(set_id, set)| {
                set.path_annotations
                    .contains_key(&path)
                    .then_some((*set_id, set))
            })
    }

    pub fn total_annotation_count(&self) -> usize {
        self.annotation_sets
            .values()
            .map(|set| set.annotations.len())
            .sum()
    }
}
