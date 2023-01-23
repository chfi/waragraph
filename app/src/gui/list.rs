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

    column_getter:
        Arc<dyn Fn(&Row, usize) -> Option<(Elem, Dimension)> + 'static>,
}

impl<Row, Elem> DynamicListLayout<Row, Elem> {
    pub fn new(
        column_widths: impl IntoIterator<Item = Dimension>,
        column_getter: impl Fn(&Row, usize) -> Option<(Elem, Dimension)> + 'static,
    ) -> Self {
        let column_widths = column_widths.into_iter().collect::<Vec<_>>();
        let column_count = column_widths.len();

        Self {
            layout: FlexLayout::default(),
            column_count,
            column_widths,
            column_getter: Arc::new(column_getter),
        }
    }

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
                if let Some((elem, height)) = (self.column_getter)(&row, col_ix)
                {
                    elems.push((elem, height));

                    let height = match height {
                        Dimension::Points(p) => p,
                        _ => 0.0,
                    };

                    row_height = row_height.max(height);
                }
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

        Ok(avail_height)
    }

    pub fn visit_layout(
        &self,
        f: impl FnMut(Layout, &Elem),
    ) -> Result<(), TaffyError> {
        self.layout.visit_layout(f)
    }
}
