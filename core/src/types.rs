#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Node(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct OrientedNode(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Edge {
    pub from: OrientedNode,
    pub to: OrientedNode,
}

impl From<(OrientedNode, OrientedNode)> for Edge {
    fn from((from, to): (OrientedNode, OrientedNode)) -> Self {
        Self { from, to }
    }
}

impl Edge {
    pub fn new(from: OrientedNode, to: OrientedNode) -> Self {
        Self { from, to }
    }

    pub fn endpoints(&self) -> (OrientedNode, OrientedNode) {
        let Edge { from, to } = self;
        match (from.is_reverse(), to.is_reverse()) {
            (false, false) => (from.node_end(), to.node_start()),
            (false, true) => (from.node_end(), to.node_end()),
            (true, false) => (from.node_start(), to.node_start()),
            (true, true) => (from.node_start(), to.node_end()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Bp(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct PathId(pub u32);

impl Node {
    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn as_forward(&self) -> OrientedNode {
        OrientedNode::new(self.0, false)
    }

    #[inline]
    pub fn as_reverse(&self) -> OrientedNode {
        OrientedNode::new(self.0, true)
    }
}

impl From<u32> for Node {
    fn from(u: u32) -> Node {
        Node(u)
    }
}

impl From<Node> for u32 {
    fn from(n: Node) -> u32 {
        n.0
    }
}

impl From<usize> for Node {
    fn from(u: usize) -> Node {
        Node(u as u32)
    }
}

impl From<u32> for OrientedNode {
    fn from(u: u32) -> OrientedNode {
        OrientedNode(u)
    }
}

impl OrientedNode {
    #[inline]
    pub fn new(id: u32, reverse: bool) -> Self {
        OrientedNode((id << 1) | reverse as u32)
    }

    #[inline]
    pub fn node_start(&self) -> OrientedNode {
        let i = self.node().0;
        Self::new(i, true)
    }

    #[inline]
    pub fn node_end(&self) -> OrientedNode {
        let i = self.node().0;
        Self::new(i, false)
    }

    #[inline]
    pub fn node(&self) -> Node {
        Node(self.0 >> 1)
    }

    #[inline]
    pub fn is_reverse(&self) -> bool {
        (self.0 & 1) == 1
    }

    #[inline]
    pub fn flip(self) -> Self {
        Self::new(self.node().0, !self.is_reverse())
    }

    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }
}

impl From<u64> for Bp {
    fn from(u: u64) -> Bp {
        Bp(u)
    }
}

impl From<Bp> for u64 {
    fn from(bp: Bp) -> u64 {
        bp.0
    }
}

impl PathId {
    #[inline]
    pub fn ix(&self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for PathId {
    fn from(u: u32) -> PathId {
        PathId(u)
    }
}

impl From<usize> for PathId {
    fn from(u: usize) -> PathId {
        PathId(u as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const GFA_PATH: &'static str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test/data/",
        "A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa"
    );

    #[test]
    fn node_lengths() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();

        let node_lengths = (0..10)
            .map(|i| index.node_length(Node(i)).0)
            .collect::<Vec<_>>();

        let expected = vec![44, 12, 19, 1, 1, 13, 1, 1, 1, 2];

        assert_eq!(node_lengths, expected);

        let n = index.node_count as u32;
        let node_lengths = ((n - 10)..n)
            .map(|i| index.node_length(Node(i)).0)
            .collect::<Vec<_>>();

        let expected = vec![1, 1, 1, 3, 1, 1, 2, 1, 1, 12];
        assert_eq!(node_lengths, expected);
    }

    #[test]
    fn pangenome_nodes_range() {
        let index = PathIndex::from_gfa(GFA_PATH).unwrap();
        let total_len = index.pangenome_len();

        let pos_range = 44..55;
        let range0 = index.pos_range_nodes(pos_range);

        let mut last_start = (total_len.0 - 12);
        last_start -= 1;

        let pos_range = last_start..total_len.0;
        let range1 = index.pos_range_nodes(pos_range);

        assert_eq!(range0, Node(1)..=Node(1));
        assert_eq!(range1, Node(4964)..=Node(4965));
    }
}
