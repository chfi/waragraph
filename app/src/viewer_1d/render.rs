use crate::app::settings_menu::SettingsWindow;
use crate::color::{ColorMap, ColorSchemeId};
use crate::util::BufferDesc;

use raving_wgpu::graph::dfrog::Graph;
use raving_wgpu::{NodeId, State, WindowState};

use anyhow::Result;
use waragraph_core::graph::{PathId, PathIndex};

// contains all the config/info needed to render a data buffer
// sampled from the data source corresponding to `data_key`
#[derive(Clone)]
pub struct VizModeConfig {
    pub name: String,
    pub data_key: String,
    pub color_scheme: ColorSchemeId,
    pub default_color_map: ColorMap,
}

pub fn sequence_shapes_in_slot(
    graph: &PathIndex,
    path: PathId,
    view_range: std::ops::Range<u64>,
    rect: egui::Rect,
    shapes: &mut Vec<egui::Shape>,
) {
    // let nodes = graph.nodes_span_iter(view_range);

    let path_set = &graph.path_node_sets[path.ix()];

    for (node, span) in graph.nodes_span_iter(view_range) {
        if path_set.contains(node.into()) {
            let seq = graph.node_sequence(node);

            for (ix, &base) in seq.iter().enumerate() {
                //
            }
            //
        }
    }

    todo!();
}
