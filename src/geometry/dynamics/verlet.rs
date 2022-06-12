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

    pub color: rgb::RGBA<f32>,
}

impl Entity {
    pub fn rect(&self) -> Rect {
        Rect::new(self.pos, self.size)
    }

    pub fn new(x: f32, y: f32, color: rgb::RGBA<f32>) -> Self {
        let pos = point2(x, y);
        let prev_pos = pos;
        let acc = vec2(0.0, 0.0);
        let size = size2(40.0, 40.0);

        Self {
            prev_pos,
            pos,
            acc,
            size,
            color,
        }
    }

    pub fn to_vertex(&self) -> [u8; 32] {
        rect_rgba(
            ScreenRect {
                origin: self.pos,
                size: self.size,
            },
            self.color,
        )
    }

    pub fn accelerate(&mut self, a: Vec2) {
        self.acc += a;
    }

    pub fn update(&mut self, dt: f32) {
        let vel = self.pos - self.prev_pos;
        self.prev_pos = self.pos;
        self.pos = self.pos + vel + self.acc * dt * dt;
        self.acc = vec2(0.0, 0.0);
    }
}

#[derive(Clone)]
pub struct VerletSolver {
    pub entities: Vec<Entity>,

    pub links: Vec<((usize, usize), f32)>,

    pub bounds: Rect,
}

impl VerletSolver {
    pub fn new(width: u32, height: u32) -> Self {
        let bounds = rect(0.0, 0.0, width as f32, height as f32);

        Self {
            entities: Vec::new(),
            bounds,
            links: Vec::new(),
        }
    }

    pub fn stop(&mut self) {
        self.apply_constraints();
        self.solve_collisions();
        self.entities.iter_mut().for_each(|e| e.prev_pos = e.pos);
    }

    pub fn update(&mut self, dt: f32) {
        let sub_steps = 16;

        let sub_dt = dt / sub_steps as f32;

        for _ in 0..sub_steps {
            for ent in self.entities.iter_mut() {
                ent.accelerate(vec2(0.0, 100.0));
                ent.update(sub_dt);
            }

            self.apply_constraints();
            self.solve_collisions();
        }
    }

    pub fn apply_constraints(&mut self) {
        let n = self.entities.len();

        for ent in self.entities.iter_mut() {
            let bounds = self.bounds;

            let top = bounds.min_y();
            let btm = bounds.max_y();

            let lhs = bounds.min_x();
            let rhs = bounds.max_x();

            let rect: Rect = ent.rect();

            if rect.min_y() < top {
                ent.pos.y = top;
            }

            if rect.min_x() < lhs {
                ent.pos.x = lhs;
            }

            if rect.max_y() > btm {
                ent.pos.y = btm - rect.height();
            }

            if rect.max_x() > rhs {
                ent.pos.x = rhs - rect.width();
            }
        }

        /*
        for &((i, j), len) in self.links.iter() {
            let mut a = self.entities[i];
            let mut b = self.entities[j];

            let ra = a.rect();
            let rb = b.rect();

            let oa = ra.center();
            let ob = rb.center();

            let v: Vec2 = ob - oa;

            if v.length() >= len {
                let n = v / v.length();

                let delta = (v.length() - len) / 2.0;

                a.pos += n * delta;
                b.pos -= n * delta;
            }

            self.entities[i] = a;
            self.entities[j] = b;
        }
        */
    }

    pub fn solve_collisions(&mut self) {
        let n = self.entities.len();

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let mut a = self.entities[i];
                let mut b = self.entities[j];

                let r_a = a.rect();
                let r_b = b.rect();

                if let Some(intersect) = r_a.intersection(&r_b) {
                    let dv = r_b.origin - r_a.origin;

                    let dx = intersect.width() / 2.0;
                    let dy = intersect.height() / 2.0;

                    let p = a.pos;
                    let q = b.pos;

                    if dx > dy {
                        if p.y > q.y {
                            a.pos.y += dy;
                            b.pos.y -= dy;
                        } else {
                            a.pos.y -= dy;
                            b.pos.y += dy;
                        }
                    } else {
                        if p.x > q.x {
                            a.pos.x += dx;
                            b.pos.x -= dx;
                        } else {
                            a.pos.x -= dx;
                            b.pos.x += dx;
                        }
                    }

                    // if dx > dy {
                    //     if p.x > q.x {
                    //         a.pos.x -= dx;
                    //         b.pos.x += dx;
                    //     } else {
                    //         a.pos.x += dx;
                    //         b.pos.x -= dx;
                    //     }
                    // } else {
                    //     if p.y > q.y {
                    //         a.pos.y -= dy;
                    //         b.pos.y += dy;
                    //     } else {
                    //         a.pos.y += dy;
                    //         b.pos.y -= dy;
                    //     }
                    // }

                    self.entities[i] = a;
                    self.entities[j] = b;
                }
            }
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
                    self.entities.iter().enumerate().map(|(i, e)| {
                        //
                        e.to_vertex()
                    }),
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

            if let Some(sublayer_data) = layer
                .get_sublayer_mut(line_2_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                sublayer_data.update_vertices_array(self.links.iter().map(
                    |&((i, j), tgt_len)| {
                        let a = self.entities[i];
                        let b = self.entities[j];

                        // let d_a = o[*i];
                        // let d_b = o[*j];
                        // let d_a = vx_vals[*i];
                        // let d_b = vx_vals[*j];

                        let p = a.rect().center();
                        let q = b.rect().center();

                        let dist = (p - q).length();

                        let c1 = Srgb::from_color(
                            lch_color.shift_hue((dist - tgt_len).abs()),
                        );

                        // Srgb::from_color(lch_color.shift_hue(d_a * 200.0));

                        let color =
                            rgb::RGBA::new(c1.red, c1.green, c1.blue, 1.0);

                        let w = 1.0;

                        line_width_rgba2(p, q, w, w, color, color)

                        // let c0 =
                        //     Srgb::from_color(lch_color.shift_hue(d_a * 200.0));
                        // let c1 =
                        //     Srgb::from_color(lch_color.shift_hue(d_b * 200.0));

                        // let color0 =
                        //     rgb::RGBA::new(c0.red, c0.green, c0.blue, 1.0);
                        // let color1 =
                        //     rgb::RGBA::new(c1.red, c1.green, c1.blue, 1.0);

                        // line_width_rgba2(a, b, w, w, color0, color1)
                    },
                ))?;

                /*
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
                */
            }

            Ok(())
        })?;

        Ok(())
    }
}
