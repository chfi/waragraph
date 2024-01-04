use arrow2::{array::PrimitiveArray, offset::OffsetsBuffer};

// #[cfg(target = "wasm32")]
// #[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct CoordSys {
    pub node_order: PrimitiveArray<u32>,
    // TODO offsets should probably be i64; maybe generic
    pub step_offsets: OffsetsBuffer<i32>,
    // step_offsets: PrimitiveArray<u32>,
}

impl CoordSys {
    pub fn segment_range(&self, segment: u32) -> Option<std::ops::Range<u64>> {
        let ix = segment as usize;

        if ix >= self.step_offsets.len() {
            return None;
        }

        let (start, end) = self.step_offsets.start_end(ix);

        Some((start as u64)..(end as u64))
    }

    pub fn bp_to_step_range(
        &self,
        start: u64,
        end: u64,
    ) -> std::ops::RangeInclusive<usize> {
        let start_i = self
            .step_offsets
            .buffer()
            .binary_search_by_key(&start, |&o| o as u64);
        let end_i = self
            .step_offsets
            .buffer()
            .binary_search_by_key(&end, |&o| o as u64);

        let start_out = start_i.unwrap_or_else(|i| i - 1);
        let end_out = end_i.unwrap_or_else(|i| i);

        start_out..=end_out
    }

    pub fn sample_impl(
        &self,
        bp_range: std::ops::RangeInclusive<u64>,
        data_indices: &[u32],
        data: &[f32],
        bins: &mut [f32],
    ) {
        // find range in step index using bp_range
        let indices = self.bp_to_step_range(*bp_range.start(), *bp_range.end());

        // slice `data` according to step range
        let s_i = *indices.start();
        let e_i = *indices.end();

        // `indices` is inclusive
        let _len = (e_i + 1) - s_i;
        // let data_slice = data.sliced(s_i, len);

        let bp_range_len = (*bp_range.end() + 1) - *bp_range.start();
        let _bin_size = bp_range_len / bins.len() as u64;

        let make_bin_range_f = {
            let bin_size = (bp_range_len as f64) / bins.len() as f64;
            let bin_count = bins.len();
            let s = *bp_range.start() as f64;
            let _e = (*bp_range.end() + 1) as f64;

            move |bin_i: usize| -> std::ops::Range<f64> {
                let i = (bin_i.min(bin_count - 1)) as f64;
                let left = s + i * bin_size as f64;
                let right = s + (i + 1.0) * bin_size as f64;
                left..right
            }
        };

        let _bin_count = bins.len();

        let mut data_iter = CoordSysDataIter::new(
            &self,
            data_indices,
            data,
            (s_i as u32)..(e_i as u32 - 1),
        );

        // clear bins
        bins.iter_mut().for_each(|bin| *bin = 0.0);
        let mut bin_sizes = vec![0.0; bins.len()];
        let mut bins_iter = bins.iter_mut().enumerate();

        let mut current_data = if let Some(data) = data_iter.next() {
            data
        } else {
            return;
        };
        let mut current_bin = bins_iter.next().unwrap();

        let mut cur_bin_range = make_bin_range_f(current_bin.0);

        loop {
            let data_left = current_data.bp_start as f64;
            let data_right = current_data.bp_end as f64;

            let _bin_i = current_bin.0;

            loop {
                let _bin_left = cur_bin_range.start;
                let bin_right = cur_bin_range.end;
                if data_left >= bin_right {
                    // increment bin
                    if let Some(next_bin) = bins_iter.next() {
                        current_bin = next_bin;
                        cur_bin_range = make_bin_range_f(current_bin.0);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            let bin_i = current_bin.0;
            let bin_left = cur_bin_range.start;
            let bin_right = cur_bin_range.end;
            // add data to bin
            {
                let start = data_left.max(bin_left);
                let end = data_right.min(bin_right);

                let overlap = end - start;

                if overlap > 0.0 {
                    bin_sizes[bin_i] += overlap as f32;
                    *current_bin.1 += current_data.value * overlap as f32;
                }
            }

            if data_right <= bin_right {
                // if the current data ends in the current bin, increment the data iterator
                if let Some(next_data) = data_iter.next() {
                    current_data = next_data;
                } else {
                    break;
                }
            } else {
                // if the current data ends beyond the current bin, increment the bin iterator
                if let Some(next_bin) = bins_iter.next() {
                    current_bin = next_bin;
                    cur_bin_range = make_bin_range_f(current_bin.0);
                } else {
                    break;
                }
            }
        }

        for (_i, (value, size)) in std::iter::zip(bins, bin_sizes).enumerate() {
            *value = *value / size;
        }
    }
}

pub struct CoordSysDataIter<'a, 'b> {
    coord_sys: &'a CoordSys,
    data_indices: &'b [u32],
    data_values: &'b [f32],
    // bp_range:
}

impl<'a, 'b> CoordSysDataIter<'a, 'b> {
    fn new(
        coord_sys: &'a CoordSys,
        data_indices: &'b [u32],
        data_values: &'b [f32],
        segment_range: std::ops::Range<u32>,
    ) -> Self {
        // find the indices in `data_indices` that correspond to the `segment_range`
        let data_ix_start = data_indices
            .binary_search(&segment_range.start)
            .unwrap_or_else(|i| if i == 0 { 0 } else { i - 1 })
            as usize;

        let data_ix_end = data_indices
            .binary_search(&segment_range.end)
            .unwrap_or_else(|i| if i == 0 { 0 } else { i - 1 })
            as usize;

        let data_ix_end = (data_ix_end + 1).max(data_indices.len());

        let data_indices = &data_indices[data_ix_start..data_ix_end];
        let data_values = &data_values[data_ix_start..data_ix_end];

        Self {
            coord_sys,
            data_indices,
            data_values,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoordSysDataIterOutput<T> {
    segment: u32,
    bp_start: u64,
    bp_end: u64,
    value: T,
}

impl<'a, 'b> Iterator for CoordSysDataIter<'a, 'b> {
    type Item = CoordSysDataIterOutput<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data_indices.is_empty() {
            return None;
        }

        let segment = self.data_indices[0];
        let value = self.data_values[0];

        let range = self.coord_sys.segment_range(segment)?;

        self.data_indices = &self.data_indices[1..];
        self.data_values = &self.data_values[1..];

        Some(CoordSysDataIterOutput {
            segment,
            bp_start: range.start,
            bp_end: range.end,
            value,
        })
    }
}
