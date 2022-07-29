use anyhow::Result;
use nalgebra::{vector, Vector2};
use raving::{
    compositor::Compositor,
    vk::{BufferIx, VkEngine},
};

use crate::{geometry::graph::GraphLayout, graph::Waragraph};

pub struct Viewer2D {
    layout: GraphLayout<(), ()>,
    ubo: BufferIx,

    offset: Vector2<f32>,
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
        let layout = GraphLayout::load_layout_tsv(graph, layout_path)?;

        let ubo = layout.prepare_sublayer(
            engine,
            compositor,
            Self::LAYER_NAME,
        )?;

        let offset = vector![0.0, 0.0];
        let scale = 1.0;

        let mut viewer = Self {
            layout,
            ubo,
            offset,
            scale,
        };

        let buf = &mut engine.resources[ubo];

        crate::geometry::graph::sublayer::write_uniform_buffer(
            buf, offset, scale,
        )
        .unwrap();

        viewer
            .layout
            .update_layer(compositor, Self::LAYER_NAME)?;

        Ok(viewer)
    }

    pub fn update(
        &mut self,
        engine: &mut VkEngine,
        compositor: &mut Compositor,
    ) -> Result<()> {
        let buf = &mut engine.resources[self.ubo];

        crate::geometry::graph::sublayer::write_uniform_buffer(
            buf, self.offset, self.scale,
        )
        .unwrap();

        self.layout
            .update_layer(compositor, Self::LAYER_NAME)?;

        Ok(())
    }
}
