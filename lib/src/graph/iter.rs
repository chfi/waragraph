use std::iter::FusedIterator;

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
}

impl<'index> Iterator for PangenomeNodeIter<'index> {
    type Item = (Node, Bp);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.node_index_range.next()?;
        let node = Node(index as u32);
        let length = self.index.node_length(node);
        Some((node, length))
    }
}


pub struct PangenomeNodePosRangeIter<'index> {
    index: &'index PathIndex,
    node_index_range: std::ops::RangeInclusive<usize>,
    pos_range: std::ops::Range<u64>,
}

impl<'index> PangenomeNodePosRangeIter<'index> {
    pub(super) fn new_pos_range(
        index: &'index PathIndex,
        pos_range: std::ops::Range<u64>,
    ) -> Self {
        let (start, end) = index.pos_range_nodes(pos_range.clone()).into_inner();
        let i_s = start.0 as usize;
        let i_e = end.0 as usize;

        let node_index_range = i_s..=i_e;

        Self {
            index,
            node_index_range,
            pos_range,
        }
    }
}

impl<'index> Iterator for PangenomeNodePosRangeIter<'index> {
    type Item = (Node, Bp);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.node_index_range.next()?;
        let node = Node(index as u32);
        let (offset, length) = self.index.node_offset_length(node);

        let node_start = offset.0;
        let node_end = offset.0 + length.0;

        let vis_start = self.pos_range.start.max(node_start);
        let vis_end = self.pos_range.end.min(node_end);

        let length = vis_end - vis_start;

        let end = vis_end;
        let new_start = end.min(self.pos_range.end);
        self.pos_range = new_start..self.pos_range.end;
        
        Some((node, Bp(length)))
    }
}

impl<'index> FusedIterator for PangenomeNodePosRangeIter<'index> {}

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

    #[test]
    fn pangenome_nodes_pos_range_iter() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();

        let test_range = |start: u64, end: u64| {
            let range = start..end;
            let iter = PangenomeNodePosRangeIter::new_pos_range(&index, range);

            let vis_len = iter.map(|(_, l)| l.0).sum::<u64>();
            assert_eq!(end - start, vis_len);
        };

        test_range(0, 60);
        test_range(40, 100);
    }
}
