use std::{
    num::NonZeroI32,
    ops::{Add, Div, Sub},
};

use bstr::ByteSlice;
use euclid::*;
use raving::{
    compositor::Compositor,
    vk::{BufferIx, VkEngine},
};

use anyhow::Result;

use crate::{
    graph::{Node, Path, Waragraph},
    gui::layer::{line_width_rgba, line_width_rgba2},
};

use super::{ScreenPoint, ScreenRect, ScreenSize, ScreenSpace};

use nalgebra::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrientedNode(NonZeroI32);

pub struct GraphLayout<N, E> {
    vertices: Vec<Point2<f32>>,
    edges: Vec<(usize, usize)>,

    pub aabb: (Point2<f32>, Point2<f32>),

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

        log::warn!("loading {:?}", tsv_path.as_ref());

        let mut vertices = Vec::new();
        let mut edges = Vec::new();

        let layout_file = File::open(tsv_path)?;
        let mut reader = BufReader::new(layout_file);

        let mut line_buf = String::new();

        // skip header
        let _ = reader.read_line(&mut line_buf)?;

        let mut min = point![std::f32::MAX, std::f32::MAX];
        let mut max = point![std::f32::MIN, std::f32::MIN];

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

                // TODO obviously don't scale here!
                // let scale = 0.05;

                max = point![max.x.max(x), max.y.max(y)];
                min = point![min.x.min(x), min.y.min(y)];

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

        log::debug!("loaded {} vertex positions", vertices.len());

        log::debug!("layout bounding box: min: {:?}\tmax: {:?}", min, max);

        let result = Self {
            vertices,
            edges,

            aabb: (min, max),

            node_data: Vec::new(),
            edge_data: Vec::new(),
        };

