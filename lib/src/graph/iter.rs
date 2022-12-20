use std::iter::FusedIterator;

use roaring::RoaringBitmap;

use super::{Bp, Node, PathIndex};

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
        let pos_range = {
            let start = pos_range.start.min(index.pangenome_len().0);
            let end = pos_range.end.min(index.pangenome_len().0);
            start..end
        };

        let (start, end) =
            index.pos_range_nodes(pos_range.clone()).into_inner();
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

        let length = vis_end.checked_sub(vis_start)?;

        let end = vis_end;
        let new_start = end.min(self.pos_range.end);
        self.pos_range = new_start..self.pos_range.end;

        Some((node, Bp(length)))
    }
}

impl<'index> FusedIterator for PangenomeNodePosRangeIter<'index> {}

// TODO: reimplement by stepping through the data slice, using the enumerated
// index with the path set to avoid using the dense node position iterator
pub struct PangenomePathDataPosRangeIter<'index, 'data, T> {
    path_id: usize,
    path_nodes: &'index RoaringBitmap,

    node_iter: PangenomeNodePosRangeIter<'index>,
    data_iter: std::slice::Iter<'data, T>,
}


impl<'index, 'data, T> PangenomePathDataPosRangeIter<'index, 'data, T> {
    pub(super) fn new_pos_range(
        index: &'index PathIndex,
        pos_range: std::ops::Range<u64>,
        path_id: usize,
        data: &'data [T],
    ) -> Self {
        let path_nodes = &index.path_node_sets[path_id];
        assert_eq!(
            data.len(),
            path_nodes.len() as usize,
            "Data vector must contain exactly one value per node in path"
        );

        let node_iter =
            PangenomeNodePosRangeIter::new_pos_range(index, pos_range);
        let (start_id, end_id) =
            node_iter.node_index_range.clone().into_inner();

        let start_ix = path_nodes.rank(start_id as u32) as usize;
        let end_ix = path_nodes.rank(end_id as u32) as usize;
        let start_ix = start_ix.checked_sub(1).unwrap_or_default();

        let data_iter = data[start_ix..end_ix].iter();

        Self {
            path_id,
            path_nodes,

            node_iter,
            data_iter,
        }
    }
}

impl<'index, 'data, T> Iterator
    for PangenomePathDataPosRangeIter<'index, 'data, T>
{
    type Item = ((Node, Bp), &'data T);

    fn next(&mut self) -> Option<Self::Item> {
        let mut node = self.node_iter.next()?;
        while !self.path_nodes.contains(node.0 .0) {
            node = self.node_iter.next()?;
        }
        let data = self.data_iter.next()?;
        Some((node, data))
    }
}

impl<'index, 'data, T> FusedIterator
    for PangenomePathDataPosRangeIter<'index, 'data, T>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::graph::{sampling::PathDepthData, tests::GFA_PATH};

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

        let test_range = |start: u64, end: u64, expected: u64| {
            let range = start..end;
            let iter = PangenomeNodePosRangeIter::new_pos_range(&index, range);

            let vis_len = iter.map(|(_, l)| l.0).sum::<u64>();
            assert_eq!(expected, vis_len);
        };

        test_range(0, 60, 60);
        test_range(40, 100, 60);

        let len = index.pangenome_len().0;

        test_range(len / 2, (len / 2) + 200, 200);

        test_range(len - 100, len, 100);
        test_range(len - 100, len + 100, 100);
    }

    #[test]
    fn pangenome_path_data_iter() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();
        let depth_data = PathDepthData::new(&index);

        let pos_range = 10..60;

        let make_expected = |vals: &[(u32, u64, u32)]| {
            vals.iter()
                .map(|&(n, l, v)| {
                    (
                        (Node::from(n - index.segment_id_range.0), Bp(l)),
                        v as f32,
                    )
                })
                .collect::<Vec<_>>()
        };

        let expected = [
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(2, 12, 1), (3, 4, 1)],
            vec![(1, 34, 1), (2, 12, 2), (3, 4, 3)],
            vec![(1, 34, 1), (2, 12, 2), (3, 4, 3)],
        ]
        .into_iter()
        .map(|s| make_expected(s.as_slice()))
        .collect::<Vec<_>>();

        let mut path_names = index.path_names.iter().collect::<Vec<_>>();
        path_names.sort_by_key(|(_, i)| *i);

        for ((_path_name, path_id), expected) in path_names.iter().zip(expected)
        {
            let data = &depth_data.node_depth_per_path[**path_id];
            let iter = PangenomePathDataPosRangeIter::new_pos_range(
                &index,
                pos_range.clone(),
                **path_id,
                data,
            );

            let result = iter.map(|(a, v)| (a, *v)).collect::<Vec<_>>();

            assert_eq!(expected, result);
        }
    }
}
