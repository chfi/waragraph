use std::ops::{Add, Div, Sub};

use euclid::*;
use num_traits::{FromPrimitive, ToPrimitive};

// use num_traits::{
//     one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive,
// };

use super::{ScreenPoint, ScreenRect, ScreenSize, ScreenSpace};

use ndarray::prelude::*;

#[derive(Debug, Clone)]
pub struct GraphLayout {
    vertices: Vec<ScreenPoint>,
    sizes: Vec<ScreenSize>,

    adj: Array2<f32>,
    deg: Array2<f32>,

    laplacian: Array2<f32>,
}

pub fn loop_layout(shape: ScreenRect) -> GraphLayout {
    let side_len = 4;
    let short_len = side_len - 2;
    let n = side_len * 2 + short_len * 2;

    let size = ScreenSize::new(40.0, 20.0);

    let mut vertices = Vec::new();
    let sizes = vec![size; n];

    let h_step = vec2(shape.size.width, 0.0) / n as f32;
    let v_step = vec2(0.0, shape.size.height) / n as f32;

    eprintln!("h_step: {:?}", h_step);
    eprintln!("v_step: {:?}", v_step);

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
        for i in 0..side_len {
            let point = base + step * i as f32;
            vertices.push(point);
        }
    }

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

    GraphLayout {
        vertices,
        sizes,

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

        assert!(false);

        Ok(())
    }
}