        Ok(result)
    }

    pub fn prepare_sublayer(
        &self,
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        layer_name: &str,
    ) -> Result<BufferIx> {
        let (buffer, set) = engine.with_allocators(|ctx, res, alloc| {
            sublayer::allocate_uniform_buffer(compositor, ctx, res, alloc)
        })?;

        let polyline_sublayer = "nodes";

        compositor.with_layer(layer_name, |layer| {
            if let Some(sublayer_data) = layer
                .get_sublayer_mut(polyline_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                sublayer_data.update_sets([set]);
            }

            Ok(())
        })?;

        Ok(buffer)
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
        let polyline_sublayer = "nodes";

        compositor.with_layer(layer_name, |layer| {
            if let Some(sublayer_data) = layer
                .get_sublayer_mut(rect_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                /*
                sublayer_data.update_vertices_array(
                    self.entities.iter().enumerate().map(|(i, e)| {
                        //
                        e.to_vertex()
                    }),
                )?;
                */
            }

            if let Some(sublayer_data) = layer
                .get_sublayer_mut(polyline_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                assert!(self.vertices.len() % 2 == 0);

                let mut min_x = std::f32::MAX;
                let mut min_y = std::f32::MAX;
                let mut max_x = std::f32::MIN;
                let mut max_y = std::f32::MIN;

                sublayer_data.update_vertices_array(
                    self.vertices.chunks_exact(2).map(|chnk| {
                        if let [back, front] = chnk {
                            let p = back;
                            let q = front;

                            /*

                            let c1 = Srgb::from_color(
                                lch_color.shift_hue((dist - tgt_len).abs()),
                            );
                            */

                            let dist = distance(p, q);

                            let c1 =
                                Srgb::from_color(lch_color.shift_hue(dist));

                            // Srgb::from_color(lch_color.shift_hue(d_a * 200.0));

                            // let color =
                            //     rgb::RGBA::new(c1.red, c1.green, c1.blue, 1.0);

                            let color = rgb::RGBA::new(0.8, 0.1, 0.1, 1.0);

                            let w = 4.0;

                            // let back = back - self.aabb.0;
                            // let front = front - self.aabb.0;

                            min_x = min_x.min(back.x).min(front.x);
                            min_y = min_y.min(back.y).min(front.y);

                            max_x = max_x.max(back.x).max(front.x);
                            max_y = max_y.max(back.y).max(front.y);

                            let p = point2(back.x, back.y);
                            let q = point2(front.x, front.y);

                            line_width_rgba2(p, q, w, w, color, color)
                        } else {
                            unreachable!();
                        }
                    }),
                )?;
            }

            Ok(())
        })?;

        Ok(())
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

pub mod sublayer {

    use gpu_allocator::vulkan::Allocator;
    use raving::compositor::SublayerDef;

    use raving::vk::context::VkContext;
    use raving::vk::{BufferIx, BufferRes, DescSetIx, GpuResources, VkEngine};

    use ash::vk;

    use zerocopy::AsBytes;

    use super::*;

    pub fn write_uniform_buffer(
        buf: &mut BufferRes,
        window_dims: [u32; 2],
        offset: ultraviolet::Vec2,
        scale: f32,
    ) -> Option<()> {
        let [w, h] = window_dims;

        let left = 0.0;
        let top = 0.0;
        let right = w as f32;
        let bottom = h as f32;

        let near = 1.0;
        let far = 1000.0;

        let proj = ultraviolet::projection::orthographic_vk(
            left, right, bottom, top, near, far,
        );

        let dst = buf.mapped_slice_mut()?;

        let mat_size = std::mem::size_of::<ultraviolet::Mat4>();

        dst[0..mat_size].clone_from_slice(bytemuck::cast_slice(&[proj]));
        dst[mat_size..(mat_size + 8)]
            .clone_from_slice(bytemuck::cast_slice(&[offset]));
        dst[(mat_size + 8)..(mat_size + 12)]
            .clone_from_slice(bytemuck::cast_slice(&[scale]));

        Some(())
    }

    /// Allocates the UBO that holds the layout offset and scale
    pub(super) fn allocate_uniform_buffer(
        compositor: &Compositor,
        ctx: &VkContext,
        res: &mut GpuResources,
        alloc: &mut Allocator,
    ) -> Result<(BufferIx, DescSetIx)> {
        let size =
            std::mem::size_of::<(ultraviolet::Mat4, ultraviolet::Vec2, f32)>();

        let size = VkEngine::aligned_ubo_size(ctx, size);

        let usage = vk::BufferUsageFlags::UNIFORM_BUFFER;
        let location = gpu_allocator::MemoryLocation::CpuToGpu;

        let buffer = res.allocate_buffer(
            ctx,
            alloc,
            location,
            1,
            size,
            usage,
            Some("polyline-uniform-buffer"),
        )?;

        let shader_ix = compositor
            .sublayer_defs
            .get("nodes_polyline")
            .and_then(|def| {
                let pipeline = &res[def.load_pipeline];
                pipeline.vertex
            })
            .unwrap();

        let desc_set =
            res.allocate_desc_set(shader_ix, 0, |res, builder| {
                let info = vk::DescriptorBufferInfo::builder()
                    .buffer(buffer.buffer)
                    .offset(0)
                    .range(ash::vk::WHOLE_SIZE)
                    .build();
                let buffer_info = [info];
                builder.bind_buffer(0, &buffer_info);
                Ok(())
            })?;

        let buffer = res.insert_buffer(buffer);
        let set = res.insert_desc_set(desc_set);

        Ok((buffer, set))
    }

    pub(crate) fn sublayer_def(
        ctx: &VkContext,
        res: &mut GpuResources,
        clear_pass: vk::RenderPass,
        load_pass: vk::RenderPass,
    ) -> Result<SublayerDef> {
        let vert = res.load_shader(
            "shaders/viewer_2d/nodes_polyline.vert.spv",
            vk::ShaderStageFlags::VERTEX,
        )?;

        let frag = res.load_shader(
            "shaders/viewer_2d/nodes_color_buf.frag.spv",
            vk::ShaderStageFlags::FRAGMENT,
        )?;

        let vert = res.insert_shader(vert);
        let frag = res.insert_shader(frag);

        let vertex_stride = std::mem::size_of::<[f32; 14]>();

        let vert_binding_desc = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(vertex_stride as u32)
            .input_rate(vk::VertexInputRate::INSTANCE)
            .build();

        let p0_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0)
            .build();


        let p0_width_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32_SFLOAT)
            .offset(8)
            .build();
    

        let p1_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(12)
            .build();

        let p1_width_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(3)
            .format(vk::Format::R32_SFLOAT)
            .offset(20)
            .build();

        let color0_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(4)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(24)
            .build();

        let color1_desc = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(5)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(40)
            .build();

        let vert_binding_descs = [vert_binding_desc];
        let vert_attr_descs = [p0_desc, p0_width_desc, p1_desc, p1_width_desc, color0_desc, color1_desc];

        let vert_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vert_binding_descs)
            .vertex_attribute_descriptions(&vert_attr_descs);

        let vertex_offset = 0;

        let mut def =
            SublayerDef::new::<([f32; 3], [f32; 3], [f32; 4], [f32; 4]), _>(
                ctx,
                res,
                "nodes_polyline",
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
