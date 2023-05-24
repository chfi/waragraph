use std::collections::HashSet;
use std::sync::Arc;

use egui::scroll_area::ScrollAreaOutput;
use tokio::sync::RwLock;

use crate::annotations::{
    Annotation, AnnotationId, AnnotationStore, GlobalAnnotationId,
};

pub(crate) struct AnnotationListWidget {
    annotation_store: Arc<RwLock<AnnotationStore>>,
    filter_string: String,

    list: Vec<GlobalAnnotationId>,
}

impl AnnotationListWidget {
    pub fn new(annotation_store: Arc<RwLock<AnnotationStore>>) -> Self {
        let mut result = Self {
            annotation_store,
            filter_string: String::new(),
            list: Vec::new(),
        };

        result.recreate_list();

        result
    }

    fn recreate_list(&mut self) {
        let annotations = self.annotation_store.blocking_read();

        self.list.clear();
        for (set_id, set) in annotations.annotation_sets.iter() {
            for (a_id, annot) in set.annotations.iter().enumerate() {
                if annot.label.contains(&self.filter_string) {
                    let global_id = GlobalAnnotationId {
                        set_id: *set_id,
                        annot_id: AnnotationId(a_id),
                    };

                    self.list.push(global_id);
                }
            }
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        mut show_annotation: impl FnMut(&mut egui::Ui, &Annotation),
    ) {
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let total_rows = self.list.len();

        let annotation_sets = self
            .annotation_store
            .blocking_read()
            .annotation_sets
            .clone();

        let filter_entry = ui.text_edit_singleline(&mut self.filter_string);

        if filter_entry.changed() {
            self.recreate_list();
        }

        egui::ScrollArea::vertical().max_height(500.0).show_rows(
            ui,
            row_height,
            total_rows,
            |ui, range| {
                for ix in range {
                    let annotation = self
                        .list
                        .get(ix)
                        .and_then(|annot_id| {
                            let set = annotation_sets.get(&annot_id.set_id)?;
                            set.get(annot_id.annot_id)
                        })
                        .unwrap();

                    show_annotation(ui, annotation);
                }
            },
        );
    }
}
