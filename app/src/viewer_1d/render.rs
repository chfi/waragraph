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

            let color = {
                use std::hash::{Hash, Hasher};
                let mut hasher =
                    std::collections::hash_map::DefaultHasher::new();
                node.hash(&mut hasher);
                let bytes = hasher.finish().to_ne_bytes();
                let [r, g, b, ..] = bytes;
                egui::Rgba::from_srgba_unmultiplied(r, g, b, 255)
            };

            let shape = egui::Shape::rect_filled(
                egui::Rect::from_x_y_ranges(xl..=xr, y_range).shrink(1.0),
                0.,
                color,
            );
            shapes.push(shape);

            /*
            let seq = graph.node_sequence(node);

            let span_start = span.start.0 as usize;
            let p0 = p0
                + egui::vec2(bp_width, 0.0)
                    * (span_start - view_range.start as usize) as f32;

            for (ix, &base) in seq.iter().enumerate() {
                // let pos = p0 + egui::vec2(bp_width, 0.0) * ix as f32;
                let pos = p0 + egui::vec2(bp_width, 0.0) * ix as f32;

                let color = match base.to_ascii_uppercase() {
                    b'G' => {
                        egui::Rgba::from_rgba_unmultiplied(0.8, 0.2, 0.2, 0.5)
                    }
                    b'C' => {
                        egui::Rgba::from_rgba_unmultiplied(0.2, 0.8, 0.2, 0.5)
                    }
                    b'T' => {
                        egui::Rgba::from_rgba_unmultiplied(0.2, 0.2, 0.8, 0.5)
                    }
                    b'A' => {
                        egui::Rgba::from_rgba_unmultiplied(0.8, 0.8, 0.2, 0.5)
                    }
                    _ => egui::Rgba::from_rgba_unmultiplied(0.3, 0.3, 0.3, 0.5),
                };

                let shape = egui::Shape::rect_filled(
                    egui::Rect::from_center_size(
                        pos,
                        egui::vec2(bp_width, rect.height()),
                    ),
                    0.0,
                    color,
                );

                shapes.push(shape);
            }
            */
            //
        }
    }
}

// fn base_color(b: u8) ->
