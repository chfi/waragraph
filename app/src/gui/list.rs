use std::sync::Arc;

use taffy::{
    error::TaffyError,
    prelude::{Layout, Node},
    style::Dimension,
};

use ultraviolet::Vec2;

use super::FlexLayout;

pub struct DynamicListLayout<Row, Elem> {
    layout: FlexLayout<Elem>,
    column_count: usize,
    column_widths: Vec<Dimension>,
    column_getters: Vec<Arc<dyn Fn(&Row) -> (Elem, Dimension) + 'static>>,
}

impl<Row, Elem> std::default::Default for DynamicListLayout<Row, Elem> {
    fn default() -> Self {
        Self {
            layout: FlexLayout::default(),
            column_count: 0,
            column_widths: Vec::new(),
            column_getters: Vec::new(),
        }
    }
}

impl<Row, Elem> DynamicListLayout<Row, Elem> {
    pub fn layout(&self) -> &FlexLayout<Elem> {
        &self.layout
    }

    pub fn clear_layout(&mut self) {
        self.layout.clear();
    }

    pub fn column_widths(&self) -> &[Dimension] {
        self.column_widths.as_slice()
    }

    pub fn column_widths_mut(&mut self) -> &mut [Dimension] {
        self.column_widths.as_mut_slice()
    }

    pub fn push_column(
        &mut self,
        width: Dimension,
        getter: impl Fn(&Row) -> (Elem, Dimension) + 'static,
    ) {
        self.column_widths.push(width);
        self.column_getters.push(Arc::new(getter));
        self.column_count += 1;
    }

    pub fn with_column(
        mut self,
        width: Dimension,
        getter: impl Fn(&Row) -> (Elem, Dimension) + 'static,
    ) -> Self {
        self.push_column(width, getter);
        self
    }

    /// Returns the number of rows processed/laid out
    pub fn build_layout(
        &mut self,
        offset: Vec2,
        dims: Vec2,
        rows: impl IntoIterator<Item = Row>,
    ) -> Result<(), TaffyError> {
        let mut avail_height = dims.y;

        let mut row_elems = Vec::new();

        for row in rows {
            if avail_height < 0.0 {
                break;
            }

            let mut row_height = 0f32;

            let mut elems = Vec::with_capacity(self.column_count);

            for col_ix in 0..self.column_count {
                let get = &self.column_getters[col_ix];

                let (elem, height) = get(&row);
                elems.push((elem, height));

                let height = match height {
                    Dimension::Points(p) => p,
                    _ => 0.0,
                };

                row_height = row_height.max(height);
            }

            row_elems.push(elems);
            avail_height -= row_height;
        }

        self.layout.clear();

        let row_iter = row_elems.into_iter().map(|row| {
            row.into_iter().enumerate().map(|(col_ix, (elem, height))| {
                let width = self.column_widths[col_ix];
                (elem, width, height)
            })
        });

        self.layout.fill_with_rows(row_iter)?;
        self.layout.compute_layout(offset, dims)?;

        Ok(())
    }

    pub fn visit_layout(
        &self,
        f: impl FnMut(Layout, &Elem),
    ) -> Result<(), TaffyError> {
        self.layout.visit_layout(f)
    }
}
