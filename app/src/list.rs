pub struct ListView<T> {
    values: Vec<(usize, T)>,
    offset: usize,
    view_len: usize,
}

impl<T> ListView<T> {
    pub fn new(
        values: impl IntoIterator<Item = T>,
        len: Option<usize>,
    ) -> Self {
        let values: Vec<_> = values.into_iter().enumerate().collect();
        let list_len = values.len();
        let offset = 0;
        let view_len = len.unwrap_or(list_len).min(list_len);

        Self {
            values,
            offset,
            view_len,
        }
    }

    pub fn as_slice(&self) -> &[(usize, T)] {
        &self.values
    }

    pub fn get_in_view(&self, local_offset: usize) -> Option<&T> {
        if local_offset >= self.view_len {
            return None;
        }

        let (_rank, val) = self.values.get(self.offset + local_offset)?;
        Some(val)
    }

    pub fn visible_indices(&self) -> std::ops::Range<usize> {
        let start = self.offset;
        let end = start + self.view_len;
        start..end
    }

    pub fn visible_iter(&self) -> impl Iterator<Item = &T> {
        self.values[self.visible_indices()].iter().map(|(_, v)| v)
    }

    pub fn offset_to_end_iter(&self) -> impl Iterator<Item = &T> {
        self.values[self.offset..].iter().map(|(_, v)| v)
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn view_len(&self) -> usize {
        self.view_len
    }

    pub fn max_len(&self) -> usize {
        self.values.len()
    }

    pub fn scroll_relative_filtered(
        &mut self,
        delta: isize,
        include: impl Fn(&T) -> bool,
    ) {
        if delta == 0 {
            return;
        }
        let mut real_delta = 0isize;
        let mut remaining = delta.abs() as usize;

        if delta > 0 {
            for (_, val) in self.values[self.offset..].iter().skip(1) {
                if remaining == 0 {
                    break;
                }

                if include(val) {
                    remaining -= 1;
                }
                real_delta += 1;
            }
        } else if delta < 0 {
            for (_, val) in self.values[..self.offset].iter().rev() {
                if remaining == 0 {
                    break;
                }

                if include(val) {
                    remaining -= 1;
                }
                real_delta -= 1;
            }
        }

        self.scroll_relative(real_delta);
    }

    pub fn scroll_relative(&mut self, delta: isize) {
        let mut offset = self.offset as isize;

        let max_offset = (self.max_len() - self.view_len) as isize;
        offset = (offset + delta).clamp(0, max_offset);

        self.offset = offset as usize;
        debug_assert!(self.offset + self.view_len <= self.max_len());
    }

    pub fn scroll_absolute(&mut self, offset: usize) {
        let max_offset = self.max_len() - self.view_len;
        self.offset = offset.min(max_offset);
        debug_assert!(self.offset + self.view_len <= self.max_len());
    }

    pub fn resize(&mut self, new_view_len: usize) {
        let max_len = self.max_len() - self.offset;
        self.view_len = new_view_len.min(max_len);
        debug_assert!(self.offset + self.view_len <= self.max_len());
    }
}
