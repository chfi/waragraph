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
    viewer::{
        gui::{layer::Compositor, tree_list::LabelSpace},
        DataSource, SlotFnCache, ViewDiscrete1D,
    },
};

use rhai::plugin::*;

use lazy_static::lazy_static;

use rand::prelude::*;

use nalgebra::{point, vector, Point2, Vector2};

pub type Pos2 = Point2<f32>;
pub type Vec2 = Vector2<f32>;

pub struct LabelLayout {
    pub label_space: LabelSpace,
    labels: Vec<(usize, usize)>,

    label_pos: Vec<Pos2>,
    label_size: Vec<Vec2>,

    label_vel: Vec<Vec2>,

    layout_width: f32,
    layout_height: f32,

    t: f32,

    layer_name: rhai::ImmutableString,

    sublayer_rect: rhai::ImmutableString,
    sublayer_text: rhai::ImmutableString,
}

impl LabelLayout {
    pub fn step(&mut self, width: f32, dt: f32) {
        let x_mult = width / self.layout_width;

        self.layout_width = width;

        let x0 = self.layout_width / 2.0;
        let y0 = self.layout_height / 2.0;

        self.t += dt;

        let t = self.t;

        let mut forces = vec![0.0f32; self.label_pos.len()];

        for ((ix, pos), size) in
            self.label_pos.iter().enumerate().zip(&self.label_size)
        {
            let at_top = pos[1] == y0;
            let mut ddy = 0.0;

            let a_l = pos.x;
            let a_u = pos.y;
            let a_r = a_l + size.x;
            let a_d = a_u + size.y;

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

                let b_l = other.x;
                let b_u = other.y;
                let b_r = b_l + other_size.x;
                let b_d = b_u + other_size.y;

                let collides = a_l < b_r && a_r > b_l && a_u < b_d && a_d > b_u;

                if !collides {
                    continue;
                }

                let a_mid = a_u + 4.0;
                let b_mid = b_u + 4.0;

                let v = (a_mid - b_mid).abs();

                ddy += v;
            }

            if at_top {
                ddy = 0.0;
            }

            forces[ix] = ddy;
        }

        for ((pos, vel), acc) in self
            .label_pos
            .iter_mut()
            .zip(self.label_vel.iter_mut())
            .zip(forces)
        {
            let mut x = pos.x;
            let y = pos.y;
            let dx = vel.x;
            let mut dy = vel.y;
            // let [dx, mut dy] = *vel;

            // dy += t.cos();
            // dy += acc;
            dy += acc;
            dy *= dt * 0.98;
            // x = x_offset ;
            x *= x_mult;

            *vel = vector![dx, dy];
            *pos = point![x + dx, y + dy];
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

        let mut labels_vec = Vec::new();
        let mut label_pos = Vec::new();
        let mut label_vel = Vec::new();
        let mut label_size = Vec::new();

        for (text, x, y) in labels.into_iter() {
            let (s, l) = label_space.bounds_for_insert(text)?;

            let w = 8.0 * l as f32;

            let pos = point![x - w / 2.0, y];
            let vel = vector![0.0f32, 0.0];
            let size = vector![8.0 * l as f32, 8.0];
            labels_vec.push((s, l));
            label_pos.push(pos);
            label_vel.push(vel);
            label_size.push(size);
        }

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

            label_pos,
            label_size,
            label_vel,

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
    ) -> Result<()> {
        compositor.with_layer(&self.layer_name, |layer| {
            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_text)
            {
                sublayer.update_vertices_array(
                    self.labels.iter().enumerate().map(|(i, &(s, l))| {
                        let pos = self.label_pos[i];

                        let color = [0.0f32, 0.0, 0.0, 1.0];
                        let mut out = [0u8; 8 + 8 + 16];
                        out[0..8].clone_from_slice(
                            [slot_offset + pos.x, pos.y].as_bytes(),
                        );
                        out[8..16]
                            .clone_from_slice([s as u32, l as u32].as_bytes());
                        out[16..32].clone_from_slice(color.as_bytes());
                        out
                    }),
                )?;

                // self.list.iter().enumerate().map(|(i, (text, v))| {
                //     let (s, l) =
                //         self.label_space.bounds_for_insert(text).unwrap();

                //     let h = 10.0;

                //     let x = x0;
                //     let y = y0 + h * i as f32;

                //     let color = [0.0f32, 0.0, 0.0, 1.0];

                //     let mut out = [0u8; 8 + 8 + 16];
                //     out[0..8].clone_from_slice([x, y].as_bytes());
                //     out[8..16]
                //         .clone_from_slice([s as u32, l as u32].as_bytes());
                //     out[16..32].clone_from_slice(color.as_bytes());
                //     out
                // }),
            }

            /*
            if let Some(sublayer) = layer.get_sublayer_mut(&self.sublayer_rect)
            {
                let w = 4.0 + 8.0 * max_label_len as f32;
                let h = 4.0 + 8.0 * self.list.len() as f32;

                let mut bg = [0u8; 8 + 8 + 16];
                bg[0..8].clone_from_slice([x0, y0].as_bytes());
                bg[8..16].clone_from_slice([w, h].as_bytes());
                bg[16..32]
                    .clone_from_slice([0.85f32, 0.85, 0.85, 1.0].as_bytes());

                sublayer.update_vertices_array_range(0..1, [bg])?;

                sublayer.update_vertices_array(Some(bg).into_iter().chain(
                    self.list.iter().enumerate().map(|(i, (s, v))| {
                        let color = if i % 2 == 0 {
                            [0.85f32, 0.85, 0.85, 1.0]
                        } else {
                            [0.75f32, 0.75, 0.75, 1.0]
                        };

                        let h = 10.0;

                        let x = x0;
                        let y = y0 + h * i as f32;

                        let mut out = [0u8; 32];
                        out[0..8].clone_from_slice([x, y].as_bytes());
                        out[8..16].clone_from_slice([w, h].as_bytes());
                        out[16..32].clone_from_slice(color.as_bytes());
                        out
                    }),
                ))?;
            }
            */

            Ok(())
        })?;

        Ok(())
    }
}

#[export_module]
pub mod rhai_module {
    //
}
