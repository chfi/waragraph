use anyhow::Result;
use std::collections::HashMap;

#[derive(Default, Clone)]
pub struct AnnotationStore {
    // path name -> list of (range, text) pairs
    path_annotations: HashMap<String, Vec<(std::ops::Range<usize>, String)>>,
}

impl AnnotationStore {
    pub fn fill_from_bed(
        &mut self,
        bed_path: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        let mut reader = std::fs::File::open(bed_path)
            .map(std::io::BufReader::new)
            .map(noodles::bed::Reader::new)?;

        let records = reader.records::<3>();

        for record in records {
            println!("{:?}", record);
        }

        Ok(())
    }
}
