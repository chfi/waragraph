/*
    At this stage, the "GUI" consists of two parts:
    - egui for widget stuff
    - a simple flexbox-based layout for slot-based visualizations etc.

    The latter is powered by `taffy`
*/

use std::collections::{BTreeMap, HashMap, HashSet};

use taffy::{error::TaffyError, prelude::*};
use ultraviolet::Vec2;

pub mod list;

pub mod util;

/*
Each row can contain its own inline grid layout, subject to parameters shared
by the entire layout -- or, if needed, a row can control its own size and grid
*/
pub struct RowGridLayout<T> {
    taffy: Taffy,
    node_data: BTreeMap<Node, T>,
    root: Option<Node>,

    root_style: Style,
    row_base_style: Style,
    // row_templates: HashMap<String, RowTemplate>,

    // in logical pixels
    computed_for_rect: Option<egui::Rect>,
}

// enum RowTemplate {
//     Grid,
//     Flex,
//     Single,
// }

// pub struct RowTemplate {
//     style: Style,
// }

impl<T> RowGridLayout<T> {
    //
    pub fn new() -> Self {
        let root_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..Default::default()
        };

        let base_row_height = 20.0; // pixels

        let row_base_style = Style {
            display: Display::Grid,
            flex_basis: points(base_row_height),
            grid_template_rows: vec![points(100.0), fr(1.0)],
            grid_template_columns: vec![fr(1.0)],
            ..Default::default()
        };

