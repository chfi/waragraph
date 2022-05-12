use std::{
    collections::{BTreeMap, HashMap},
    io::BufReader,
    num::NonZeroU32,
};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use parking_lot::RwLock;
use raving::compositor::{label_space::LabelSpace, Compositor};
use raving::{
    script::console::BatchBuilder,
    vk::{context::VkContext, BufferIx, GpuResources, VkEngine},
};
use rustc_hash::FxHashMap;

use sled::IVec;
use thunderdome::{Arena, Index};

use sprs::{CsMatI, CsVecI, TriMatI};
use zerocopy::{AsBytes, FromBytes};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use bstr::ByteSlice as BstrByteSlice;

use crate::{
    graph::{Node, Path, Waragraph},
    util::{BufFmt, BufId, BufMeta, BufferStorage, LabelStorage},
    viewer::{DataSource, SlotFnCache, ViewDiscrete1D},
};

use rhai::plugin::*;

use lazy_static::lazy_static;

use rand::prelude::*;

use nalgebra::{point, vector, Point2, Vector2};

pub type Pos2 = Point2<f32>;
pub type Vec2 = Vector2<f32>;

pub struct LabelStacks {
    pub label_space: LabelSpace,
    label_map: HashMap<rhai::ImmutableString, roaring::RoaringBitmap>,

    layer_name: rhai::ImmutableString,

    sublayer_rect: rhai::ImmutableString,
    sublayer_text: rhai::ImmutableString,
}

impl LabelStacks {
    pub fn from_label_map(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        label_map: HashMap<rhai::ImmutableString, roaring::RoaringBitmap>,
    ) -> Result<Self> {
        let mut label_space =
            LabelSpace::new(engine, "label-stacks-labels", 1024 * 1024)?;

        let layer_name = "label-stacks-layer";
        let rect_name = "label-stacks:rect";
        let text_name = "label-stacks:text";

        compositor.new_layer(layer_name, 1, true);

        compositor.with_layer(layer_name, |layer| {
            Compositor::push_sublayer(
                &compositor.sublayer_defs,
                engine,
                layer,
                "rect-rgb",
                rect_name,
                None,
            )?;

            Compositor::push_sublayer(
                &compositor.sublayer_defs,
                engine,
                layer,
                "text",
                text_name,
                [label_space.text_set],
            )?;

            Ok(())
        })?;

        for text in label_map.keys() {
            label_space.insert(text)?;
        }

        label_space.write_buffer(&mut engine.resources).unwrap();

        Ok(Self {
            label_space,
            label_map,

            layer_name: layer_name.into(),

            sublayer_rect: rect_name.into(),
            sublayer_text: text_name.into(),
        })
    }

