use taffy::error::TaffyError;

use crate::gui::FlexLayout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SlotElem {
    PathData {
        slot_id: usize,
        data_id: std::sync::Arc<String>,
    },
    PathName {
        slot_id: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GuiElem {
    Label { id: String },
}

pub(super) fn create_fixed_gui_layout() -> FlexLayout<GuiElem> {
    todo!();
}

pub(super) fn create_slot_layout(
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
