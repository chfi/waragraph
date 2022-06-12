use std::ops::{Add, Div, Sub};

use euclid::*;
use nalgebra::{Norm, Normed};
use num_traits::{FromPrimitive, ToPrimitive};
use raving::compositor::Compositor;
use rayon::iter::ParallelIterator;
use rustc_hash::FxHashMap;

// use num_traits::{
//     one, zero, AsPrimitive, FromPrimitive, Num, NumOps, One, ToPrimitive,
// };

use crate::{
    geometry::{
        ScreenPoint, ScreenRect, ScreenSize, ScreenSpace, ScreenVector,
    },
    viewer::gui::layer::{line_width_rgba, line_width_rgba2, rect_rgba},
};

use anyhow::Result;

pub type Point = ScreenPoint;
pub type Vec2 = ScreenVector;
pub type Rect = ScreenRect;
pub type Size = ScreenSize;

#[derive(Clone, Copy)]
pub struct Entity {
    // pub prev_origin: Point,
    pub prev_pos: Point,
    pub pos: Point,
    pub acc: Vec2,
    pub size: Size,
}

impl Entity {
    pub fn new(x: f32, y: f32) -> Self {
        let pos = point2(x, y);
        let prev_pos = pos;
        let acc = vec2(0.0, 0.0);
        let size = size2(40.0, 40.0);

        Self {
            prev_pos,
            pos,
            acc,
            size,
        }
    }

    pub fn to_vertex(&self) -> [u8; 32] {
        let color = rgb::RGBA::new(0.7f32, 0.0, 0.0, 1.0);
        rect_rgba(
            ScreenRect {
                origin: self.pos,
                size: self.size,
            },
            color,
        )
    }

    pub fn accelerate(&mut self, a: Vec2) {
        self.acc += a;
    }

    pub fn update(&mut self, dt: f32) {
        let vel = self.pos - self.prev_pos + self.acc * dt * dt;
        self.prev_pos = self.pos;
        self.pos += vel;
        self.acc = vec2(0.0, 0.0);
    }
}

#[derive(Default, Clone)]
pub struct VerletSolver {
    pub entities: Vec<Entity>,
}

impl VerletSolver {
    pub fn update(&mut self, dt: f32) {
        for ent in self.entities.iter_mut() {
            ent.accelerate(vec2(0.0, 100.0));
            ent.update(dt);
        }
    }

    pub fn update_layer(
        &self,
        compositor: &mut Compositor,
        layer_name: &str,
    ) -> Result<()> {
        use palette::{FromColor, Hue, IntoColor, Lch, Srgb};

        let lch_color: Lch = Srgb::new(0.8, 0.2, 0.1).into_color();
        // let new_color = Srgb::from_color(lch_color.shift_hue(180.0));

        let rect_sublayer = "rects";
        let line_sublayer = "lines";
        let line_2_sublayer = "lines-2";

        // let lin_color =

        // let rect = euclid::rect(100.0f32, 100.0, 500.0, 500.0);
        // let origin = rect.center();

        compositor.with_layer(layer_name, |layer| {
            if let Some(sublayer_data) = layer
                .get_sublayer_mut(rect_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                sublayer_data.update_vertices_array(
                    self.entities.iter().map(|e| e.to_vertex()),
                )?;
                /*
                sublayer_data.update_vertices_array(self.e_to_v.iter().map(
                    |(i, j)| {
                        let a = self.vertices[*i];
                        let b = self.vertices[*j];

                        line_width_rgba(a, b, w, w, color)
                    },
                ));
                */
            }

            /*
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
            */

            Ok(())
        })?;

        Ok(())
    }
}
