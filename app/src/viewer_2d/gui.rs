use std::collections::HashSet;
use std::sync::Arc;

use egui::{mutex::Mutex, scroll_area::ScrollAreaOutput};
use tokio::sync::RwLock;

use crate::annotations::{AnnotationId, AnnotationStore, GlobalAnnotationId};

// egui::util::id_type_map::
pub(super) fn toggle_pinned_annotation(
    ui: &mut egui::Ui,
    annot_id: GlobalAnnotationId,
) {
    ui.data_mut(|data| {
        let pinned_annots: &mut Arc<Mutex<HashSet<GlobalAnnotationId>>> = data
            .get_temp_mut_or_insert_with(egui::Id::null(), || {
                Arc::new(Mutex::new(Default::default()))
            });

        let mut pinned_annots = pinned_annots.lock();

        if pinned_annots.contains(&annot_id) {
            pinned_annots.remove(&annot_id);
        } else {
            pinned_annots.insert(annot_id);
        }
    })
}
