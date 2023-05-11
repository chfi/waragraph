use waragraph_core::graph::Bp;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct View1D {
    range: std::ops::Range<u64>,
    max: u64,
}

impl View1D {
    pub fn new(max: u64) -> Self {
        let range = 0..max;
        Self { range, max }
    }

    pub fn range(&self) -> &std::ops::Range<u64> {
        &self.range
    }

    pub fn offset(&self) -> u64 {
        self.range.start
    }

    pub fn len(&self) -> u64 {
        self.range.end - self.range.start
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    pub fn reset(&mut self) {
        self.range = 0..self.max;
    }

    fn make_valid(&mut self) {
        let len = self.len();

        if self.range.end > self.max() {
            self.range.end = self.max();
        }

        let max_offset = self.max.checked_sub(len).unwrap_or_default();
        if self.offset() > max_offset {
            self.range.start = max_offset;
        }
    }

    pub fn set(&mut self, left: u64, right: u64) {
        self.range = left..right;
        self.make_valid();
    }

    pub fn translate(&mut self, delta: i64) {
        let d = delta.abs() as u64;
        let len = self.len();
        if delta > 0 {
            self.range.end += d;
            self.range.start += d;
        } else if delta < 0 {
            self.range.start =
                self.range.start.checked_sub(d).unwrap_or_default();
            self.range.end = self.range.start + len;
        }

        self.make_valid();
    }

    /// `delta` is in "view width" units, so +1 means panning the view
    /// to the right by `self.len()` units.
    pub fn translate_norm_f32(&mut self, fdelta: f32) {
        let delta = (fdelta * self.len() as f32) as i64;
        self.translate(delta);
    }

    /// `fix` is a normalized point in the view [0..1] that will not
    /// move during the zoom
    pub fn zoom_around_norm_f32(&mut self, fix: f32, zdelta: f32) {
        println!("zdelta: {zdelta}");
        let old_len = self.len() as f32;
        let new_len = old_len * zdelta;
        let extra = new_len - old_len;

        let mut l = self.range.start as f32;
        let mut r = self.range.end as f32;

        let left_prop = fix;
        let right_prop = 1.0 - fix;

        let max = self.max() as f32;

        l -= (left_prop * extra).clamp(0.0, max);
        r += (right_prop * extra).clamp(l, max);

        let l = l as u64;
        let r = r as u64;
        self.range = l..r;
        println!("new range: {l}..{r}");

        self.make_valid();
    }

    /// Expands/contracts the view by a factor of `s`, keeping the point
    /// corresponding to `t` fixed in the view.
    ///
    /// `t` should be in `[0, 1]`, if `s` > 1.0, the view is zoomed out,
    /// if `s` < 1.0, it is zoomed in.
    pub fn zoom_with_focus(&mut self, t: f32, s: f32) {
        let l0 = self.range.start as f32;
        let r0 = self.range.end as f32;

        let v = r0 - l0;

        let x = l0 + t * v;

        let p_l = t;
        let p_r = 1.0 - t;

        let mut v_ = v * s;

        // just so things don't implode
        if v_ < 1.0 {
            v_ = 1.0;
        }

        let x_l = p_l * v_;
        let x_r = p_r * v_;

        let l1 = x - x_l;
        let r1 = x + x_r;

        let max = self.max as f32;

        let l = l1.min(r1).clamp(0.0, max);
        let r = r1.max(l1).clamp(0.0, max);

        let range = (l.round() as u64)..(r.round() as u64);
        self.range = range;
    }
}

// various useful view-related transformations
impl View1D {
    // to implement:
    // View1D & Screen space range (e.g. slots)
    //   => Bp -> Screen X;
    //   => Screen X -> Bp

    // View1D & View1D, treated as the view at t = 0 and t = 1 respectively (e.g. transforms)
    //   => Bp -> Bp
    //   => Screen space range -> Bp -> Bp

    /// Maps the view (`self`) to `screen_interval`, and then returns
    /// the intersection of the image of `pan_range` under this map
    /// with the given `screen_interval`. Returns `None` if the
    /// intersection is empty.
    pub fn map_bp_interval_to_screen_x(
        &self,
        pan_range: &std::ops::Range<Bp>,
        screen_interval: &std::ops::RangeInclusive<f32>,
    ) -> Option<std::ops::RangeInclusive<f32>> {
        let vrange = self.range();
        let pleft = pan_range.start.0;
        let pright = pan_range.end.0;

        if pleft > vrange.end || pright < vrange.start {
            return None;
        }

        let vl = vrange.start as f32;
        let vr = vrange.end as f32;
        let vlen = vr - vl;

        let left = pleft.max(vrange.start);
        let right = pright.min(vrange.end);

        let l = left as f32;
        let r = right as f32;

        let lt = (l - vl) / vlen;
        let rt = (r - vl) / vlen;

        let (sleft, sright) = screen_interval.clone().into_inner();
        let slen = sright - sleft;

        let a_left = sleft + lt * slen;
        let a_right = sleft + rt * slen;

        Some(a_left..=a_right)
    }
}

impl View1D {
    pub fn try_center(&mut self, on: std::ops::Range<Bp>) {
        let range_len = on.end.0 - on.start.0;

        let mid = on.start.0 + range_len / 2;

        let cur_mid = self.offset() + self.len() / 2;

        if range_len > self.len() {
            // if `on` is bigger than the current view, make the new view
            // some fixed multiple of the input range in size, centered
            // correctly
            self.set(on.start.0, on.end.0);
            self.zoom_around_norm_f32(0.5, 1.5);
        } else {
            // otherwise, do not resize the view, just translate it (if possible)
            let delta = cur_mid as i64 - mid as i64;
            self.translate(delta);
        }
    }
}
