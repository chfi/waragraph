use ultraviolet::{Isometry3, Mat4, Rotor3, Vec2, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub struct View2D {
    center: Vec2,
    size: Vec2,
}

impl View2D {
    pub fn new(center: Vec2, size: Vec2) -> Self {
        Self { center, size }
    }

    pub fn center(&self) -> Vec2 {
        self.center
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    pub fn aspect(&self) -> f32 {
        self.size.x / self.size.y
    }

    pub fn set_aspect(&mut self, x_over_y: f32) {
        let height = self.size.y;
        let width = height * x_over_y;
        self.size.x = width;
    }

    pub fn x_range(&self) -> (f32, f32) {
        let x = self.center.x;
        let dx = self.size.x / 2.0;
        (x - dx, x + dx)
    }

    pub fn y_range(&self) -> (f32, f32) {
        let y = self.center.y;
        let dy = self.size.y / 2.0;
        (y - dy, y + dy)
    }

    /// Expands/contracts the view by a factor of `s`, keeping the
    /// point corresponding to `t` fixed in the view.
    ///
    /// Both `t.x` and `t.y` should be in `[0, 1]`, if `s` > 1.0, the
    /// view is zoomed out, if `s` < 1.0, it is zoomed in.
    pub fn zoom_with_focus(&mut self, t: Vec2, s: f32) {
        let (l, r) = self.x_range();
        let (u, d) = self.y_range();

        let (l_, r_) = expand_with_fixpoint(l, r, t.x, s);
        let (u_, d_) = expand_with_fixpoint(u, d, t.y, s);

        let width = r_ - l_;
        let height = d_ - u_;

        self.center = Vec2::new(l_ + width / 2.0, u_ + height / 2.0);
        self.size = Vec2::new(width, height);
    }

    /// Translate the view by `delta * self.size`.
    pub fn translate_size_rel(&mut self, delta: Vec2) {
        self.center += delta * self.size;
    }

    pub fn to_matrix(&self) -> Mat4 {
        let right = self.size.x / 2.0;
        let left = -right;
        let top = self.size.y / 2.0;
        let bottom = -top;

        let near = 1.0;
        let far = 10.0;

        let proj = ultraviolet::projection::rh_yup::orthographic_wgpu_dx(
            left, right, bottom, top, near, far,
        );

        let p = self.center;
        let p_ = Vec3::new(p.x, p.y, 5.0);

        let view = Isometry3::new(p_, Rotor3::identity()).inversed();

        proj * view.into_homogeneous_matrix()
    }
}

fn expand_with_fixpoint(a: f32, b: f32, t: f32, s: f32) -> (f32, f32) {
    let l = b - a;
    let x = a + t * l;

    let p_a = t;
    let p_b = 1.0 - t;

    let mut l_ = l * s;

    /* // NB: this should probably be handled elsewhere
        // just so things don't implode
        if l_ < 1.0 {
            l_ = 1.0;
        }
    */

    let x_a = p_a * l_;
    let x_b = p_b * l_;

    let a_ = x - x_a;
    let b_ = x + x_b;

    (a_, b_)
}
