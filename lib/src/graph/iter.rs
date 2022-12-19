use roaring::RoaringTreemap;

use super::{Node, PathIndex};

/// Iterator over a compact range of nodes in the pangenome (i.e. node ID) order,
/// returning the nodes with their lengths
pub struct PangenomeNodeIter<'index> {
    index: &'index PathIndex,
    node_index_range: std::ops::Range<usize>,
}

impl<'index> PangenomeNodeIter<'index> {
    pub(super) fn new_index_range(
        index: &'index PathIndex,
        node_index_range: std::ops::Range<usize>,
    ) -> Self {
        Self {
            index,
            node_index_range,
        }
    }

    #[inline]
    fn next_impl(&mut self) -> Option<(Node, usize)> {
        let index = self.node_index_range.next()?;
        let node = Node(index as u32);
        let length = self.index.node_length(node);
        Some((node, length))
    }
}

impl<'index> Iterator for PangenomeNodeIter<'index> {
    type Item = (Node, usize);

    fn next(&mut self) -> Option<Self::Item> {
        self.next_impl()
    }
}