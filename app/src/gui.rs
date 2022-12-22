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

pub fn test_layout() -> taffy::error::TaffyResult<FlexLayout<String>> {
    let mut taffy = Taffy::new();

    let mut node_data: BTreeMap<Node, String> = BTreeMap::default();

    let mut children = Vec::new();

    let style = Style {
        size: Size {
            width: Dimension::Auto,
            // width: Dimension::Percent(1.0),
            // width: Dimension::Points(20.0),
            // width: Dimension::Undefined,
            height: Dimension::Points(20.0),
        },
        ..Default::default()
    };

    for ix in 0..10 {
        let node = taffy.new_leaf(style.clone())?;
        // taffy.set_measure(node, measure)
        node_data.insert(node, format!("node:{ix}"));
        children.push(node);
    }

    let root = taffy.new_with_children(
        Style {
            flex_direction: FlexDirection::Column,
            flex_wrap: FlexWrap::Wrap,
            // align_items: AlignItems::Stretch,
            // align_self: AlignSelf::Stretch,
            // align_content: AlignContent::Stretch,
            min_size: Size {
                // width: Dimension::Percent(100.0),
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            size: Size {
                // width: Dimension::Percent(100.0),
                // height: Dimension::Percent(1.0),
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            gap: Size {
                width: Dimension::Undefined,
                height: Dimension::Points(10.0),
            },
            padding: Rect {
                left: Dimension::Points(4.0),
                right: Dimension::Points(4.0),
                top: Dimension::Points(0.0),
                bottom: Dimension::Points(0.0),
            },
            ..Default::default()
        },
        children.as_slice(),
    )?;

    // let width = AvailableSpace::Definite(800.0);
    // let height = AvailableSpace::Definite(600.0);

    // let space = Size { width, height };
    // taffy.compute_layout(root, space)?;
    // taffy::debug::print_tree(&taffy, root);

    Ok(FlexLayout {
        taffy,
        node_data,
        root: Some(root),
    })
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

        let new_style = {
            let style = layout.taffy.style(root)?;
            Style {
                size: Size {
                    width: Dimension::Points(dims.x),
                    height: Dimension::Points(dims.y),
                },
                ..style.clone()
            }
        };
        layout.taffy.set_style(root, new_style)?;
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
                // width: Dimension::Undefined,
                // height: Dimension::Points(10.0),
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

