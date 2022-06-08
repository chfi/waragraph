use std::ops::{Add, Sub};

use euclid::*;
use num_traits::{FromPrimitive, ToPrimitive};

// use num_traits::{
//     one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive,
// };

use super::ScreenSpace;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PangenomeSpace;

pub type PangenomeScreenScale<T> = Scale<T, PangenomeSpace, ScreenSpace>;

pub type PangenomeView = View1D<usize, PangenomeSpace>;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct View1D<I, U = UnknownUnit>
where
    I: Clone + PartialOrd,
{
    max: Length<I, U>,

    offset: Length<I, U>,
    len: Length<I, U>,
}

impl View1D<usize, PangenomeSpace> {
    /// Returns the scaling factor for the provided pixel width
    pub fn screen_scale(&self, width: usize) -> PangenomeScreenScale<f32> {
        let scale = width as f32 / self.len.0 as f32;
        Scale::new(scale)
    }
}

impl<I: Clone + PartialOrd, U> Clone for View1D<I, U> {
    fn clone(&self) -> Self {
        Self {
            max: self.max.clone(),
            offset: self.offset.clone(),
            len: self.len.clone(),
        }
    }
}

impl<I: Copy + PartialOrd, U> Copy for View1D<I, U> {}

impl<I, U> View1D<I, U>
where
    I: Copy + PartialEq + PartialOrd + Add<Output = I> + num_traits::Zero,
{
    pub fn new(max: I) -> Self {
        let max = Length::new(max);
        Self {
            max,
            offset: Length::new(I::zero()),
            len: max,
        }
    }

    pub fn new_with(max: I, offset: I, len: I) -> Option<Self>
    where
        I: Ord,
    {
        if offset < I::zero() || offset > max || offset + len > max {
            return None;
        }

        Some(Self {
            max: Length::new(max),
            offset: Length::new(offset),
            len: Length::new(len),
        })
    }

    pub fn offset(&self) -> Length<I, U> {
        self.offset
    }

    pub fn len(&self) -> Length<I, U> {
        self.len
    }

    pub fn max(&self) -> Length<I, U> {
        self.max
    }

    pub fn is_valid(&self) -> bool {
        self.len.0 > euclid::num::Zero::zero()
            && (self.offset + self.len <= self.max)
    }

    pub fn reset(&mut self) {
        self.offset = euclid::num::Zero::zero();
        self.len = self.max;
    }

    pub fn set(&mut self, offset: I, len: I) {
        let o = Length::new(offset);
        let l = Length::new(len);
        assert!(len > I::zero());
        assert!(o + l <= self.max);
        self.offset = o;
        self.len = l;
    }
}

//

