use taffy::error::TaffyError;
use waragraph_core::graph::{Bp, PathId};

use crate::gui::FlexLayout;

use super::annotations::AnnotSlotId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SlotElem {
    Empty,
    ViewRange,
    PathData { path_id: PathId, data_id: String },
    PathName { path_id: PathId },
    Annotations { annotation_slot_id: AnnotSlotId },
    // Annotations { path: PathId, annotation_id: String },
}

pub(super) fn fill_h_range_of_rect(
    color: impl Into<egui::Color32>,
    rect: egui::Rect,
    range: std::ops::RangeInclusive<f32>,
) -> egui::Shape {
    let range_rect = egui::Rect::from_x_y_ranges(range, rect.y_range());
    egui::Shape::rect_filled(range_rect, 0.0, color)

    /*
                            if range_rect.width() >= 1.0 {
                                egui::Shape::rect_filled(range_rect, 0.0, color)
                            } else {
                                // this part looks a bit weird since there's a lot of subpixel ranges
                                // that actually overlap, so leave it for later
                                let rect = range_rect.expand(1.0);
                                let color = egui::Rgba::RED;
                                let stroke = egui::Stroke::new(1.0, color);
                                let top = rect.center_top();
                                let btm = rect.center_bottom();

                                use egui::vec2;
                                let mid = [top, btm];
                                let ul = top + vec2(-1.0, -1.0);
                                let ur = top + vec2(1.0, -1.0);
                                let dl = btm + vec2(-1.0, 1.0);
                                let dr = btm + vec2(1.0, 1.0);

                                let upper = [ul, ur];
                                let lower = [dl, dr];

                                egui::Shape::Vec(vec![
                                    egui::Shape::line_segment(upper, stroke),
                                    egui::Shape::line_segment(mid, stroke),
                                    egui::Shape::line_segment(lower, stroke),
                                ])
                            }
    */
}

pub(super) fn view_range_shapes(
    fonts: &egui::text::Fonts,
    rect: egui::Rect,
    left: Bp,
    right: Bp,
    ruler: Option<Bp>,
) -> impl Iterator<Item = egui::Shape> {
    let center = rect.center();

    let pad = 1.0;

    let r_left = rect.left() + pad;
    let r_right = rect.right() - pad;
    let r_mid_y = center.y;

    let font_id = egui::FontId::monospace(16.0);
    let color = egui::Color32::WHITE;

    let left_pos = egui::pos2(r_left, r_mid_y);
    let right_pos = egui::pos2(r_right, r_mid_y);

    let left_text = egui::Shape::text(
        &fonts,
        left_pos,
        egui::Align2::LEFT_CENTER,
        left.0,
        font_id.clone(),
        color,
    );

    let right_text = egui::Shape::text(
        &fonts,
        right_pos,
        egui::Align2::RIGHT_CENTER,
        right.0,
        font_id.clone(),
        color,
    );

    let ruler_shapes = ruler.map(|r| {
        let rf = r.0 as f64;
        let rl = left.0 as f64;
        let rr = right.0 as f64;

        let t = (rf - rl) / (rr - rl);
        let w = (r_right as f64) - (r_left as f64);
        let x = (t * w) as f32;

        let rt_pos = egui::pos2(r_left + x + 4.0, r_mid_y);

        let ruler_text = egui::Shape::text(
            &fonts,
            rt_pos,
            egui::Align2::LEFT_CENTER,
            r.0,
            font_id,
            color,
        );

        // let stroke = egui::Stroke::new(2.0, color);

        // let r_pos_u = egui::pos2(x, rect.top());
        // let r_pos_d = egui::pos2(x, rect
        // let ruler_line = egui::Shape::line_segment

        ruler_text
    });

    [left_text, right_text].into_iter().chain(ruler_shapes)
}

/*
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GuiElem {
    Label { id: String },
}

fn create_fixed_gui_layout() -> FlexLayout<GuiElem> {
    todo!();
}

fn create_slot_layout(
    slots: usize,
    data_id: &str,
) -> Result<FlexLayout<SlotElem>, TaffyError> {
    use taffy::prelude::*;

    let data_id = std::sync::Arc::new(data_id.to_string());

    let mut rows = Vec::with_capacity(slots);

    let mk_entry = |perc: f32, elem: SlotElem| (elem, Dimension::Percent(perc));

    for slot_id in 0..slots {
        rows.push(vec![
            mk_entry(0.2, SlotElem::PathName { slot_id }),
            mk_entry(
                0.8,
                SlotElem::PathData {
                    slot_id,
                    data_id: data_id.clone(),
                },
            ),
        ]);
    }

    FlexLayout::from_rows_iter(rows)
}
*/
