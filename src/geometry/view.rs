use std::ops::{Add, Sub};

use euclid::*;

// use num_traits::{
//     one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive,
// };

use super::ScreenSpace;

#[derive(Debug)]
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

    /*
    pub fn resize_around(&self, p: I, new_len: I) -> Self
    where
        I: ToPrimitive + FromPrimitive,
    {
        let p_f = p.to_f64().unwrap();
        let ol_f = self.len.0.to_f64().unwrap();
        let nl_f = new_len.to_f64().unwrap();

        let new_len = Length::new(new_len);

        dbg!(p_f);

        // let mut new = self.shift_right(p);
        let mut new = *self;
        new.len = new_len;

        if new_len > self.len {
            // "zooming out"
            let fact = nl_f / ol_f;
            let rev_delta = I::from_f64(fact * p_f).unwrap();

            dbg!(fact);
            // dbg!(rev_delta);

            new.shift_left(rev_delta)
        } else if new_len < self.len {
            // "zooming in"
            let fact = ol_f / nl_f;
            let rev_delta = I::from_f64(fact * p_f).unwrap();

            dbg!(fact);
            // dbg!(rev_delta);

            new.shift_left(rev_delta)
        } else {
            *self
        }
    }
    */

    /*
    /// Returns a new `View1D<I>` by resizing this view while keeping the right-hand side fixed.
    pub fn resize_from_right(&self, new_len: I) -> Self {
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_zoom() -> anyhow::Result<()> {
        // let mut view: PangenomeView = View1D::new(1_000_000);
        let view: PangenomeView = View1D::new(10_000);

        // let zoomed = view.resize_around(5_000, 5_000);
        let zoomed = view.resize_from_left(5_000);

        eprintln!("original: {:?}", view);
        eprintln!("zoomed:   {:?}", zoomed);

        assert!(false);

        Ok(())
    }
}