impl<I, U> View1D<I, U>
where
    I: Copy
        + PartialEq
        + PartialOrd
        + Add<Output = I>
        + Sub<Output = I>
        + num_traits::Zero,
{
    pub fn set_offset(&self, new_offset: I) -> Self {
        let mut new = self.to_owned();

        let new_offset = Length::new(new_offset);

        if new_offset + new.len >= new.max {
            new.offset = new.max - new.len;
        } else {
            new.offset = new_offset;
        }

        new
    }

    pub fn shift_right(&self, delta: I) -> Self {
        let mut new = *self;

        let delta = Length::new(delta);

        if delta + self.offset + self.len >= self.max {
            new.offset = self.max - self.len;
        } else {
            new.offset = self.offset + delta;
        }

        new
    }

    pub fn shift_left(&self, delta: I) -> Self {
        let mut new = *self;

        let delta = Length::new(delta);

        if delta >= self.offset {
            new.offset = euclid::num::Zero::zero();
        } else {
            new.offset = self.offset - delta;
        }

        new
    }

    /// Returns a new `View1D` with the same offset but a new length.
    pub fn resize_from_left(&self, new_len: I) -> Self {
        let new_len = Length::new(new_len);
        if self.offset + new_len >= self.max {
            Self {
                len: self.max - self.offset,
                ..*self
            }
        } else {
            Self {
                len: new_len,
                ..*self
            }
        }
    }

    pub fn resize_from_right(&self, new_len: I) -> Self {
        if self.len.0 >= new_len {
            let diff = self.len.0 - new_len;
            let new = self.resize_from_left(new_len);
            new.shift_right(diff)
        } else {
            let diff = new_len - self.len.0;
            let new = self.resize_from_left(new_len);
            new.shift_left(diff)
        }
    }

    pub fn resize_around(&self, around: I, new_len: I) -> Self
    where
        I: ToPrimitive + FromPrimitive + Ord,
    {
        let around = around.clamp(self.offset.0, self.offset.0 + self.len.0);
        let t = (around - self.offset.0).to_f64().unwrap()
            / self.len.0.to_f64().unwrap();

        let new = if self.len.0 >= new_len {
            let diff = t * (self.len.0 - new_len).to_f64().unwrap();
            let new = self.resize_from_left(new_len);
            new.shift_right(I::from_f64(diff).unwrap())
        } else {
            let diff = t * (new_len - self.len.0).to_f64().unwrap();
            let new = self.resize_from_left(new_len);
            new.shift_left(I::from_f64(diff).unwrap())
        };

        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_zoom() -> anyhow::Result<()> {
        let view: PangenomeView = View1D::new(10_000);

        let zoomed0 = view.resize_from_left(5_000);

        let new_view = |offset: usize, len: usize| {
            View1D::new_with(10_000, offset, len).unwrap()
        };

        assert_eq!(zoomed0, new_view(0, 5_000));

        let zoomed1 = view.resize_from_right(5_000);

        assert_eq!(zoomed1, new_view(5_000, 5_000));

        let zoomed_out = zoomed0.resize_from_right(6_000);

        assert_eq!(zoomed_out, new_view(0, 6_000));

        let translated = zoomed0.shift_right(3_000);

        assert_eq!(translated, new_view(3_000, 5_000));

        let t_zoom = translated.resize_from_right(6_000);
        let t_zoom2 = t_zoom.resize_from_left(6_500);

        assert_eq!(t_zoom, new_view(2_000, 6_000));
        assert_eq!(t_zoom2, new_view(2_000, 6_500));

        // eprintln!("original: {:?}", view);
        // eprintln!("zoomed0:   {:?}", zoomed0);
        // eprintln!("zoomed1:   {:?}", zoomed1);
        // eprintln!("zoomed_out:   {:?}", zoomed_out);
        // eprintln!("translated:   {:?}", translated);
        // eprintln!("t_zoom:   {:?}", t_zoom);
        // eprintln!("t_zoom2:   {:?}", t_zoom2);

        let t_zoom3 = translated.resize_around(4_000, 6_000);
        let t_zoom4 = t_zoom3.resize_around(3_000, 3_000);
        let t_zoom5 = t_zoom4.resize_around(3_000, 3_000);

        assert_eq!(t_zoom4, t_zoom5);

        let t_zoom5 = t_zoom4.resize_around(4_000, 3_300);
        let t_zoom6 = t_zoom5.resize_around(4_000, 3_000);

        // eprintln!("t_zoom3:   {:?}", t_zoom3);
        // eprintln!("t_zoom4:   {:?}", t_zoom4);
        // eprintln!("t_zoom5:   {:?}", t_zoom5);
        // eprintln!("t_zoom6:   {:?}", t_zoom6);

        assert_eq!(t_zoom4, t_zoom6);

        assert_eq!(t_zoom3, new_view(2_800, 6_000));
        assert_eq!(t_zoom4, new_view(2_900, 3_000));

        assert_eq!(t_zoom5, new_view(2_791, 3_300));
        assert_eq!(t_zoom6, new_view(2_900, 3_000));

        Ok(())
    }
}
