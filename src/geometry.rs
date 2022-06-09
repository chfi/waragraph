use rhai::plugin::*;

use euclid::*;

use crate::config::ConfigMap;

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
pub type ScreenSideOffsets = SideOffsets2D<f32, ScreenSpace>;
// pub type ScreenBox2D = Box2D<f32, ScreenSpace>;

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
    pub fn from_config_map(
        config: &ConfigMap,
        size: ScreenSize,
    ) -> Option<Self> {
        let map = config.map.read();
        let get_cast = |m: &rhai::Map, k| m.get(k).unwrap().clone_cast::<i64>();

        let label = map.get("layout.label").unwrap().clone_cast::<rhai::Map>();
        let slot = map.get("layout.slot").unwrap().clone_cast::<rhai::Map>();

        let bottom_pad = get_cast(&map, "layout.list_bottom_pad") as usize;

        let label_x = get_cast(&label, "x") as f32;

        let slot_y = get_cast(&slot, "y") as f32;
        let slot_w = get_cast(&slot, "w") as f32;
        let slot_h = get_cast(&slot, "h") as f32;

        let origin = point2(0.0, 0.0);

        let top = slot_y;
        let right = -slot_w;
        let bottom = bottom_pad as f32;
        let left = label_x;

        let side_offsets = Some(SideOffsets2D::new(top, right, bottom, left));

        let slot_height = Length::new(slot_h as f32);

        Some(Self {
            origin,
            size,
            side_offsets,

            slot_height,
        })
    }

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

#[derive(Clone)]
pub struct CachedListLayout {
    list_layout: ListLayout,
    cached_slots: Vec<ScreenRect>,
}

impl CachedListLayout {
    pub fn from_layout(layout: &ListLayout) -> Self {
        let mut res = Self {
            list_layout: *layout,
            cached_slots: Vec::with_capacity(layout.slot_count().0),
        };

        res.refresh();

        res
    }

    pub fn set_layout(&mut self, list_layout: ListLayout) {
        self.list_layout = list_layout;
        self.refresh();
    }

    pub fn with_layout_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut ListLayout),
    {
        f(&mut self.list_layout);
        self.refresh();
    }

    pub fn apply_to_rows<'a, T: 'a>(
        &'a self,
        rows: impl Iterator<Item = T> + 'a,
    ) -> impl Iterator<Item = (usize, Rect<f32, ScreenSpace>, T)> + 'a {
        rows.zip(self.cached_slots.iter())
            .enumerate()
            .map(|(ix, (val, rect))| (ix, *rect, val))
    }

    fn refresh(&mut self) {
        self.cached_slots.clear();
        let layout = &self.list_layout;
        self.cached_slots
            .extend(layout.apply_to_rows(0..).map(|(_, rect, _)| rect))
    }
}

#[export_module]
pub mod rhai_module {
    //

    // pub type Point2DF = euclid::Point2D<f32, euclid::UnknownUnit>;
    // pub type Point2DI = euclid::Point2D<i64, euclid::UnknownUnit>;

    // pub type Vec2DF = euclid::Vector2D<f32, euclid::UnknownUnit>;
    // pub type Vec2DI = euclid::Vector2D<i64, euclid::UnknownUnit>;

    use euclid::{point2, rect, size2};

    use crate::console::EvalResult;

    pub type ScreenPoint = super::ScreenPoint;
    pub type ScreenSize = super::ScreenSize;
    pub type ScreenVector = super::ScreenVector;
    pub type ScreenRect = super::ScreenRect;
    pub type ScreenSideOffsets = super::ScreenSideOffsets;

    #[rhai_fn(global, return_raw, name = "point_from_map")]
    pub fn screen_point_from_map(
        map: &mut rhai::Map,
    ) -> EvalResult<ScreenPoint> {
        let res = map.get("x").and_then(|x| {
            let y = map.get("y")?;

            let x = x.as_float().ok()?;
            let y = y.as_float().ok()?;

            Some((x, y))
        });

        if let Some((x, y)) = res {
            Ok(point2(x, y))
        } else {
            Err("Map must have `x` and `y` float fields".into())
        }
    }

    #[rhai_fn(global, return_raw, name = "size_from_map")]
    pub fn screen_size_from_map(map: &mut rhai::Map) -> EvalResult<ScreenSize> {
        let res = map.get("width").and_then(|w| {
            let h = map.get("height")?;

            let w = w.as_float().ok()?;
            let h = h.as_float().ok()?;

            Some((w, h))
        });

        if let Some((w, h)) = res {
            Ok(size2(w, h))
        } else {
            Err("Map must have `width` and `height` float fields".into())
        }
    }

    /*
    #[rhai_fn(global, return_raw, name = "rect_from_map")]
    pub fn screen_rect_from_map(map: &mut rhai::Map) -> EvalResult<ScreenRect> {
        let res = map.get("origin").and_then(|origin| {
            let size = map.get("size")?;

            let o_lock = origin.read_lock::<rhai::Map>()?;

            let origin = origin.as_float().ok()?;
            let size = size.as_float().ok()?;

            Some((origin, size))
        });

        if let Some((origin, size)) = res {
            Ok(ScreenRect { origin, size })
        } else {
            Err("Map must have `origin` and `size` geometry fields".into())
        }
    }
    */

    #[rhai_fn(global, pure, get = "x")]
    pub fn screen_point_get_x(p: &mut ScreenPoint) -> f32 {
        p.x
    }

    #[rhai_fn(global, pure, get = "y")]
    pub fn screen_point_get_y(p: &mut ScreenPoint) -> f32 {
        p.y
    }

    #[rhai_fn(global, set = "x")]
    pub fn screen_point_set_x(p: &mut ScreenPoint, x: f32) {
        p.x = x;
    }

    #[rhai_fn(global, set = "y")]
    pub fn screen_point_set_y(p: &mut ScreenPoint, y: f32) {
        p.y = y;
    }

    #[rhai_fn(global, pure, get = "top")]
    pub fn screen_offsets_get_top(s: &mut ScreenSideOffsets) -> f32 {
        s.top
    }

    #[rhai_fn(global, pure, get = "right")]
    pub fn screen_offsets_get_right(s: &mut ScreenSideOffsets) -> f32 {
        s.right
    }

    #[rhai_fn(global, pure, get = "bottom")]
    pub fn screen_offsets_get_bottom(s: &mut ScreenSideOffsets) -> f32 {
        s.bottom
    }

    #[rhai_fn(global, pure, get = "left")]
    pub fn screen_offsets_get_left(s: &mut ScreenSideOffsets) -> f32 {
        s.left
    }

    #[rhai_fn(global, set = "top")]
    pub fn screen_offsets_set_top(s: &mut ScreenSideOffsets, v: f32) {
        s.top = v;
    }

    #[rhai_fn(global, set = "right")]
    pub fn screen_offsets_set_right(s: &mut ScreenSideOffsets, v: f32) {
        s.right = v;
    }

    #[rhai_fn(global, set = "bottom")]
    pub fn screen_offsets_set_bottom(s: &mut ScreenSideOffsets, v: f32) {
        s.bottom = v;
    }

    #[rhai_fn(global, set = "left")]
    pub fn screen_offsets_set_left(s: &mut ScreenSideOffsets, v: f32) {
        s.left = v;
    }
}
