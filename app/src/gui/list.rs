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

    pub fn build_layout<'a>(
        &mut self,
        dims: Vec2,
        rows: impl IntoIterator<Item = &'a Row>,
    ) -> Result<(), TaffyError>
    where
        Row: 'a,
    {
        todo!();
    }

    pub fn visit_layout(
        &self,
        mut f: impl FnMut(Layout, &Elem),
    ) -> Result<(), TaffyError> {
        todo!();
    }
}
