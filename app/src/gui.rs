/*
    At this stage, the "GUI" consists of two parts:
    - egui for widget stuff
    - a simple flexbox-based layout for slot-based visualizations etc.

    The latter is powered by `taffy`
*/

use std::collections::{BTreeMap, HashSet};

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

impl<T> FlexLayout<T> {
    pub fn map_node_data<F, U>(self, f: F) -> FlexLayout<U>
    where
        F: Fn(T) -> U,
    {
        let node_data =
            self.node_data.into_iter().map(|(k, v)| (k, f(v))).collect();

        FlexLayout {
            taffy: self.taffy,
            node_data,
            root: self.root,
        }
    }

    pub fn from_rows_iter<Rows, Row>(rows: Rows) -> Result<Self, TaffyError>
    where
        Rows: IntoIterator<Item = Row>,
        Row: IntoIterator<Item = (T, Dimension)>,
    {
        let root_style = Style {
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
                width: Dimension::Auto,
                height: Dimension::Auto,
                // width: Dimension::Points(800.0),
                // height: Dimension::Points(600.0),
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
        };

        let row_style = Style {
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            padding: Rect {
                left: Dimension::Points(4.0),
                right: Dimension::Points(4.0),
                top: Dimension::Points(0.0),
                bottom: Dimension::Points(0.0),
            },
            align_self: AlignSelf::Stretch,
            // position_type: PositionType::Relative,
            ..Style::default()
        };

        let child_style = Style {
            margin: Rect {
                left: Dimension::Points(4.0),
                right: Dimension::Points(4.0),
                top: Dimension::Points(0.0),
                bottom: Dimension::Points(0.0),
            },
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Points(20.0),
            },
            ..Default::default()
        };

        let mut taffy = Taffy::new();

        let mut children = Vec::new();

        let mut node_data = BTreeMap::default();
        let mut inner_children = Vec::new();

        for row in rows {
            inner_children.clear();

            for (data, dim) in row {
                let mut style = child_style.clone();
                style.size.width = dim;

                let inner = taffy.new_leaf(style)?;
                node_data.insert(inner, data);
                inner_children.push(inner);
            }

            let row_node =
                taffy.new_with_children(row_style, &inner_children)?;
            children.push(row_node);
        }

        let root = taffy.new_with_children(root_style, children.as_slice())?;

        Ok(FlexLayout {
            taffy,
            node_data,
            root: Some(root),
        })
    }

    pub fn visit_layout(
        &mut self,
        dims: Vec2,
        mut v: impl FnMut(Layout, &T),
    ) -> Result<(), TaffyError> {
        let mut stack: Vec<(Vec2, Node)> = Vec::new();

        if let Some(root) = self.root {
            let width = AvailableSpace::Definite(dims.x);
            let height = AvailableSpace::Definite(dims.y);

            let space = Size { width, height };

            let new_style = {
                let style = self.taffy.style(root)?;
                Style {
                    size: Size {
                        width: Dimension::Points(dims.x),
                        height: Dimension::Points(dims.y),
                    },
                    ..style.clone()
                }
            };
            self.taffy.set_style(root, new_style)?;
            self.taffy.compute_layout(root, space)?;

            stack.push((Vec2::zero(), root));
        }

        let mut visited = HashSet::new();

        while let Some((offset, node)) = stack.pop() {
            visited.insert(node);
            let mut this_layout = *self.taffy.layout(node)?;
            // let mut this_layout = *this_layout;

            let children = LayoutTree::children(&self.taffy, node);
            // let offset = this_layout.

            let loc = this_layout.location;
            let this_pos = Vec2::new(loc.x, loc.y);
            let offset = offset + this_pos;

            if let Some(data) = self.node_data.get(&node) {
                this_layout.location.x = offset.x;
                this_layout.location.y = offset.y;
                v(this_layout, data);
            }

            for &inner in children {
                if !visited.contains(&inner) {
                    stack.push((offset, inner));
                }
            }
        }

        Ok(())
    }
}

pub fn layout_egui_rect(layout: &Layout) -> egui::Rect {
    let btm_left = layout.location;
    let size = layout.size;
    let size = egui::vec2(size.width, size.height);

    let bl = egui::pos2(btm_left.x, btm_left.y);
    let center = bl + egui::vec2(size.x / 2.0, -size.y / 2.0);
    egui::Rect::from_center_size(center, size)
}