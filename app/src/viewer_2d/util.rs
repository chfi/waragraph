use waragraph_core::graph::{Node, PathId, PathIndex};

use crate::context::ContextState;

pub(super) fn path_list_labels(
    graph: &PathIndex,
    node: Node,
    paths: impl IntoIterator<Item = PathId>,
    ui: &mut egui::Ui,
) -> egui::Response {
    ui.vertical(|ui| {
        for path in paths {
            let name = graph.path_names.get_by_left(&path).unwrap();
            let mut label_text = format!("{name}");

            let steps = graph.node_path_step_offsets(node, path);

            if let Some(steps) = steps {
                label_text.push_str(" - ");
                for (ix, (_, pos)) in steps.enumerate() {
                    if ix > 0 {
                        label_text.push_str(", ");
                    }
                    label_text.push_str(&format!("{} bp", pos.0));
                }
            };

            ui.label(label_text);
            ui.end_row();
        }
    })
    .response
}

pub(super) fn node_context_side_panel_info(
    graph: &PathIndex,
    context_state: &ContextState,
    ui: &mut egui::Ui,
) {
    let hovered_node = context_state
        .query_get_cast::<_, Node>(Some("Viewer2D"), ["hover"])
        // .query_get_cast::<_, Node>(None, ["hover"])
        .copied();

    if let Some((node, paths)) =
        hovered_node.and_then(|node| Some((node, graph.paths_on_node(node)?)))
    {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Node {}", node.ix()));
            ui.end_row();
            ui.separator();
            ui.label("Paths on node");
            ui.end_row();
            path_list_labels(graph, node, paths, ui);
        });
    }
}
