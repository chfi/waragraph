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
    fonts: &egui::text::Fonts,
    graph: &PathIndex,
    path: PathId,
    view_range: std::ops::Range<u64>,
    rect: egui::Rect,
    shapes: &mut Vec<egui::Shape>,
) {
    // let nodes = graph.nodes_span_iter(view_range);
    let view_len = (view_range.end - view_range.start) as f64;

    let path_set = &graph.path_node_sets[path.ix()];

    let p0 = rect.left_center();
    // let bp_width = (view_len / rect.width() as f64) as f32;
    let bp_width = (rect.width() as f64 / view_len) as f32;

    let view_start = view_range.start;

    for (node, span) in graph.nodes_span_iter(view_range.clone()) {
        if path_set.contains(node.into()) {
            let span_l = (span.start.0 - view_start);
            let span_r = (span.end.0 - view_start);

            let xl = rect.left() + (span_l as f32) * bp_width;
            let xr = rect.left() + (span_r as f32) * bp_width;

            let y_range = rect.y_range();

            // let color = egui::Rgba::from_rgba_unmultiplied(0.8, 0.2, 0.2, 0.9);

            let node_rect = egui::Rect::from_x_y_ranges(xl..=xr, y_range);

            let color = {
                use std::hash::{Hash, Hasher};
                let mut hasher =
                    std::collections::hash_map::DefaultHasher::new();
                node.hash(&mut hasher);
                let bytes = hasher.finish().to_ne_bytes();
                let [r, g, b, ..] = bytes;
                egui::Rgba::from_srgba_unmultiplied(r, g, b, 255)
            };

            let shape =
                egui::Shape::rect_filled(node_rect.shrink(1.0), 0., color);
            shapes.push(shape);

            let seq = graph.node_sequence(node);

            for (ix, &base) in seq.iter().enumerate() {
                let x = xl + bp_width / 2.0 + bp_width * ix as f32;

                let c = base as char;

                let shape = egui::Shape::text(
                    fonts,
                    egui::pos2(x, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    c,
                    egui::FontId::monospace(10.0),
                    egui::Color32::BLACK,
                );
                shapes.push(shape);
                //
            }
        }
    }
}

// fn base_color(b: u8) ->