    pub fn update_layer(
        &self,
        compositor: &mut Compositor,
        graph: &Arc<Waragraph>,
        view: ViewDiscrete1D,
        slot_offset: f32,
        slot_width: f32,
    ) -> Result<()> {
        let start = graph.node_at_pos(view.offset).unwrap();

        let view_right = (view.offset + view.len).min(view.max - 1);

        let end = graph.node_at_pos(view_right).unwrap();

        let s = u32::from(start);
        let e = u32::from(end);

        let mut view_set = roaring::RoaringBitmap::new();
        view_set.insert_range(s..e);

        let pos_to_x = |p: usize| -> f32 {
            let x0 = view.offset as f64;
            let v_len = view.len as f64;
            let x_p = ((p as f64) - x0) / v_len;
            let w_len = slot_width as f64;
            ((slot_offset as f64) + (w_len * x_p)) as f32
        };

        let mut labels_by_x: Vec<((usize, usize), f32)> = self
            .label_map
            .iter()
            .filter_map(|(label, set)| {
                if view_set.intersection_len(set) == 0 {
                    return None;
                }

                let intersect = &view_set & set;
                let l = intersect.min().unwrap();
                let r = intersect.max().unwrap();

                let len = intersect.len() as u32;
                let m = intersect.select(len / 2);
                let mid = l + ((r - l) / 2);

                let bounds =
                    self.label_space.bounds_for(label.as_str()).unwrap();
                let pos = graph.node_pos(Node::from(mid));

                Some((bounds, pos_to_x(pos)))
            })
            .collect();

        // it's fine to just sort as integers
        labels_by_x.sort_by_key(|(_, x)| *x as usize);

        // let mut label_pos_map: HashMap<rhai::ImmutableString, usize> = self
        let mut label_x_map: HashMap<
            rhai::ImmutableString,
            ((usize, usize), f32),
        > = self
            .label_map
            .iter()
            .filter_map(|(label, set)| {
                if view_set.intersection_len(set) == 0 {
                    return None;
                }

                let intersect = &view_set & set;
                let l = intersect.min().unwrap();
                let r = intersect.max().unwrap();

                let len = intersect.len() as u32;
                let m = intersect.select(len / 2);
                let mid = l + ((r - l) / 2);

                let bounds =
                    self.label_space.bounds_for(label.as_str()).unwrap();
                let pos = graph.node_pos(Node::from(mid));

                Some((label.clone(), (bounds, pos_to_x(pos))))
            })
            .collect();

        compositor.with_layer(&self.layer_name, |layer| {
            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_text)
            {
                sublayer.update_vertices_array(label_x_map.into_iter().map(
                    |(_, ((s, l), x))| {
                        //
                        let color = [0f32, 0.0, 0.0, 1.0];

                        let mut out = [0u8; 8 + 8 + 16];

                        out[0..8].clone_from_slice(
                            [x, 400.0]
                                // [slot_offset + (pos.x * slot_width), pos.y]
                                .as_bytes(),
                        );
                        // let x = slot_offset + (pos.x * view_len as f32);

                        // out[0..8].clone_from_slice([x, pos.y].as_bytes());
                        out[8..16]
                            .clone_from_slice([s as u32, l as u32].as_bytes());
                        out[16..32].clone_from_slice(color.as_bytes());
                        out
                    },
                ));

                /*
                self.labels.iter().enumerate().map(|(ix, &(s, l))| {
                    let pos = self.label_pos[ix];

                    let color = if self.label_flag[ix] & 16 == 0 {
                        [0.0f32, 0.0, 0.0, 1.0]
                    } else {
                        [1.0f32, 1.0, 1.0, 1.0]
                    };

                    let mut out = [0u8; 8 + 8 + 16];

                    let x = (slot_offset + (pos.x * slot_width));

                    out[0..8].clone_from_slice(
                        [x, pos.y]
                            // [slot_offset + (pos.x * slot_width), pos.y]
                            .as_bytes(),
                    );
                    // let x = slot_offset + (pos.x * view_len as f32);

                    // out[0..8].clone_from_slice([x, pos.y].as_bytes());
                    out[8..16]
                        .clone_from_slice([s as u32, l as u32].as_bytes());
                    out[16..32].clone_from_slice(color.as_bytes());
                    out
                }),
                */
                // )?;
            }

            //

            Ok(())
        })?;

        Ok(())
    }
}

pub struct LabelLayout {
    pub label_space: LabelSpace,
    labels: Vec<(usize, usize)>,

    label_anchors: Vec<f32>,

    label_pos: Vec<Pos2>,
    label_size: Vec<Vec2>,
    label_vel: Vec<Vec2>,

    label_flag: Vec<u64>,

    layout_width: f32,
    layout_height: f32,

    t: f32,

    layer_name: rhai::ImmutableString,

    sublayer_rect: rhai::ImmutableString,
    sublayer_text: rhai::ImmutableString,
}

