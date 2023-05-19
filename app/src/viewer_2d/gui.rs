use std::collections::HashSet;
use std::sync::Arc;

use egui::scroll_area::ScrollAreaOutput;
use tokio::sync::RwLock;

use crate::annotations::{AnnotationId, AnnotationStore, GlobalAnnotationId};

pub(super) struct AnnotationPinListWidget {
    annotation_store: Arc<RwLock<AnnotationStore>>,
    filter_string: String,

    pinned: Arc<RwLock<HashSet<GlobalAnnotationId>>>,

    list: Vec<GlobalAnnotationId>,
}

impl AnnotationPinListWidget {
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

        let mut pinned = self.pinned.blocking_read();

        self.list.sort_by(|a0, a1| {
            let a0_pinned = pinned.contains(a0);
            let a1_pinned = pinned.contains(a1);

            if a0_pinned != a1_pinned {
                a0_pinned.cmp(&a1_pinned)
            } else {
                a0.cmp(a1)
            }
        });
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let total_rows = self.list.len();

        let annotation_sets = self
            .annotation_store
            .blocking_read()
            .annotation_sets
            .clone();

        let filter_entry = ui.text_edit_singleline(&mut self.filter_string);

        egui::ScrollArea::vertical().show_rows(
            ui,
            row_height,
            total_rows,
            |ui, range| {
                let mut pinned = self.pinned.blocking_write();

                for ix in range {
                    let annot_id = self.list[ix];
                    let annot_label = annotation_sets
                        .get(&annot_id.set_id)
                        .and_then(|set| set.get(annot_id.annot_id))
                        .map(|annot| annot.label.as_str())
                        .unwrap();

                    let mut is_pinned = pinned.contains(&annot_id);
                    if ui.checkbox(&mut is_pinned, annot_label).clicked() {
                        if is_pinned {
                            pinned.remove(&annot_id);
                        } else {
                            pinned.insert(annot_id);
                        }
                    }
                }
            },
        );
    }
}
