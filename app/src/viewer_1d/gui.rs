use taffy::error::TaffyError;
use waragraph_core::graph::{Bp, PathId};

use crate::gui::FlexLayout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SlotElem {
    Empty,
    ViewRange,
    PathData {
        slot_id: usize,
        data_id: std::sync::Arc<String>,
    },
    PathName {
        slot_id: usize,
    },
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
        font_id,
        color,
    );

    [left_text, right_text].into_iter()
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
