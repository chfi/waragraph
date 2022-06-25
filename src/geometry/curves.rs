use std::{
    num::NonZeroI32,
    ops::{Add, Div, Sub},
};

use euclid::*;
use nalgebra::{Norm, Normed};
use num_traits::{FromPrimitive, ToPrimitive};
use raving::compositor::Compositor;
use rustc_hash::FxHashMap;

use raving::compositor::label_space::LabelSpace;
use raving::vk::context::VkContext;
use raving::vk::{BufferIx, DescSetIx, GpuResources, VkEngine};

use ash::vk;

use std::sync::Arc;

use anyhow::Result;

use ndarray::prelude::*;

use crate::graph::{Node, Path, Waragraph};

use super::{ScreenPoint, ScreenRect, ScreenSize, ScreenSpace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurveId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrientedCurve(NonZeroI32);

impl From<OrientedCurve> for CurveId {
    fn from(oc: OrientedCurve) -> CurveId {
        CurveId((oc.0.get().abs() - 1) as u32)
    }
}

impl From<CurveId> for OrientedCurve {
    fn from(CurveId(id): CurveId) -> OrientedCurve {
        let id = unsafe { NonZeroI32::new_unchecked(1 + id as u32) };
        OrientedCurve(id)
    }
}

impl OrientedCurve {
    pub fn id(&self) -> CurveId {
        CurveId((self.0.get() - 1) as u32)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrientedNode(NonZeroI32);

impl From<Node> for OrientedNode {
    fn from(node: Node) -> OrientedNode {
        Self::new(node.0, false)
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

pub struct Curve<D> {
    id: CurveId,

    seq_len: usize,

    steps: Vec<OrientedNode>,

    node_set: roaring::RoaringBitmap,
    path_set: roaring::RoaringBitmap,

    fwd_adj: Vec<OrientedCurve>,
    rev_adj: Vec<OrientedCurve>,

    step_data: Vec<D>,
}

impl<D> Curve<D> {
    pub fn steps(&self) -> &[OrientedNode] {
        &self.steps
    }

    pub fn paths(&self) -> impl Iterator<Item = Path> + '_ {
        self.path_set.iter().map(|i| Path::from(i))
    }

    pub fn contains_node(&self, node: Node) -> bool {
        self.node_set.contains(node.ix() as u32)
    }

    pub fn contains_path(&self, path: Path) -> bool {
        self.path_set.contains(path.ix() as u32)
    }

    pub fn first(&self) -> Option<OrientedNode> {
        self.steps.first().copied()
    }

    pub fn last(&self) -> Option<OrientedNode> {
        self.steps.last().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn seq_len(&self) -> usize {
        self.seq_len
    }
}

pub struct GraphCurveMap {
    source: Arc<Waragraph>,
}

pub struct CurveNet {
    // curves:
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
