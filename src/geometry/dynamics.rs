use std::ops::{Add, Div, Sub};

use euclid::*;
use nalgebra::{Norm, Normed};
use num_traits::{FromPrimitive, ToPrimitive};
use raving::compositor::Compositor;
use rustc_hash::FxHashMap;

// use num_traits::{
//     one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive,
// };

use crate::{
    geometry::ScreenVector,
    viewer::gui::layer::{line_width_rgba, line_width_rgba2},
};

use super::{ScreenPoint, ScreenRect, ScreenSize, ScreenSpace};

use anyhow::Result;

use ndarray::prelude::*;

pub mod verlet;

#[derive(Debug, Clone)]
pub struct CurveLayout {
    vertices: Vec<ScreenPoint>,
    sizes: Vec<ScreenSize>,

    // e_to_v: Array2<usize>,
    e_to_v: Vec<(usize, usize)>,

    adj: Array2<f32>,
    deg: Array2<f32>,

    laplacian: Array2<f32>,
}

impl CurveLayout {
    pub fn update_layer(
        &self,
        compositor: &mut Compositor,
        layer_name: &str,
    ) -> Result<()> {
        use palette::{FromColor, Hue, IntoColor, Lch, Srgb};

        let lch_color: Lch = Srgb::new(0.8, 0.2, 0.1).into_color();
        // let new_color = Srgb::from_color(lch_color.shift_hue(180.0));

        let line_sublayer = "lines";
        let line_2_sublayer = "lines-2";

        // let lin_color =

        let rect = euclid::rect(100.0f32, 100.0, 500.0, 500.0);
        let origin = rect.center();

        let vx_vals = self
            .vertices
            .iter()
            .enumerate()
            .map(|(ix, p)| {
                let p: ScreenPoint = *p;
                let dist_from_mid = p.distance_to(origin);
                dist_from_mid * 2.0
            })
            .collect::<Vec<_>>();

        let mut t_vecs: FxHashMap<usize, ScreenVector> = FxHashMap::default();

        for (i, j) in self.e_to_v.iter() {
            let a = self.vertices[*i];
            let b = self.vertices[*j];

            let c: ScreenVector = b - a;

            t_vecs.insert(*i, c.normalize());
        }

        let mut t_vecs = t_vecs.into_iter().collect::<Vec<_>>();
        t_vecs.sort_by_key(|(i, _)| *i);
        let t_vecs = t_vecs.into_iter().map(|(_, t)| t).collect::<Vec<_>>();

        let mut a_rads: Vec<f32> = Vec::new();

        for (i, &vi) in t_vecs.iter().enumerate() {
            let j = (i + 1) % t_vecs.len();
            let vj = t_vecs[j];

            let vi: ScreenVector = vi;

            a_rads.push(vi.angle_to(vj).radians);
        }

        let v = Array1::from_iter(vx_vals.iter().copied());

        let angles = Array1::from_iter(a_rads.iter().copied());

        // let o = &self.laplacian * &v * 0.001;
        let o = self.laplacian.dot(&angles);

        eprintln!("{:#?}", o);

        // eprintln!("{:?}", vx_vals);

        let color = rgb::RGBA::new(0.8f32, 0.2, 0.2, 1.0);
        let w = 2.0;

        compositor.with_layer(layer_name, |layer| {
            // if let Some(sublayer_data) = layer
            //     .get_sublayer_mut(line_sublayer)
            //     .and_then(|s| s.draw_data_mut().next())
            // {
            //     sublayer_data.update_vertices_array(self.e_to_v.iter().map(
            //         |(i, j)| {
            //             let a = self.vertices[*i];
            //             let b = self.vertices[*j];

            //             line_width_rgba(a, b, w, w, color)
            //         },
            //     ));
            // }

            if let Some(sublayer_data) = layer
                .get_sublayer_mut(line_2_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                sublayer_data.update_vertices_array(self.e_to_v.iter().map(
                    |(i, j)| {
                        let a = self.vertices[*i];
                        let b = self.vertices[*j];

                        let d_a = o[*i];
                        let d_b = o[*j];
                        // let d_a = vx_vals[*i];
                        // let d_b = vx_vals[*j];

                        let c0 =
                            Srgb::from_color(lch_color.shift_hue(d_a * 200.0));
                        let c1 =
                            Srgb::from_color(lch_color.shift_hue(d_b * 200.0));

                        let color0 =
                            rgb::RGBA::new(c0.red, c0.green, c0.blue, 1.0);
                        let color1 =
                            rgb::RGBA::new(c1.red, c1.green, c1.blue, 1.0);

                        line_width_rgba2(a, b, w, w, color0, color1)
                    },
                ));
            }

            Ok(())
        })?;

        Ok(())
    }

    // pub fn edges(&self) -> impl Iterator<Item = (usize, usize)> {
    //     self.e_to_v.rows().
    // }
}

pub fn loop_layout(shape: ScreenRect) -> CurveLayout {
    let side_len = 4;
    let short_len = side_len - 2;
    let n = side_len * 2 + short_len * 2;

    let size = ScreenSize::new(40.0, 20.0);

    let mut vertices = Vec::new();
    let sizes = vec![size; n];

    let h_step = vec2(shape.size.width, 0.0) / n as f32;
    let v_step = vec2(0.0, shape.size.height) / n as f32;

    // eprintln!("n: {}", n);
    // eprintln!("h_step: {:?}", h_step);
    // eprintln!("v_step: {:?}", v_step);

    let top_left = shape.origin;

    let top_right = shape.origin + vec2(shape.size.width, 0.0);
    let bottom_right = shape.origin + vec2(shape.size.width, shape.size.height);
    let bottom_left = shape.origin + vec2(0.0, shape.size.height);

    for (base, step) in [
        (top_left, h_step),
        (top_right, v_step),
        (bottom_right, -h_step),
        (bottom_left, -v_step),
    ] {
        for i in 0..(side_len - 1) {
            // eprintln!("i: {}", i);
            let point = base + step * i as f32;
            vertices.push(point);
        }
    }

    // eprintln!("vertex count: {}", vertices.len());

    let e_to_v = vertices
        .iter()
        .enumerate()
        .map(|(ix, _)| (ix, (ix + 1) % n))
        .collect();

    /*
    let e_to_v = Array2::from_shape_fn((n, n), |(i, j)| {
        if i == j || i + 1 == j {
            1
        } else if i == n - 1 && j == 0 {
            1
        } else {
            0
        }
    });
    */

    let adj = Array2::from_shape_fn((n, n), |(i, j)| {
        let a = i.min(j);
        let b = i.max(j);

        if i.abs_diff(j) == 1 {
            1.0
        } else if a == 0 && b + 1 == n {
            1.0
        } else {
            0.0
        }
    });

    let deg = Array2::from_diag_elem(n, 2.0);

    let laplacian = &deg - &adj;

    CurveLayout {
        vertices,
        sizes,

        e_to_v,

        adj,
        deg,
        laplacian,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_layout() -> anyhow::Result<()> {
        let origin = point2(150.0, 200.0);
        let size = size2(100.0, 100.0);

        let rect = ScreenRect { origin, size };

        let mut layout = loop_layout(rect);

        eprintln!("{:#?}", layout);

        // assert!(false);

        Ok(())
    }
}
