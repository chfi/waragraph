use anyhow::Result;
use nalgebra::{vector, Vector2};
use raving::{
    compositor::{Compositor, SublayerAllocMsg},
    vk::{BufferIx, VkEngine},
};

use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

use crate::{geometry::graph::GraphLayout, graph::Waragraph};

#[derive(Debug, Clone, Copy)]
pub struct View2D {
    pub offset: ultraviolet::Vec2,
    pub scale: f32,
}

pub struct Viewer2D {
    pub layout: GraphLayout<(), ()>,
    ubo: BufferIx,

    pub view: Arc<AtomicCell<View2D>>,
    // offset: ultraviolet::Vec2,
    // scale: f32,
}

impl Viewer2D {
    pub const LAYER_NAME: &'static str = "graph-viewer-2d";

    pub fn create_index_buffer_for_path(
        engine: &mut VkEngine,
        graph: &Waragraph,
        path: crate::graph::Path,
    ) -> Result<(BufferIx, usize)> {
        let nodes = &graph.path_nodes[path.ix()];

        let path_name = graph
            .path_name(path)
            .expect("Path not found, this should be impossible!");

        let node_count = nodes.len() as usize;

        let buf = engine.with_allocators(|ctx, res, alloc| {
            let mem_loc = gpu_allocator::MemoryLocation::CpuToGpu;
            let usage = ash::vk::BufferUsageFlags::INDEX_BUFFER;
            // | vk::BufferUsageFlags::TRANSFER_DST;

            let buffer = res.allocate_buffer(
                ctx,
                alloc,
                mem_loc,
                std::mem::size_of::<u32>(),
                node_count,
                usage,
                Some(&format!("Index buffer - Path {}", path_name,)),
            )?;

            let buf_ix = res.insert_buffer(buffer);

            Ok(buf_ix)
        })?;

        Ok((buf, node_count))
    }

    pub fn new(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        graph: &Waragraph,
        layout_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        compositor.new_layer(Self::LAYER_NAME, 500, true);

        let sublayer_msg = SublayerAllocMsg::new(
            Self::LAYER_NAME.into(),
            "nodes".into(),
            "nodes_polyline".into(),
            &[],
        );
        compositor.sublayer_alloc_tx.send(sublayer_msg)?;

        compositor.allocate_sublayers(engine)?;

        let layout = GraphLayout::load_layout_tsv(graph, layout_path)?;

        let ubo =
            layout.prepare_sublayer(engine, compositor, Self::LAYER_NAME)?;

        let (p0, p1) = layout.aabb;

        let center = p0 + (p1 - p0) / 2.0;

        let offset = ultraviolet::Vec2 {
            x: center.x,
            y: center.y,
        };

        let scale = 20.0;
        // let scale = 1.0;

        let view = View2D { offset, scale };

        let mut viewer = Self {
            layout,
            ubo,

            view: Arc::new(view.into()),
        };

        /*
        let buf = &mut engine.resources[ubo];

        crate::geometry::graph::sublayer::write_uniform_buffer(
            buf, offset, scale,
        )
        .unwrap();

        viewer
            .layout
            .update_layer(compositor, Self::LAYER_NAME)?;
            */

        Ok(viewer)
    }

    pub fn update_view<F>(&self, f: F)
    where
        F: Fn(&mut View2D),
    {
        let mut view = self.view.load();
        log::warn!("old view: {:?}", view.offset);
        f(&mut view);
        log::warn!("new view: {:?}", view.offset);
        self.view.store(view);
    }

    pub fn update(
        &mut self,
        engine: &mut VkEngine,
        compositor: &mut Compositor,
    ) -> Result<()> {
        let buf = &mut engine.resources[self.ubo];

        let dims = compositor.window_dims();

        let view = self.view.load();

        log::warn!("updating graph view with offset {:?}", view.offset);

        crate::geometry::graph::sublayer::write_uniform_buffer(
            buf,
            dims,
            view.offset,
            view.scale,
        )
        .unwrap();

        self.layout.update_layer(compositor, Self::LAYER_NAME)?;

        Ok(())
    }

    /*
    pub fn set_view_offset(&self, offset: ultraviolet::Vec2) {
        self.offset.store(offset);
    }

    pub fn translate_view(&self, delta: ultraviolet::Vec2) {
        let mut offset = self.offset.load();
        self.offset += delta;
    }

    pub fn zoom_view(&self, scale_mult: f32) {
        self.scale *= scale_mult;
        self.scale = self.scale.max(1.0);
    }
    */
}