impl LabelLayout {
    pub fn step(&mut self, slot_width: f32, dt: f32) {
        // let x_mult = width / self.layout_width;

        // self.layout_width = width;

        // let x0 = self.layout_width / 2.0;
        let y0 = 250.0;

        self.t += dt;

        let t = self.t;

        let mut forces = vec![(0.0f32, false); self.label_pos.len()];

        let mut fall_count = 0;

        for ((ix, pos), size) in
            self.label_pos.iter().enumerate().zip(&self.label_size)
        {
            self.label_flag[ix] = 0;
            let at_top = pos[1] == y0;
            let mut ddy = 0.0;

            let mut none_above = true;

            let mut free_above = true;

            let mut min_above = std::f32::INFINITY;

            let a_l = pos.x * slot_width;
            let a_u = pos.y;
            let a_r = a_l + size.x;
            let a_d = a_u + size.y;

            let mut no_collides = true;

            for ((i_ix, other), other_size) in self
                .label_pos
                .iter()
                .enumerate()
                .zip(&self.label_size)
                .skip(ix)
            {
                if i_ix == ix {
                    continue;
                }

                let b_l = other.x * slot_width;
                let b_u = other.y;
                let b_r = b_l + other_size.x;
                let b_d = b_u + other_size.y;

                let other_above = a_l < b_r && a_r > b_l && a_u > b_d;

                if other_above {
                    free_above = false;
                }

                let collides = other_above && a_d < b_u;

                if !collides {
                    continue;
                }

                no_collides = false;

                self.label_flag[ix] |= 1;

                // if other.y < pos.y {
                //     none_above = false;
                // }

                let a_mid = a_u + 4.0;
                let b_mid = b_u + 4.0;

                let d = (4.0 - (a_mid - b_mid).abs()) * 8.0;

                if a_u > b_u {
                    ddy += d;
                } else {
                    ddy -= d;
                }

                /*
                if d.abs() > 0.1 {
                    ddy -= d * 8.0;
                } else {
                    if d
                    ddy -= d * 8.0;
                }
                */

                // let v = (a_mid - b_mid).abs();
            }

            if none_above {
                self.label_flag[ix] |= 2;
                // ddy = -12.0 * dt;
                ddy = -8.0;
                fall_count += 1;
            }

            if free_above {
                self.label_flag[ix] |= 8;
            }

            if at_top {
                self.label_flag[ix] |= 4;
                ddy = 0.0;
            }

            // if no_collides {
            //     self.label_flag[ix] |= 16;
            // }

            forces[ix] = (ddy, none_above);
        }

        log::warn!("fall count: {}", fall_count);

        for (ix, ((pos, vel), (acc, none_above))) in self
            .label_pos
            .iter_mut()
            .zip(self.label_vel.iter_mut())
            .zip(forces)
            .enumerate()
        {
            let mut x = pos.x;
            let mut y = pos.y;
            let dx = vel.x;
            let mut dy = vel.y;
            // let [dx, mut dy] = *vel;
            dy += acc * dt;
            dy *= 0.99999;

            // let at_top = pos == y0;
            if pos.y <= 250.0 || !none_above {
                dy = dy.max(0.0);
            }

            if self.label_flag[ix] & 8 != 0 {
                dy = 0.0;
                y = 250.0;
            }
            // dy += t.cos();
            // dy += acc;
            /*
            if none_above {
            } else {
                dy = dy.max(0.0);
            }
            */
            // x = x_offset ;
            // x *= x_mult;

            *vel = vector![dx, dy];
            *pos = point![x + dx * dt, (y + dy * dt).max(250.0)];
            // *pos = [x + dx, y + dy];
            // *vel = vector![d
            // *vel = [dx, dy];
        }
    }

    pub fn set_dims(&mut self, width: f32, height: f32) {
        self.layout_width = width;
        self.layout_height = height;
    }

    pub fn randomize_pos_radial<R: rand::Rng>(&mut self, rng: &mut R) {
        let origin = [self.layout_width / 2.0, self.layout_height / 2.0];

        for pos in self.label_pos.iter_mut() {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let mag = rng.gen_range(0.0..self.layout_width);

            let x = origin[0] + mag * angle.cos();
            let y = origin[1] + mag * angle.sin();
            *pos = point![x, y];
        }
    }

    pub fn reset_for_view<R: rand::Rng>(
        &mut self,
        rng: &mut R,
        view: &ViewDiscrete1D,
        layout_width: f32,
    ) {
        log::warn!("resetting layout!");
        self.layout_width = layout_width;

        let view_scale = (view.max as f32) / view.len as f32;
        let offset = (view.offset as f32) / view.max as f32;

        for (i, anchor) in self.label_anchors.iter().enumerate() {
            let (_, t_len) = self.labels[i];

            let x = (anchor - offset) * view_scale;

            // let y = 250.0 + rng.gen_range(0.0..80.0);

            let y = 400.0 + rng.gen_range(0.0..200.0);
            let w = 8.0 * t_len as f32;

            self.label_pos[i] = point![x, y];
            self.label_vel[i] = vector![0.0, 0.0];
            // let x = (anchor * layout_width)
            //
        }
    }

