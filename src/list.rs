#[derive(Clone)]
pub struct ListView<T> {
    values: Vec<T>,
    offset: usize,
    len: usize,
    max: usize,
}

impl<T> ListView<T> {
    pub fn new(values: impl IntoIterator<Item = T>) -> Self {
        let values: Vec<_> = values.into_iter().collect();
        let max = values.len();
        let offset = 0;
        let len = 16.min(max);
        Self {
            values,
            max,
            offset,
            len,
        }
    }

    pub fn values(&self) -> &[T] {
        self.values.as_slice()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn scroll_to_ix(&mut self, ix: usize) {
        let visible = self.row_indices();

        if ix < visible.start {
            self.set_offset(ix);
        } else if ix + 1 >= visible.end {
            let offset = ix.checked_sub(self.len - 1).unwrap_or_default();
            self.set_offset(offset);
        }
    }

    pub fn offset_visible(&self, ix: usize) -> bool {
        ix >= self.offset && ix < self.offset + self.len
    }

    pub fn row_indices(&self) -> std::ops::Range<usize> {
        let s = self.offset;
        let e = s + self.len;
        s..e
    }

    pub fn visible_rows<'a>(&'a self) -> impl Iterator<Item = &'a T> + 'a {
        debug_assert!(self.offset + self.len <= self.max);
        debug_assert!(self.max == self.values.len());

        let s = self.offset;
        let e = s + self.len;
        self.values[s..e].iter()
    }

    pub fn set_offset(&mut self, mut offset: usize) {
        if offset + self.len > self.max {
            offset -= (offset + self.len) - self.max;
        }

        self.offset = offset;
        debug_assert!(self.offset + self.len <= self.max);
    }

    pub fn scroll(&mut self, delta: isize) {
        let mut offset = self.offset as isize;

        let max_offset = (self.max - self.len) as isize;
        offset = (offset + delta).clamp(0, max_offset);

        self.offset = offset as usize;
        debug_assert!(self.offset + self.len <= self.max);
    }

    pub fn resize(&mut self, new_len: usize) {
        self.len = new_len.min(self.max);
        // set_offset takes care of moving the offset back for the new
        // length if needed
        self.set_offset(self.offset);
    }
}
