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
    pub taffy: Taffy,
    pub node_data: BTreeMap<Node, T>,
    pub root: Option<Node>,

    root_style: Style,
    row_base_style: Style,
    // row_templates: HashMap<String, RowTemplate>,

    // in logical pixels
    computed_for_rect: Option<egui::Rect>,
}

pub struct RowEntry<T> {
    pub desired_height: Option<f32>,
    pub grid_template_columns: Vec<TrackSizingFunction>,
    pub grid_template_rows: Vec<TrackSizingFunction>,
    pub column_data: Vec<GridEntry<T>>,
}

impl<T> RowEntry<T> {
    pub fn apply_style(&self, style: Style) -> Style {
        let mut style = Style {
            grid_template_columns: self.grid_template_columns.clone(),
            grid_template_rows: self.grid_template_rows.clone(),
            ..style
        };

        if let Some(height) = self.desired_height {
            style.flex_basis = points(height);
        }

        style
    }
}

impl<T> std::default::Default for RowEntry<T> {
    fn default() -> Self {
        Self {
            desired_height: None,
            grid_template_columns: vec![fr(1.0)],
            grid_template_rows: vec![fr(1.0)],
            column_data: Vec::new(),
        }
    }
}

pub struct GridEntry<T> {
    style: Style,
    pub data: T,
}

impl<T> GridEntry<T> {
    /// Specify a specific grid coordinate with `[x, y]`, for nonzero
    /// `x`, `y`, or use `0` to use a single span, and have layout
    /// order depend on the data order.
    pub fn new(
        grid_pos: [i16; 2],
        // grid_row: Option<i16>,
        // grid_column: Option<i16>,
        data: T,
    ) -> Self {
        let to_prop = |val| if val != 0 { line(val) } else { span(1) };
        let style = Style {
            size: Size {
                width: Dimension::Auto,
                height: percent(1.0),
            },
            grid_row: to_prop(grid_pos[0]),
            grid_column: to_prop(grid_pos[1]),
            ..Default::default()
        };

        Self { style, data }
    }

    pub fn auto(data: T) -> Self {
        Self::new([0, 0], data)
    }

    pub fn columns(
        iter: impl IntoIterator<Item = T>,
    ) -> impl Iterator<Item = GridEntry<T>> {
        iter.into_iter()
            .enumerate()
            .map(|(i, data)| GridEntry::new([1, i as i16], data))
    }
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
        let base_row_height = 20.0; // pixels