    pub fn from_iter<'a, I>(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        layout_width: f32,
        layout_height: f32,
        labels: I,
    ) -> Result<Self>
    where
        I: IntoIterator<Item = (&'a str, f32, f32)> + 'a,
    {
        let mut label_space =
            LabelSpace::new(engine, "layout-labels", 1024 * 1024)?;

        let mut label_anchors = Vec::new();
        let mut labels_vec = Vec::new();
        let mut label_pos = Vec::new();
        let mut label_vel = Vec::new();
        let mut label_size = Vec::new();

        for (text, x, y) in labels.into_iter() {
            let (s, l) = label_space.bounds_for_insert(text)?;

            label_anchors.push(x);

            let w = 8.0 * l as f32;

            let pos = point![(x * layout_width) - w / 2.0, y];
            let vel = vector![0.0f32, 0.0];
            let size = vector![8.0 * l as f32, 8.0];
            labels_vec.push((s, l));
            label_pos.push(pos);
            label_vel.push(vel);
            label_size.push(size);
        }

        let label_flag = vec![0; label_anchors.len()];

        let layer_name = "label-layout-layer";
        let rect_name = "label-layout:rect";
        let text_name = "label-layout:text";

        compositor.new_layer(layer_name, 1, true);

        compositor.with_layer(layer_name, |layer| {
            Compositor::push_sublayer(
                &compositor.sublayer_defs,
                engine,
                layer,
                "rect-rgb",
                rect_name,
                None,
            )?;

            Compositor::push_sublayer(
                &compositor.sublayer_defs,
                engine,
                layer,
                "text",
                text_name,
                [label_space.text_set],
            )?;

            Ok(())
        })?;

        Ok(Self {
            label_space,
            labels: labels_vec,

            label_anchors,

            label_pos,
            label_size,
            label_vel,

            label_flag,

            layout_width,
            layout_height,

            t: 0.0,

            layer_name: layer_name.into(),

            sublayer_rect: rect_name.into(),
            sublayer_text: text_name.into(),
        })

        // let mut result = LabelLayout
    }

    pub fn update_layer(
        &mut self,
        compositor: &mut Compositor,
        slot_offset: f32,
        slot_width: f32,
        view_offset: usize,
        view_len: usize,
        max_len: usize,
    ) -> Result<()> {
        // let view_s = (view_len as f32) / (max_len as f32);
        // let x_scale = slot_width * view_s;

        // let x_offset = slot_offset + (view_offset as f32
        // let scale = slot_width /

        compositor.with_layer(&self.layer_name, |layer| {
            /*
            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_text)
            {
                sublayer.update_vertices_array(
                    self.labels.iter().enumerate().map(|(ix, &(s, l))| {
                        let pos = self.label_pos[ix];

                        let color = if self.label_flag[ix] & 16 == 0 {
                            [0.0f32, 0.0, 0.0, 1.0]
                        } else {
                            [1.0f32, 1.0, 1.0, 1.0]
                        };

                        let mut out = [0u8; 8 + 8 + 16];

                        let x = (slot_offset + (pos.x * slot_width));

                        out[0..8].clone_from_slice(
                            [x, pos.y]
                                // [slot_offset + (pos.x * slot_width), pos.y]
                                .as_bytes(),
                        );
                        // let x = slot_offset + (pos.x * view_len as f32);

                        // out[0..8].clone_from_slice([x, pos.y].as_bytes());
                        out[8..16]
                            .clone_from_slice([s as u32, l as u32].as_bytes());
                        out[16..32].clone_from_slice(color.as_bytes());
                        out
                    }),
                )?;
            }
            */

            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_rect)
            {
                /*
                let w = 4.0 + 8.0 * max_label_len as f32;
                let h = 4.0 + 8.0 * self.list.len() as f32;

                let mut bg = [0u8; 8 + 8 + 16];
                bg[0..8].clone_from_slice([x0, y0].as_bytes());
                bg[8..16].clone_from_slice([w, h].as_bytes());
                bg[16..32]
                    .clone_from_slice([0.85f32, 0.85, 0.85, 1.0].as_bytes());

                sublayer.update_vertices_array_range(0..1, [bg])?;
                sublayer.update_vertices_array(Some(bg).into_iter().chain(
                */

                sublayer.update_vertices_array(
                    self.label_flag
                        .iter()
                        .enumerate()
                        .filter(|(ix, flag)| **flag != 0)
                        .map(|(ix, flags)| {
                            // let r = (flags & 1 != 0)
                            //     .then(|| 1.0)
                            //     .unwrap_or_default();
                            let r = if flags & 1 != 0 { 0.7 } else { 0.0 };
                            let g = if flags & 2 != 0 { 0.7 } else { 0.0 };
                            let b = if flags & 4 != 0 { 0.7 } else { 0.0 };

                            let color = if self.label_flag[ix] & 16 == 0 {
                                [r, g, b, 1.0f32]
                            } else {
                                [1.0, 0.0, 0.0, 1.0]
                            };
                            // self.label_flag[ix] |= 16;
                            let pos = self.label_pos[ix];

                            let (_, t_len) = self.labels[ix];
                            let h = 10.0;
                            let w = 8.0 * t_len as f32;
                            // let w =

                            let x = (slot_offset + (pos.x * slot_width)) - 1.0;
                            let y = pos.y - 1.0;

                            let mut out = [0u8; 32];
                            out[0..8].clone_from_slice([x, y].as_bytes());
                            out[8..16].clone_from_slice([w, h].as_bytes());
                            out[16..32].clone_from_slice(color.as_bytes());
                            out
                        }),
                )?;
            }

            Ok(())
        })?;

        Ok(())
    }
}

#[export_module]
pub mod rhai_module {
    //
}
