use std::collections::HashSet;
use std::sync::Arc;

use egui::scroll_area::ScrollAreaOutput;
use tokio::sync::RwLock;

use crate::annotations::{AnnotationStore, GlobalAnnotationId};

pub(super) struct AnnotationPinListWidget {
    annotation_store: Arc<RwLock<AnnotationStore>>,
    filter_string: String,
    pinned: Arc<RwLock<HashSet<GlobalAnnotationId>>>,
}

impl AnnotationPinListWidget {
    //

    pub(super) fn show(ui: &mut egui::Ui) {
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let total_rows = annotations.total_annotation_count();

        egui::ScrollArea::vertical().show_rows(
            ui,
            row_height,
            total_rows,
            |ui, range| {
                let mut pinned = pinned.blocking_write();

                for ix in range {
                    let annot_id = annotations[ix];
                    let annot_label: &str = todo!();

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
