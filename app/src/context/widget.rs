use std::collections::HashMap;

use waragraph_core::graph::Node;

use crate::app::SharedState;

use super::{ContextQuery, ContextState, ContextValue, ContextValueExtra};

// pub type CtxWidget = Box<dyn FnMut(&mut egui::Ui, &ContextState)>;
pub type CtxWidget = Box<dyn Fn(&mut egui::Ui, &dyn ContextValue)>;

pub struct ContextInspector {
    widgets: HashMap<String, CtxWidget>,
    active: Vec<(ContextQuery<String>, String)>,
}

impl ContextInspector {
    pub fn new_widget<T, F>(&mut self, name: &str, widget: F)
    where
        T: std::any::Any,
        F: Fn(&mut egui::Ui, &T) + Send + Sync + 'static,
    {
        let widget_fn = move |ui: &mut egui::Ui, ctx: &dyn ContextValue| {
            if let Some(data) = ctx.data().downcast_ref::<T>() {
                widget(ui, data)
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
        ui.vertical(|ui| {
            //
            for (query, widget_name) in self.active.iter_mut() {
                //
                if let Some((widget, ctx)) =
                    self.widgets.get(widget_name).and_then(|w| {
                        let ctx = context_state.get(query)?;
                        Some((w, ctx))
                    })
                {
                    widget(ui, ctx);
                }
            }
            // for (query, widget) in self.active.iter().filter_map(|(q, w_name)| Some((q, self.widgets. {
            //
            // }
            //
        });

        todo!();
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
            move |ui: &mut egui::Ui, ctx: &Node| {
                let len = graph.node_length(*ctx);
                ui.label(len.0.to_string());
            },
        );

        inspector.new_active(
            "node_id",
            ContextQuery::from_source::<Node>("Viewer1D".to_string()),
        );

        inspector
    }
}
