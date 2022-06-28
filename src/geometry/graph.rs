use std::{
    num::NonZeroI32,
    ops::{Add, Div, Sub},
};

use bstr::ByteSlice;
use euclid::*;
use nalgebra::{Norm, Normed};
use num_traits::{FromPrimitive, ToPrimitive};
use raving::compositor::Compositor;
use rustc_hash::{FxHashMap, FxHashSet};

use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::{BufferIx, DescSetIx, GpuResources, VkEngine};

use ash::vk;

use std::sync::Arc;

use anyhow::Result;

use ndarray::prelude::*;

use crate::graph::{Node, Path, Waragraph};

use super::{ScreenPoint, ScreenRect, ScreenSize, ScreenSpace};

use nalgebra::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrientedNode(NonZeroI32);

pub struct GraphLayout<N, E> {
    vertices: Vec<Point2<f32>>,
    edges: Vec<(usize, usize)>,

    node_data: Vec<N>,
    edge_data: Vec<E>,
}

impl<N, E> GraphLayout<N, E> {
    pub fn load_layout_tsv<P: AsRef<std::path::Path>>(
        graph: &Waragraph,
        tsv_path: P,
    ) -> Result<Self> {
        use std::fs::File;
        use std::io::prelude::*;
        use std::io::BufReader;

        let mut vertices = Vec::new();
        let mut edges = Vec::new();

        let layout_file = File::open(tsv_path)?;
        let mut reader = BufReader::new(layout_file);

        let mut line_buf = String::new();

        // skip header
        let _ = reader.read_line(&mut line_buf)?;

        loop {
            line_buf.clear();
            let len = reader.read_line(&mut line_buf)?;

            if len == 0 {
                break;
            }

            let line = line_buf[..len].trim();

            let mut fields = line.split_whitespace();

            let fields = fields.next().and_then(|ix_s| {
                let x_s = fields.next()?;
                let y_s = fields.next()?;
                Some((ix_s, x_s, y_s))
            });

            if let Some((ix, x, y)) = fields {
                let _ix = ix.parse::<usize>()?;
                let x = x.parse::<f32>()?;
                let y = y.parse::<f32>()?;

                let p = point![x, y];
                vertices.push(p);
            }
        }

        for (a, b) in graph.edges.keys() {
            let ai = a.node().ix() * 2;
            let bi = b.node().ix() * 2;

            let (a_ix, b_ix): (usize, usize) =
                match (a.is_reverse(), b.is_reverse()) {
                    (false, false) => {
                        // (a+, b+)
                        (ai + 1, bi)
                    }
                    (false, true) => {
                        // (a+, b-)
                        (ai + 1, bi + 1)
                    }
                    (true, false) => {
                        // (a-, b+)
                        (ai, bi)
                    }
                    (true, true) => {
                        // (a-, b-)
                        (ai, bi + 1)
                    }
                };

            edges.push((a_ix, b_ix));
        }

        // let mut lines = reader.lines();
        // throw away header
        // lines.next().unwrap()?;

        let result = Self {
            vertices,
            edges,

            node_data: Vec::new(),
            edge_data: Vec::new(),
        };

        Ok(result)
    }
}

impl From<Node> for OrientedNode {
    fn from(node: Node) -> OrientedNode {
        Self::new(node.into(), false)
    }
}

impl From<u32> for OrientedNode {
    fn from(id: u32) -> OrientedNode {
        Self::new(id, false)
    }
}

impl From<OrientedNode> for Node {
    fn from(onode: OrientedNode) -> Node {
        onode.node()
    }
}

impl OrientedNode {
    pub fn id(&self) -> u32 {
        (self.0.get() - 1) as u32
    }

    pub fn node(&self) -> Node {
        let id = self.id();
        Node::from(id)
    }

    pub fn is_reverse(&self) -> bool {
        // self.0.get().is_negative()
        self.0.leading_zeros() == 0
    }

    pub fn new(node_id: u32, is_reverse: bool) -> Self {
        let mut id = 1 + node_id as i32;

        if is_reverse {
            id *= -1;
        }

        let id = unsafe { NonZeroI32::new_unchecked(id) };

        Self(id)
    }

    pub fn flip(self) -> Self {
        let id = unsafe { NonZeroI32::new_unchecked(self.0.get() * -1) };

        Self(id)
    }
}

mod sublayer {

    use raving::compositor::SublayerDef;
    use zerocopy::AsBytes;

    use super::*;

    pub(super) fn sublayer_def(
        ctx: &VkContext,
        res: &mut GpuResources,
        clear_pass: vk::RenderPass,
        load_pass: vk::RenderPass,
    ) -> Result<SublayerDef> {
        let vert = res.load_shader(
            "shaders/polyline.vert.spv",
            vk::ShaderStageFlags::VERTEX,
        )?;

        let frag = res.load_shader(
            "shaders/vector.frag.spv",
            vk::ShaderStageFlags::FRAGMENT,
        )?;

        let vert = res.insert_shader(vert);
        let frag = res.insert_shader(frag);

        let vertex_stride = std::mem::size_of::<[f32; 6]>();

        let vert_binding_desc = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(vertex_stride as u32)
            .input_rate(vk::VertexInputRate::INSTANCE)
            .build();

        let pos_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0)
            .build();

        let size_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(8)
            .build();

        let ix_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R32G32_UINT)
            .offset(16)
            .build();

        let vert_binding_descs = [vert_binding_desc];
        let vert_attr_descs = [pos_desc, size_desc, ix_desc];

        let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vert_binding_descs)
            .vertex_attribute_descriptions(&vert_attr_descs);

        let vertex_offset = 0;

        let mut def = SublayerDef::new::<([f32; 2], [f32; 2], [u32; 2]), _>(
            ctx,
            res,
            "path-slot",
            vert,
            frag,
            clear_pass,
            load_pass,
            vertex_offset,
            vertex_stride,
            true,
            Some(6),
            None,
            vert_input_info,
            None,
            // [font_desc_set],
            None,
        )?;

        fn get_cast(map: &rhai::Map, k: &str) -> Option<f32> {
            let field = map.get(k)?;
            field
                .as_float()
                .ok()
                .or(field.as_int().ok().map(|v| v as f32))
        }

        def.set_parser(|map, out| {
            let x = get_cast(&map, "x")?;
            let y = get_cast(&map, "y")?;
            let w = get_cast(&map, "w")?;
            let h = get_cast(&map, "h")?;

            let o = map.get("o").and_then(|f| f.as_int().ok())?;
            let l = map.get("l").and_then(|f| f.as_int().ok())?;

            out[0..8].clone_from_slice([x, y].as_bytes());
            out[8..16].clone_from_slice([w, h].as_bytes());
            out[16..24].clone_from_slice([o as u32, l as u32].as_bytes());
            Some(())
        });

        Ok(def)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_oriented_node() {
        let fwd = OrientedNode::new(0, false);
        let rev = OrientedNode::new(0, true);

        let fwd1 = OrientedNode::new(172893, false);
        let rev1 = OrientedNode::new(172893, true);

        assert!(!fwd.is_reverse());
        assert!(rev.is_reverse());

        assert!(!fwd1.is_reverse());
        assert!(rev1.is_reverse());
    }
}
