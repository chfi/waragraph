use super::{Node, PathIndex, Bp};

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
    fn next_impl(&mut self) -> Option<(Node, Bp)> {
        let index = self.node_index_range.next()?;
        let node = Node(index as u32);
        let length = self.index.node_length(node);
        Some((node, length))
    }
}

impl<'index> Iterator for PangenomeNodeIter<'index> {
    type Item = (Node, Bp);

    fn next(&mut self) -> Option<Self::Item> {
        self.next_impl()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use crate::graph::tests::GFA_PATH;

    #[test]
    fn pangenome_nodes_range_iter() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();

        let iter = PangenomeNodeIter::new_index_range(&index, 0..10);

        let expected = [44, 12, 19, 1, 1, 13, 1, 1, 1, 2]
            .into_iter()
            .enumerate()
            .map(|(i, v)| (Node(i as u32), Bp(v)));

        assert!(iter.eq(expected));
    }
}