        let root_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..Default::default()
        };

        // TODO support custom styles
        let row_base_style = Style {
            display: Display::Grid,

            flex_basis: points(base_row_height),
            flex_shrink: 0.0, // probably not the way to go about this
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

        let sized_root_style = Style {
            size: Size {
                width: points(rect.width()),
                height: points(rect.height()),
            },
            ..self.root_style.clone()
        };
        self.taffy.set_style(root, sized_root_style)?;

        let container_space = Size {
            width: AvailableSpace::from_points(rect.width()),
            height: AvailableSpace::MaxContent,
            // height: AvailableSpace::from_points(rect.height()),
        };
        self.taffy.compute_layout(root, container_space)?;

        self.computed_for_rect = Some(rect);

        Ok(())
    }

    pub fn build_layout_for_rows<Rows>(
        &mut self,
        rows: Rows,
    ) -> Result<(), TaffyError>
    where
        Rows: IntoIterator<Item = RowEntry<T>>,
    {
        // create children
        let mut children = Vec::new();

        for row_entry in rows.into_iter() {
            // create inner columns
            let mut row_children = Vec::new();

            let row_style = row_entry.apply_style(self.row_base_style.clone());

            for grid_entry in row_entry.column_data {
                let mut style = grid_entry.style;

                let node = self.taffy.new_leaf(style)?;
                self.node_data.insert(node, grid_entry.data);
                row_children.push(node);
            }

            let row = self.taffy.new_with_children(row_style, &row_children)?;

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

#[cfg(test)]
mod tests {

    use super::*;

    use anyhow::Result;

    #[test]
    fn multi_row_grid_layout() -> Result<()> {
        use taffy::prelude::*;

        let mut layout: RowGridLayout<usize> = RowGridLayout::new();

        let rows = (0..8).flat_map(|u| {
            if u == 3 {
                let column_data = vec![
                    GridEntry::auto(u * 57),
                    GridEntry::auto(1 + u * 57),
                    GridEntry::auto(2 + u * 57),
                    GridEntry::auto(3 + u * 57),
                ];

                let mut entry = RowEntry {
                    grid_template_columns: vec![points(300.0), fr(1.0)],
                    grid_template_rows: vec![points(100.0), points(70.0)],
                    column_data,
                    ..RowEntry::default()
                };

                vec![entry]
            } else {
                let column_data =
                    vec![GridEntry::auto(u * 2), GridEntry::auto(1 + u * 2)];
                let mut entry = RowEntry {
                    grid_template_columns: vec![points(100.0), fr(1.0)],
                    column_data,
                    ..RowEntry::default()
                };
                if u % 2 == 0 {
                    let u2 = u / 2;
                    // entry.desired_height = Some(30.0 + 10.0 * u2 as f32);
                }

                vec![entry]
            }
        });

        layout.build_layout_for_rows(rows)?;

        let screen_rect =
            egui::Rect::from_x_y_ranges(100.0..=900.0, 200.0..=700.0);

        layout.compute_layout(screen_rect)?;

        layout.visit_layout(|layout, val| {
            let location = layout.location;
            let size = layout.size;

            println!("{val} - {location:?} \t {size:?}");
        })?;

        taffy::debug::print_tree(&layout.taffy, layout.root.unwrap());

        Ok(())
    }

    #[test]
    fn row_grid_layout() -> Result<()> {
        use taffy::prelude::*;

        let mut layout: RowGridLayout<usize> = RowGridLayout::new();

        let rows = (0..8).map(|u| {
            let column_data =
                vec![GridEntry::auto(u * 2), GridEntry::auto(1 + u * 2)];

            let mut entry = RowEntry {
                grid_template_columns: vec![points(100.0), fr(1.0)],
                column_data,
                ..RowEntry::default()
            };
            if u % 2 == 0 {
                let u2 = u / 2;
                entry.desired_height = Some(30.0 + 10.0 * u2 as f32);
            }
            entry
        });

        layout.build_layout_for_rows(rows)?;

        let screen_rect =
            egui::Rect::from_x_y_ranges(100.0..=900.0, 200.0..=700.0);

        layout.compute_layout(screen_rect)?;

        let expected_results = [
            (15, [200.0, 440.0], [700.0, 20.0]),
            (14, [100.0, 440.0], [100.0, 20.0]),
            (13, [200.0, 380.0], [700.0, 60.0]),
            (12, [100.0, 380.0], [100.0, 60.0]),
            (11, [200.0, 360.0], [700.0, 20.0]),
            (10, [100.0, 360.0], [100.0, 20.0]),
            (9, [200.0, 310.0], [700.0, 50.0]),
            (8, [100.0, 310.0], [100.0, 50.0]),
            (7, [200.0, 290.0], [700.0, 20.0]),
            (6, [100.0, 290.0], [100.0, 20.0]),
            (5, [200.0, 250.0], [700.0, 40.0]),
            (4, [100.0, 250.0], [100.0, 40.0]),
            (3, [200.0, 230.0], [700.0, 20.0]),
            (2, [100.0, 230.0], [100.0, 20.0]),
            (1, [200.0, 200.0], [700.0, 30.0]),
            (0, [100.0, 200.0], [100.0, 30.0]),
        ]
        .into_iter()
        .map(|(val, [x, y], [width, height])| {
            let pos = taffy::geometry::Point { x, y };
            let size = Size { width, height };
            (val, (pos, size))
        })
        .collect::<HashMap<_, _>>();

        layout.visit_layout(|layout, val| {
            let location = layout.location;
            let size = layout.size;

            let expected = expected_results.get(val).unwrap();

            assert_eq!(location, expected.0);
            assert_eq!(size, expected.1);
            println!("{val} - {location:?} \t {size:?}");
        })?;

        taffy::debug::print_tree(&layout.taffy, layout.root.unwrap());

        Ok(())
    }
}
