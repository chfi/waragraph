use anyhow::Result;
use nalgebra::{vector, Vector2};
use raving::{
    compositor::{Compositor, SublayerAllocMsg},
    vk::{BufferIx, VkEngine},
};

use crate::{geometry::graph::GraphLayout, graph::Waragraph};

pub struct Viewer2D {
    layout: GraphLayout<(), ()>,
    ubo: BufferIx,

    offset: ultraviolet::Vec2,
    scale: f32,
}

impl Viewer2D {
    pub const LAYER_NAME: &'static str = "graph-viewer-2d";

    pub fn new(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
        graph: &Waragraph,
        layout_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        compositor.new_layer(Self::LAYER_NAME, 1, true);

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

        let offset = ultraviolet::Vec2 { x: 0.0, y: 0.0 };
        let scale = 1.0;

        let mut viewer = Self {
            layout,
            ubo,
            offset,
            scale,
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

    pub fn update(
        &mut self,
        engine: &mut VkEngine,
        compositor: &mut Compositor,
    ) -> Result<()> {
        let buf = &mut engine.resources[self.ubo];

        let dims = compositor.window_dims();

        crate::geometry::graph::sublayer::write_uniform_buffer(
            buf,
            dims,
            self.offset,
            self.scale,
        )
        .unwrap();

        self.layout.update_layer(compositor, Self::LAYER_NAME)?;

        Ok(())
    }
}
