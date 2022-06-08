use rhai::plugin::*;

use euclid::*;

// pub enum LayoutInput {
//     ScalarInt(i32),
//     ScalarUInt(u32),
//     ScalarFloat(f32),
// }

pub mod view;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScreenSpace;

pub type ScreenLen = Length<f32, ScreenSpace>;
pub type ScreenPoint = Point2D<f32, ScreenSpace>;
pub type ScreenVector = Vector2D<f32, ScreenSpace>;
pub type ScreenSize = Size2D<f32, ScreenSpace>;
pub type ScreenRect = Rect<f32, ScreenSpace>;
pub type ScreenBox2D = Box2D<f32, ScreenSpace>;

/// Basically a helper trait for adding methods to Rect, for now
pub trait LayoutElement: Sized {
    fn split_hor(self, at: f32) -> [Self; 2];
    fn split_ver(self, at: f32) -> [Self; 2];

    fn partitions_hor<const N: usize>(self) -> [Self; N];
    fn partitions_ver<const N: usize>(self) -> [Self; N];
}

impl<U> LayoutElement for Rect<f32, U> {
    fn split_hor(self, at: f32) -> [Self; 2] {
        let mut r0 = self;
        r0.size.width = at;

        let mut r1 = self;
        r1.origin.x += at;
        r1.size.width -= at;

        [r0, r1]
    }

    fn split_ver(self, at: f32) -> [Self; 2] {
        let mut r0 = self;
        r0.size.height = at;

        let mut r1 = self;
        r1.origin.y += at;
        r1.size.height -= at;

        [r0, r1]
    }

    fn partitions_hor<const N: usize>(self) -> [Self; N] {
        let mut out = [self; N];

        let mut r0 = self;
        r0.size.width = self.width() / N as f32;

        for (i, rect) in out.iter_mut().enumerate() {
            rect.origin.x += r0.size.width * i as f32;
            rect.size.width = r0.size.width;
        }

        out
    }

    fn partitions_ver<const N: usize>(self) -> [Self; N] {
        let mut out = [self; N];

        let mut r0 = self;
        r0.size.height = self.height() / N as f32;

        for (i, rect) in out.iter_mut().enumerate() {
            rect.origin.y += r0.size.height * i as f32;
            rect.size.height = r0.size.height;
        }

        out
    }
}

#[derive(Clone, Copy)]
pub struct ListLayout {
    pub origin: Point2D<f32, ScreenSpace>,
    pub size: Size2D<f32, ScreenSpace>,
    pub side_offsets: Option<SideOffsets2D<f32, ScreenSpace>>,

    pub slot_height: Length<f32, ScreenSpace>,
}

impl ListLayout {
    /// Returns the rectangle that will contain the list slots (i.e.
    /// with `side_offsets` taken into account)
    pub fn inner_rect(&self) -> Rect<f32, ScreenSpace> {
        let rect = Rect::new(self.origin, self.size);
        if let Some(offsets) = self.side_offsets {
            rect.inner_rect(offsets)
        } else {
            rect
        }
    }

    /// Returns the number of slots that are visible in this layout,
    /// and the remainder if the slot height isn't evenly divisible
    /// with the list's inner height.
    pub fn slot_count(&self) -> (usize, f32) {
        let inner = self.inner_rect();
        let slot = self.slot_height.0;

        let count = inner.height().div_euclid(slot);
        let rem = inner.height().rem_euclid(slot);

        (count as usize, rem)
    }

    /// Returns the rectangle for the slot at the given index. If `ix`
    /// is pointing to a slot beyond the available height, `None` is
    /// returned.
    pub fn slot_rect(&self, ix: usize) -> Option<Rect<f32, ScreenSpace>> {
        let (count, _) = self.slot_count();

        if ix >= count {
            return None;
        }

        let inner = self.inner_rect();

        let mut slot = inner;

        slot.origin.y += ix as f32 * self.slot_height.0;
        slot.size.height = self.slot_height.0;

        Some(slot)
    }

    pub fn slot_at_screen_pos(
        &self,
        pos: Point2D<f32, ScreenSpace>,
    ) -> Option<usize> {
        let inner = self.inner_rect();

        if !inner.contains(pos) {
            return None;
        }

        let ix = pos.y.div_euclid(self.slot_height.0) as usize;

        if ix >= self.slot_count().0 {
            return None;
        }

        Some(ix)
    }

    pub fn apply_to_rows<'a, T: 'a>(
        &'a self,
        rows: impl Iterator<Item = T> + 'a,
    ) -> impl Iterator<Item = (usize, Rect<f32, ScreenSpace>, T)> + 'a {
        let (count, rem) = self.slot_count();
        // log::warn!("apply_to_rows slot count: {}, {}", count, rem);

        // ignore rows that would end up outside the list area
        rows.take(count).enumerate().map(|(ix, v)| {
            let rect = self.slot_rect(ix).unwrap();
            // log::warn!("apply_to_rows: {} -> {:?}", ix, rect);
            (ix, rect, v)
        })
    }

    /*
    // the output can then be mapped to vertices
    pub fn apply_to_rows<'a, 'b, T: 'a + 'b>(
        &'b self,
        rows: impl Iterator<Item = &'a T> + 'a + 'b,
    ) -> impl Iterator<Item = (usize, Rect<f32, ScreenSpace>, &'a T)> + 'a + 'b
    where
        'b: 'a,
    {
        let (count, _) = self.slot_count();

        // let layout = *self;

        // ignore rows that would end up outside the list area
        rows.take(count).enumerate().map(|(ix, v)| {
            let rect = self.slot_rect(ix).unwrap();
            (ix, rect, v)
        })
    }
    */
}

#[export_module]
pub mod rhai_module {
    //

    pub type Point2DF = euclid::Point2D<f32, euclid::UnknownUnit>;
    pub type Point2DI = euclid::Point2D<i64, euclid::UnknownUnit>;

    pub type Vec2DF = euclid::Vector2D<f32, euclid::UnknownUnit>;
    pub type Vec2DI = euclid::Vector2D<i64, euclid::UnknownUnit>;
}
