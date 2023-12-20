use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;
use raving_wgpu::egui;
use waragraph_core::graph::{Node, PathId};

// use crate::{
//     annotations::GlobalAnnotationId,
//     app::{
//         settings_menu::{SettingsUiResponse, SettingsWidget},
//         AppType, SharedState,
//     },
// };

use crate::SharedState;

use super::{
    ContextMeta, ContextQuery, ContextState, ContextValue, ContextValueExtra,
};

// pub type CtxWidget = Box<dyn FnMut(&mut egui::Ui, &ContextState)>;
pub type CtxWidget = Box<dyn Fn(&mut egui::Ui, &dyn ContextValue)>;

#[derive(Default)]
pub struct ContextInspector {
    widgets: HashMap<String, CtxWidget>,
    active: Vec<(ContextQuery<String>, String)>,
}

impl ContextInspector {
    // pub fn active_targets(&self) -> HashSet<AppType> {
    //     let settings = self.settings.read();
    //     settings.inspector_targets.clone()
    // }

    pub fn new_widget<T, F>(&mut self, name: &str, widget: F)
    where
        T: std::any::Any,
        F: Fn(&mut egui::Ui, &ContextMeta, &T) + Send + Sync + 'static,
    {
        let widget_fn = move |ui: &mut egui::Ui, ctx: &dyn ContextValue| {
            if let Some(data) = ctx.data().downcast_ref::<T>() {
                widget(ui, ctx.meta(), data)
            }
        };

        self.widgets
            .insert(name.to_string(), Box::new(widget_fn) as CtxWidget);
    }

    pub fn new_active(
        &mut self,
        widget_name: &str,
        query: ContextQuery<String>,
    ) {
        if self.widgets.contains_key(widget_name) {
            self.active.push((query, widget_name.to_string()));
        }
    }

    pub fn show(&mut self, context_state: &ContextState, ui: &mut egui::Ui) {
        // context_state.debug_print();

        ui.vertical(|ui| {
            for (query, widget_name) in self.active.iter_mut() {
                if let Some((widget, ctx)) =
                    self.widgets.get(widget_name).and_then(|w| {
                        let ctx = context_state.get(query)?;
                        Some((w, ctx))
                    })
                {
                    widget(ui, ctx);
                }
            }
        });
    }

    pub fn with_default_widgets(shared: &SharedState) -> Self {
        let mut inspector = Self {
            widgets: HashMap::default(),
            active: Vec::new(),
        };

        // Node length
        let graph = shared.graph.clone();
        inspector.new_widget(
            "node_length",
            move |ui: &mut egui::Ui, _, ctx: &Node| {
                let len = graph.node_length(*ctx).0;
                ui.label(len.to_string());
            },
        );

        // node, short desc
        let graph = shared.graph.clone();
        inspector.new_widget(
            "node_short",
            move |ui: &mut egui::Ui, meta: &ContextMeta, &node: &Node| {
                let id = node.ix();
                let len = graph.node_length(node).0;
                let source = &meta.source;
                let tag = meta
                    .tags
                    .set
                    .iter()
                    .map(|s| s.as_str())
                    .next()
                    .unwrap_or("");
                ui.label(format!(" [{source}:{tag}] Node {id} - {len}bp"));
            },
        );

        // path, short desc
        let graph = shared.graph.clone();
        inspector.new_widget(
            "path_short",
            move |ui: &mut egui::Ui, meta: &ContextMeta, &path: &PathId| {
                let path_name = graph
                    .path_names
                    .get_by_left(&path)
                    .map(|s| s.as_str())
                    .unwrap_or("<ERROR>");
                let steps_len = graph.path_steps[path.ix()].len();

                let source = &meta.source;
                let tag = meta
                    .tags
                    .set
                    .iter()
                    .map(|s| s.as_str())
                    .next()
                    .unwrap_or("");
                ui.label(format!(
                    " [{source}:{tag}] Path {path_name} - {steps_len} steps"
                ));
            },
        );

        /*
        // annotation, with path, short desc
        let graph = shared.graph.clone();
        let annotations = shared.annotations.clone();
        inspector.new_widget(
            "annotation_path_short",
            move |ui: &mut egui::Ui,
                  meta: &ContextMeta,
                  (path, annot_id): &(PathId, GlobalAnnotationId)| {
                let annot_text = {
                    let annots = annotations.blocking_read();
                    let set =
                        annots.annotation_sets.get(&annot_id.set_id).unwrap();
                    let annot = set.get(annot_id.annot_id).unwrap();
                    annot.label.to_string()
                };

                let path_name = graph
                    .path_names
                    .get_by_left(&path)
                    .map(|s| s.as_str())
                    .unwrap_or("<ERROR>");

                let source = &meta.source;
                let tag = meta
                    .tags
                    .set
                    .iter()
                    .map(|s| s.as_str())
                    .next()
                    .unwrap_or("");
                ui.label(format!(
                    " [{source}:{tag}] Path {path_name} - {annot_text}"
                ));
            },
        );

        inspector.new_active(
            "annotation_path_short",
            ContextQuery::from_source::<(PathId, GlobalAnnotationId)>(
                "Viewer1D".to_string(),
            ),
        );
        */

        // inspector.new_active(
        //     "node_length",
        //     ContextQuery::from_source::<Node>("Viewer1D".to_string()),
        // );

        inspector.new_active(
            "node_short",
            ContextQuery::from_source::<Node>("Viewer1D".to_string()),
        );

        inspector.new_active(
            "path_short",
            ContextQuery::from_source::<Node>("Viewer1D".to_string()),
        );

        inspector
    }
}