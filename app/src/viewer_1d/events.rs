#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathListEvent {
    ScrollUp,
    ScrollDown,
    SetSlotOffset(usize),
    SetSlotCount(usize),
}