        Self {
            taffy: Taffy::new(),
            node_data: BTreeMap::default(),
            root: None,

            root_style,
            row_base_style,

            computed_for_rect: None,
        }
    }

    pub fn compute_layout(&mut self, rect: egui::Rect) -> anyhow::Result<()> {
        let root = if let Some(root) = self.root {
            root
        } else {
            anyhow::bail!(RowGridLayoutError::ComputeEmptyLayout);
        };

        // let sized_root_style =
        //     Style {
        //         size: Size {
        //             width: points(rect.width()),
        //             height: points(rect.height()),
        //         },
        //         ..self.root_style.clone()
        //     }
        // ;
        // self.taffy.set_style(root, sized_root_style)?;

        let container_space = Size {
            width: AvailableSpace::from_points(rect.width()),
            height: AvailableSpace::from_points(rect.height()),
        };
        self.taffy.compute_layout(root, container_space)?;

        self.computed_for_rect = Some(rect);

        Ok(())
    }

    pub fn build_layout_for_rows<Rows>(
        &mut self,
        //
        rows: Rows,
    ) -> Result<(), TaffyError>
    where
        Rows: IntoIterator<Item = [Option<T>; 2]>,
    {
        // create children
        let mut children = Vec::new();

        // let child_style =

        for column_data in rows {
            // create inner columns

            let elem_style = |i: i16| Style {
                grid_row: line(i),
                grid_column: span(1),
                ..Default::default()
            };

            // let elem_style = Style {
            //     grid_row: span(1),
            //     grid_column: span(1),
            //     ..Default::default()
            // };

            // let n1 = self.taffy.new_leaf(elem_style(1))?;
            // let n2 = self.taffy.new_leaf(elem_style(2))?;

            let mut row_children = Vec::new();

            for (i, d) in column_data.into_iter().enumerate() {
                if let Some(d) = d {
                    let i = (i + 1) as i16;
                    let n = self.taffy.new_leaf(elem_style(i))?;
                    self.node_data.insert(n, d);
                    row_children.push(n);
                }
            }

            // create row container
            let row = self.taffy.new_with_children(
                self.row_base_style.clone(),
                &row_children,
            )?;

            children.push(row);
        }

        // create root container with children
        let root = self
            .taffy
            .new_with_children(self.root_style.clone(), &children)?;

        self.root = Some(root);

        Ok(())
    }

    pub fn visit_layout(
        &self,
        mut visitor: impl FnMut(Layout, &T),
        // ) -> Result<(), TaffyError> {
    ) -> anyhow::Result<()> {
        let mut stack: Vec<(Vec2, Node)> = Vec::new();

        let container_offset = if let Some(rect) = self.computed_for_rect {
            let lt = rect.left_top();
            Vec2::new(lt.x, lt.y)
        } else {
            anyhow::bail!(RowGridLayoutError::VisitBeforeLayout);
        };

        if let Some(root) = self.root {
            stack.push((container_offset, root));
        }

        let mut visited = HashSet::new();

        while let Some((offset, node)) = stack.pop() {
            // println!("visit! {}", visited.len());
            visited.insert(node);
            let mut this_layout = *self.taffy.layout(node)?;

            let children = LayoutTree::children(&self.taffy, node);

            let loc = this_layout.location;
            let this_pos = Vec2::new(loc.x, loc.y);
            let offset = offset + this_pos;
            // let offset = container_offset + offset + this_pos;

            if let Some(data) = self.node_data.get(&node) {
                this_layout.location.x = offset.x;
                this_layout.location.y = offset.y;

                visitor(this_layout, data);
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

// #[derive(Clone)]
// pub struct RowLayoutStyle {
//     container_style: Style,
//     row_container_style: Style,
//     inner_row_styles: HashMap<String, Style>,
// }

/*
pub struct RowLayout<C, T> {
    // in logical pixels
    computed_for_rect: Option<egui::Rect>,

    taffy: Taffy,
    node_data: BTreeMap<Node, T>,
    root: Option<Node>,
}

*/

pub struct FlexLayout<T> {
    offset: Vec2,

    taffy: Taffy,
    node_data: BTreeMap<Node, T>,
    root: Option<Node>,

    computed_size: Option<Vec2>,
}

impl<T> std::default::Default for FlexLayout<T> {
    fn default() -> Self {
        Self {
            offset: Vec2::zero(),

            taffy: Taffy::new(),
            node_data: BTreeMap::default(),
            root: None,

            computed_size: None,
        }
    }
}

impl<T> FlexLayout<T> {
    fn root_style() -> Style {
        Style {
            flex_direction: FlexDirection::Column,
            flex_wrap: FlexWrap::Wrap,
            min_size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            margin: Rect {
                left: LengthPercentageAuto::Points(0.0),
                right: LengthPercentageAuto::Points(0.0),
                top: LengthPercentageAuto::Points(10.0),
                bottom: LengthPercentageAuto::Points(0.0),
            },
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            gap: Size {
                width: LengthPercentage::ZERO,
                height: LengthPercentage::Points(10.0),
            },
            padding: Rect {
                left: LengthPercentage::Points(4.0),
                right: LengthPercentage::Points(4.0),
                top: LengthPercentage::Points(0.0),
                bottom: LengthPercentage::Points(0.0),
            },
            ..Default::default()
        }
    }

    fn row_style() -> Style {
        Style {
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            margin: Rect {
                left: LengthPercentageAuto::Points(4.0),
                right: LengthPercentageAuto::Points(4.0),
                top: LengthPercentageAuto::Points(0.0),
                bottom: LengthPercentageAuto::Points(0.0),
            },
            padding: Rect {
                left: LengthPercentage::Points(4.0),
                right: LengthPercentage::Points(4.0),
                top: LengthPercentage::Points(0.0),
                bottom: LengthPercentage::Points(0.0),
            },
            align_self: Some(AlignItems::Stretch),
            // position_type: PositionType::Relative,
            ..Style::default()
        }
    }

    fn child_style(
        width: Option<Dimension>,
        height: Option<Dimension>,
    ) -> Style {
        let width = width.unwrap_or(Dimension::Auto);
        let height = height.unwrap_or(Dimension::Auto);

        Style {
            margin: Rect {
                left: LengthPercentageAuto::Points(4.0),
                right: LengthPercentageAuto::Points(4.0),
                top: LengthPercentageAuto::Points(0.0),
                bottom: LengthPercentageAuto::Points(0.0),
            },
            size: Size { width, height },
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.taffy.clear();
        self.node_data.clear();
        self.root = None;
        self.computed_size = None;
    }

    pub fn computed_size(&self) -> Option<Vec2> {
        self.computed_size
    }

    pub fn offset(&self) -> Vec2 {
        self.offset
    }

    pub fn map_node_data<F, U>(self, f: F) -> FlexLayout<U>
    where
        F: Fn(T) -> U,
    {
        let node_data =
            self.node_data.into_iter().map(|(k, v)| (k, f(v))).collect();

        FlexLayout {
            offset: self.offset,
            taffy: self.taffy,
            node_data,
            root: self.root,
            computed_size: self.computed_size,
        }
    }

    /*
    pub fn prepend_rows<Rows, Row>(
        &mut self,
        rows: Rows,
    ) -> Result<(), TaffyError>
    where
        Rows: IntoIterator<Item = Row>,
        Row: IntoIterator<Item = (T, Dimension, Dimension)>,
    {
        let root_style = Self::root_style();
        let row_style = Self::row_style();

        let child_style =
            Self::child_style(None, Some(Dimension::Points(20.0)));

        let mut children = Vec::new();
        let mut inner_children = Vec::new();

        for row in rows {
            inner_children.clear();

            for (data, width, height) in row {
                let mut style = child_style.clone();
                style.size.width = width;
                style.size.height = height;

                let inner = self.taffy.new_leaf(style)?;
                self.node_data.insert(inner, data);
                inner_children.push(inner);
            }

            let row_node =
                self.taffy.new_with_children(row_style, &inner_children)?;
            children.push(row_node);
        }

        if let Some(root) = self.root {
            let mut new_children = children;
            new_children.extend(self.taffy.children(root)?);
            self.taffy.set_children(root, &new_children)?;
        } else {
            let root = self
                .taffy
                .new_with_children(root_style, children.as_slice())?;

            self.root = Some(root);
        }

        Ok(())
    }
    */

    pub fn fill_with_rows<Rows, Row>(
        &mut self,
        rows: Rows,
    ) -> Result<(), TaffyError>
    where
        Rows: IntoIterator<Item = Row>,
        Row: IntoIterator<Item = (T, Dimension, Dimension)>,
    {
        let root_style = Self::root_style();
        let row_style = Self::row_style();

        let child_style =
            Self::child_style(None, Some(Dimension::Points(20.0)));

        let mut children = Vec::new();
        let mut inner_children = Vec::new();

        for row in rows {
            inner_children.clear();

            for (data, width, height) in row {
                let mut style = child_style.clone();
                style.size.width = width;
                style.size.height = height;

                let inner = self.taffy.new_leaf(style)?;
                self.node_data.insert(inner, data);
                inner_children.push(inner);
            }

            let row_node = self
                .taffy
                .new_with_children(row_style.clone(), &inner_children)?;
            children.push(row_node);
        }

        if let Some(root) = self.root {
            let mut new_children = self.taffy.children(root)?;
            new_children.extend(children);
            self.taffy.set_children(root, &new_children)?;
        } else {
            let root = self
                .taffy
                .new_with_children(root_style, children.as_slice())?;

            self.root = Some(root);
        }

        Ok(())
    }

    pub fn from_rows_iter<Rows, Row>(rows: Rows) -> Result<Self, TaffyError>
    where
        Rows: IntoIterator<Item = Row>,
        Row: IntoIterator<Item = (T, Dimension, Dimension)>,
    {
        let mut layout = FlexLayout::default();
        layout.fill_with_rows(rows)?;

        Ok(layout)
    }

    pub fn compute_layout(
        &mut self,
        offset: Vec2,
        dims: Vec2,
    ) -> Result<usize, TaffyError> {
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
        }

        self.offset = offset;
        self.computed_size = Some(dims);

        let offset = self.offset;
        let dims = self.computed_size.unwrap_or_default();

        let center = offset + dims / 2.0;

        let center = egui::pos2(center.x, center.y);
        let dims = egui::vec2(dims.x, dims.y);

        let screen_rect = egui::Rect::from_center_size(center, dims);

        // loop through all rows, counting the ones that are visible
        let root_children = {
            let mut chdn = self.taffy.children(self.root.unwrap())?;

            let mut i = 0;

            loop {
                if i >= chdn.len() {
                    break;
                }

                let node = chdn[i];
                let layout = self.taffy.layout(node)?;
                let rect = layout_egui_rect(layout);

                // this intersect check only works because of the margin
                // in the row style
                if !rect.intersects(screen_rect) {
                    chdn.remove(i);
                } else {
                    i += 1;
                }
            }

            chdn.len()
        };

        Ok(root_children)
    }

    pub fn visit_layout(
        &self,
        mut v: impl FnMut(Layout, &T),
    ) -> Result<(), TaffyError> {
        let mut stack: Vec<(Vec2, Node)> = Vec::new();

        if let Some(root) = self.root {
            stack.push((Vec2::zero(), root));
        }

        let mut visited = HashSet::new();

        while let Some((offset, node)) = stack.pop() {
            // println!("visit! {}", visited.len());
            visited.insert(node);
            let mut this_layout = *self.taffy.layout(node)?;

            let children = LayoutTree::children(&self.taffy, node);

            let loc = this_layout.location;
            let this_pos = Vec2::new(loc.x, loc.y);
            let offset = self.offset + offset + this_pos;

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
    let center = bl + egui::vec2(size.x / 2.0, size.y / 2.0);
    egui::Rect::from_center_size(center, size)
}

#[derive(Debug)]
pub enum RowGridLayoutError {
    ComputeEmptyLayout,
    VisitBeforeLayout,
}

impl std::fmt::Display for RowGridLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowGridLayoutError::VisitBeforeLayout => {
                write!(f, "Cannot visit layout before computing it")
            }
            RowGridLayoutError::ComputeEmptyLayout => {
                write!(f, "Cannot compute an empty layout")
            }
        }
    }
}

impl std::error::Error for RowGridLayoutError {}
