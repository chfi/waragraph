use ahash::HashSet;
use arrow2::{
    array::PrimitiveArray,
    datatypes::{DataType, Field},
    offset::OffsetsBuffer,
};

use crate::arrow_graph::ArrowGFA;

pub struct PathOffsets {
    step_offsets: PrimitiveArray<u32>,
}

impl PathOffsets {
    pub fn offsets_array(&self) -> &PrimitiveArray<u32> {
        &self.step_offsets
    }

    pub fn from_arrow_gfa_path(graph: &ArrowGFA, path: u32) -> Self {
        let steps = graph.path_steps(path);

        let mut offsets: Vec<u32> = Vec::new();
        let mut offset = 0;
        for step in steps.values_iter() {
            offsets.push(offset);
            let len = graph.segment_len(step >> 1);
            offset += len as u32;
        }

        offsets.push(offset);

        Self {
            step_offsets: PrimitiveArray::from_vec(offsets),
        }
    }

    pub fn step_bp_range(
        &self,
        step_index: u32,
    ) -> Option<std::ops::Range<u64>> {
        let offset = self.step_offsets.get(step_index as usize)?;
        let next = self.step_offsets.get(step_index as usize + 1)?;
        Some(offset as u64..next as u64)
    }

    pub fn bp_to_step_range(
        &self,
        range: std::ops::Range<u64>,
    ) -> Option<std::ops::Range<u32>> {
        let start = self.step_at(range.start)?;
        // TODO might need some edge case handling
        let end = self.step_at(range.end)?;
        Some(start..end)
    }

    pub fn step_and_local_offset_at(
        &self,
        position: u64,
    ) -> Option<(u32, u64)> {
        let step_ix = self.step_at(position)?;
        let step_offset = self.step_offsets.get(step_ix as usize)? as u64;
        let local_offset = position - step_offset;
        Some((step_ix, local_offset))
    }

    pub fn step_at(&self, position: u64) -> Option<u32> {
        let ix = self
            .step_offsets
            .values()
            .partition_point(|&offset| offset < position as u32);
        if ix < self.step_offsets.len() - 1 {
            Some(ix as u32)
        } else {
            None
        }
    }

    pub fn step_count(&self) -> usize {
        self.step_offsets.len() - 1
    }
}

#[derive(Debug, Clone)]
pub struct CoordSys {
    pub node_order: PrimitiveArray<u32>,
    // TODO offsets should probably be i64; maybe generic

    // TODO should also not use offsetsbuffer, so that the size matches node_order;
    // will need changing bp_to_step range etc.
    pub step_offsets: OffsetsBuffer<i32>,
    // step_offsets: PrimitiveArray<u32>,
}

impl CoordSys {
    pub fn arrow_schema() -> arrow2::datatypes::Schema {
        arrow2::datatypes::Schema::from(vec![
            Field::new("node_order", DataType::UInt32, false),
            Field::new("step_offsets", DataType::Int32, false),
        ])
    }

    pub fn segment_range(&self, segment: u32) -> Option<std::ops::Range<u64>> {
        let ix = segment as usize;

        if ix >= self.step_offsets.len_proxy() {
            return None;
        }

        let (start, end) = self.step_offsets.start_end(ix);

        Some((start as u64)..(end as u64))
    }

    pub fn segment_at_pos(&self, pos: u64) -> u32 {
        let i = self
            .step_offsets
            .buffer()
            .binary_search_by_key(&pos, |&o| o as u64);
        i.unwrap_or_else(|i| i - 1) as u32
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
            if size == 0.0 {
                *value = 0.0;
            } else {
                *value = *value / size;
            }
        }
    }

    pub fn global_from_arrow_gfa(graph: &ArrowGFA) -> Self {
        let node_count = graph.segment_count();

        let node_order = arrow2::array::UInt32Array::from_iter(
            (0..node_count as u32).map(Some),
        );

        let step_offsets = graph.segment_sequences_array().offsets().clone();

        Self {
            node_order,
            step_offsets,
        }
    }

    pub fn path_from_arrow_gfa(graph: &ArrowGFA, path_index: u32) -> Self {
        // let mut seen_nodes = HashSet::default();

        let steps = &graph.path_steps(path_index);

        let mut node_order = Vec::with_capacity(steps.len());
        let mut step_offsets = Vec::with_capacity(steps.len());

        let mut offset = 0i32;

        for &handle in steps.values_iter() {
            let node = handle >> 1;
            let seg_size = graph.segment_len(node);

            // if !seen_nodes.contains(&node) {
            node_order.push(node);
            // seen_nodes.insert(node);

            step_offsets.push(offset);
            offset += seg_size as i32;
            // }
        }
        step_offsets.push(offset);

        node_order.shrink_to_fit();
        step_offsets.shrink_to_fit();

        let node_order = arrow2::array::UInt32Array::from_vec(node_order);
        let step_offsets = OffsetsBuffer::try_from(step_offsets).unwrap();

        Self {
            node_order,
            step_offsets,
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
