/*
    At this stage, the "GUI" consists of two parts:
    - egui for widget stuff
    - a simple flexbox-based layout for slot-based visualizations etc.

    The latter is powered by `taffy`
*/

use std::collections::{BTreeMap, HashMap, HashSet};

use taffy::{error::TaffyError, prelude::*};
use ultraviolet::Vec2;

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
    pub taffy: Taffy,
    pub node_data: BTreeMap<Node, T>,
    pub root: Option<Node>,
}

pub fn layout_from_rows_iter<'a, Rows, Item>(
    rows: Rows,
) -> Result<FlexLayout<String>, TaffyError>
// ) -> Result<FlexLayout<(String, Dimension)>, TaffyError>
where
    Rows: IntoIterator<Item = &'a [(Item, Dimension)]>,
    Item: ToString + 'a,
{
    let mut taffy = Taffy::new();

    let root_style = Style {
        position_type: taffy::style::PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        flex_wrap: FlexWrap::Wrap,
        min_size: Size {
            width: Dimension::Auto,
            height: Dimension::Auto,
        },
        margin: Rect {
            left: Dimension::Points(0.0),
            right: Dimension::Points(0.0),
            top: Dimension::Points(10.0),
            bottom: Dimension::Points(0.0),
        },
        size: Size {
            // width: Dimension::Auto,
            // height: Dimension::Auto,
            width: Dimension::Points(800.0),
            height: Dimension::Points(600.0),
        },
        gap: Size {
            width: Dimension::Undefined,
            height: Dimension::Points(10.0),
        },
        padding: Rect {
            left: Dimension::Points(4.0),
            right: Dimension::Points(4.0),
            top: Dimension::Points(22.0),
            bottom: Dimension::Points(0.0),
        },
        ..Default::default()
    };

    let child_style = Style {
        position_type: taffy::style::PositionType::Absolute,
        size: Size {
            width: Dimension::Auto,
            height: Dimension::Points(20.0),
        },
        ..Default::default()
    };
    let mut children = Vec::new();

    let mut node_data = BTreeMap::default();

    for row in rows {
        let mut inner_children = Vec::new();
        // let inner_children = row.iter().
        for (label, dim) in row {
            let mut style = child_style.clone();
            style.size.width = *dim;

            let inner = taffy.new_leaf(style)?;
            node_data.insert(inner, label.to_string());
            // node_data.insert(inner, (label.to_string(), *dim));
            inner_children.push(inner);
        }

        let row_style = Style {
            // position_type: taffy::style::PositionType::Absolute,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            align_self: AlignSelf::Stretch,
            // position_type: PositionType::Relative,
            ..Style::default()
        };

        let row_node = taffy.new_with_children(row_style, &inner_children)?;
        children.push(row_node);
    }

    let root = taffy.new_with_children(root_style, children.as_slice())?;

    taffy.compute_layout(root, Size::MAX_CONTENT)?;
    taffy::debug::print_tree(&taffy, root);

    Ok(FlexLayout {
        taffy,
        node_data,
        root: Some(root),
    })
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
            position: Rect {
                left: Dimension::Points(0.0),
                right: Dimension::Points(0.0),
                top: Dimension::Points(20.0),
                bottom: Dimension::Points(0.0),
            },
            // margin: Rect {
            //     left: Dimension::Points(0.0),
            //     right: Dimension::Points(0.0),
            //     top: Dimension::Points(30.0),
            //     bottom: Dimension::Points(0.0),
            // },
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
                top: Dimension::Points(22.0),
                bottom: Dimension::Points(0.0),
            },
            ..Default::default()
        },
        children.as_slice(),
    )?;

    println!("printing in test_layout");
    taffy.compute_layout(root, Size::MAX_CONTENT)?;
    taffy::debug::print_tree(&taffy, root);

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
    let mut stack: Vec<(Vec2, Node)> = Vec::new();

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
        // dbg!(&space);
        layout.taffy.set_style(root, new_style)?;
        layout.taffy.compute_layout(root, space)?;

        stack.push((Vec2::zero(), root));
    }

    // /*

    let mut visited = HashSet::new();

    while let Some((offset, node)) = stack.pop() {
        visited.insert(node);
        let mut this_layout = *layout.taffy.layout(node)?;
        // let mut this_layout = *this_layout;

        let children = LayoutTree::children(&layout.taffy, node);
        // let offset = this_layout.

        let loc = this_layout.location;
        let this_pos = Vec2::new(loc.x, loc.y);
        let offset = offset + this_pos;

        if let Some(data) = layout.node_data.get(&node) {
            this_layout.location.x = offset.x;
            this_layout.location.y = offset.y;
            cb(painter, &this_layout, data);
        }

        /*
        let location = parent_layout
            .map(|l| l.location)
            .unwrap_or(taffy::geometry::Point::ZERO)
            + this_layout.location;
            */

        for &inner in children {
            if !visited.contains(&inner) {
                stack.push((offset, inner));
            }
        }
    }
    // */
    
    // let space = Size::MAX_CONTENT;
    // for (node, data) in layout.node_data.iter() {
    //     // layout.taffy.compute_layout(*node, space)?;
    //     let layout = layout.taffy.layout(*node)?;

    //     cb(painter, layout, data);
    // }

    Ok(())
}
