/*
    At this stage, the "GUI" consists of two parts:
    - egui for widget stuff
    - a simple flexbox-based layout for slot-based visualizations etc.

    The latter is powered by `taffy`
*/

use std::collections::{BTreeMap, HashMap};

use taffy::prelude::*;

// placeholder
pub enum GuiElem {
    PathSlot {
        slot_id: usize,
        path_id: usize,
        data: &'static str,
    },
    PathName {
        path_id: usize,
    },
    Label {
        id: &'static str,
    },
}

pub enum LayoutNode<T> {
    Single(T),
    HSplit(T, T),
    VSplit(T, T),
}

pub struct FlexLayout<T> {
    taffy: Taffy,
    node_data: BTreeMap<Node, T>,
    root: Option<Node>,
}

pub fn draw_with_layout<T>(
    painter: &egui::Painter,
    dims: ultraviolet::Vec2,
    layout: &mut FlexLayout<T>,
    cb: impl Fn(&egui::Painter, &Layout, &T),
) -> taffy::error::TaffyResult<()> {
    if let Some(root) = layout.root {
        let width = AvailableSpace::Definite(dims.x);
        let height = AvailableSpace::Definite(dims.y);

        let space = Size { width, height };

        // let space = Size::MAX_CONTENT.map_height(|_|)
        // .map()
        // Size::
        layout.taffy.compute_layout(root, space)?;
    }

    for (node, data) in layout.node_data.iter() {
        let layout = layout.taffy.layout(*node)?;
        cb(painter, layout, data);
    }

    Ok(())
}

pub fn taffy_test() -> Result<(), taffy::error::TaffyError> {
    let mut taffy = Taffy::new();

    let child_style = Style {
        size: Size {
            width: Dimension::Points(20.0),
            height: Dimension::Points(20.0),
        },
        ..Default::default()
    };
    let child0 = taffy.new_leaf(child_style)?;
    let child1 = taffy.new_leaf(child_style)?;
    let child2 = taffy.new_leaf(child_style)?;

    let root = taffy.new_with_children(
        Style {
            gap: Size {
                width: Dimension::Points(10.0),
                height: Dimension::Undefined,
            },
            ..Default::default()
        },
        &[child0, child1, child2],
    )?;

    // Compute layout and print result
    taffy.compute_layout(root, Size::MAX_CONTENT)?;
    taffy::debug::print_tree(&taffy, root);

    Ok(())
}
