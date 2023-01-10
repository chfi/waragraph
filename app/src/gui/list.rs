use std::sync::Arc;

use taffy::{
    error::TaffyError,
    prelude::{Layout, Node},
    style::Dimension,
};

use ultraviolet::Vec2;

use super::FlexLayout;

#[derive(Default)]
pub struct DynamicListLayout<Row, Elem> {
    layout: FlexLayout<Elem>,
    column_count: usize,
    column_widths: Vec<Dimension>,
    column_getters: Vec<Arc<dyn Fn(&Row) -> (Elem, Dimension) + 'static>>,
    // nodes: HashMap<(usize, usize), Node>,
}

impl<Row, Elem> DynamicListLayout<Row, Elem> {
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
    pub fn build_layout<'a>(
        &mut self,
        content_rect: egui::Rect,
        rows: impl IntoIterator<Item = &'a Row>,
    ) -> Result<(), TaffyError>
    where
        Row: 'a,
    {
        let dims = Vec2::new(content_rect.width(), content_rect.height());

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

                let (elem, height) = get(row);

                let height = match height {
                    Dimension::Points(p) => p,
                    _ => 0.0,
                };
                elems.push(elem);

                row_height = row_height.max(height);
            }

            row_elems.push(elems);
            avail_height -= row_height;
        }

        self.layout.clear();

        let row_iter = row_elems.into_iter().map(|row| {
            row.into_iter().enumerate().map(|(col_ix, elem)| {
                let width = self.column_widths[col_ix];
                (elem, width)
            })
        });

        self.layout.fill_with_rows(row_iter)?;

        if let Some(root) = self.layout.root {
            //
        }

        Ok(())
    }

    pub fn visit_layout(
        &self,
        mut f: impl FnMut(Layout, &Elem),
    ) -> Result<(), TaffyError> {
        todo!();
    }
}
