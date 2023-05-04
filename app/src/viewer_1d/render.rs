use crate::app::settings_menu::SettingsWindow;
use crate::color::{ColorMap, ColorSchemeId};
use crate::util::BufferDesc;

use raving_wgpu::graph::dfrog::Graph;
use raving_wgpu::{NodeId, State, WindowState};

use anyhow::Result;

pub struct Renderer {
    render_graph: Graph,
    draw_path_slot: NodeId,
}

// contains all the config/info needed to render a data buffer
// sampled from the data source corresponding to `data_key`
#[derive(Clone)]
pub struct VizModeConfig {
    pub name: String,
    pub data_key: String,
    pub color_scheme: ColorSchemeId,
    pub default_color_map: ColorMap,
}

pub struct RendererState {
    vertices: BufferDesc,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,

    data_buffer: BufferDesc,
}

impl Renderer {
    pub fn init(
        state: &State,
        window: &WindowState, // needed for image format
        settings_window: &mut SettingsWindow,
    ) -> Result<Self> {
        let mut graph = Graph::new();

        let draw_schema = {
            let vert_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d.vert.spv"
            ));
            let frag_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d_color_map.frag.spv"
            ));

            let primitive = wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                // cull_mode: Some(wgpu::Face::Front),
                polygon_mode: wgpu::PolygonMode::Fill,

                strip_index_format: None,
                unclipped_depth: false,
                conservative: false,
            };

            graph.add_graphics_schema_custom(
                state,
                vert_src,
                frag_src,
                primitive,
                wgpu::VertexStepMode::Instance,
                ["vertex_in"],
                None,
                &[wgpu::ColorTargetState {
                    format: window.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::all(),
                }],
            )?
        };

        /*
        let (vert_uniform, frag_uniform) = {
            let data = [win_dims[0] as f32, win_dims[1] as f32];
            let usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST;

            let vert_uniform =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[data]),
                    usage,
                });

            let data = [1.0f32, 0.0];
            let usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST;
            let frag_uniform =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[data]),
                    usage,
                });

            (vert_uniform, frag_uniform)
        };
        */
        let draw_node = graph.add_node(draw_schema);
        graph.add_link_from_transient("vertices", draw_node, 0);
        graph.add_link_from_transient("swapchain", draw_node, 1);

        graph.add_link_from_transient("vert_cfg", draw_node, 2);
        // graph.add_link_from_transient("frag_cfg", draw_node, 3);

        graph.add_link_from_transient("viz_data_buffer", draw_node, 3);
        graph.add_link_from_transient("sampler", draw_node, 4);
        graph.add_link_from_transient("color_texture", draw_node, 5);
        graph.add_link_from_transient("color_map", draw_node, 6);
        graph.add_link_from_transient("transform", draw_node, 7);

        todo!();
    }
}
