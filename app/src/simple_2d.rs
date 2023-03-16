/*
A simpler 2D graph viewer, designed for viewing subgraphs
*/

use bimap::{BiBTreeMap, BiHashMap};
use raving_wgpu::{gui::EguiCtx, WindowState};
use sprs::{TriMat, TriMatI};
use ultraviolet::Vec2;

use anyhow::Result;
use waragraph_core::graph::{matrix::MatGraph, Node, OrientedNode, PathIndex};

use crate::{
    app::{AppWindow, SharedState},
    viewer_2d::view::View2D,
};

pub struct SimpleLayout {
    positions: Vec<Vec2>,
    incoming_angles: Vec<Vec<f32>>,
    outgoing_angles: Vec<Vec<f32>>,
    // seg_vx_map: BTreeM
    // M
}

struct Vertex {
    vx_id: usize,
    segment: Node,
    depth: f32,
    incoming: Vec<usize>,
    outgoing: Vec<usize>,
}

struct Edge {
    base_length: f32,
}

pub struct SimpleGraph {
    index_map: BiBTreeMap<Node, usize>,

    graph: MatGraph<Vertex, Edge>,
}

impl SimpleGraph {
    pub fn from_subgraph<N: Into<Node>>(
        index: &PathIndex,
        nodes: impl IntoIterator<Item = N>,
    ) -> Result<Self> {
        //

        let mut index_map = BiBTreeMap::default();
        let mut vertices = Vec::new();

        for node in nodes.into_iter().map(N::into) {
            let vx_id = vertices.len();
            index_map.insert(node, vx_id);

            let depth_u: usize = index
                .path_node_sets
                .iter()
                .map(|node_set| node_set.contains(node.into()) as usize)
                .sum();

            let depth = depth_u as f32;

            let vx = Vertex {
                vx_id,
                segment: node,
                depth,
                incoming: vec![],
                outgoing: vec![],
            };

            vertices.push(vx);
        }

        let mut edges = Vec::new();

        index
            .edges_iter()
            .filter_map(|edge| {
                let vf = *index_map.get_by_left(&edge.from.node())?;
                let vt = *index_map.get_by_left(&edge.to.node())?;
                Some((vf, vt))
            })
            .enumerate()
            .for_each(|(ix, (vf, vt))| {
                // add outgoing to `from`
                // add incoming to `to`
                vertices[vf].outgoing.push(vt);
                vertices[vt].incoming.push(vf);

                edges.push((vf, vt));
            });

        let vn = vertices.len();
        let en = edges.len();

        let mut adj: TriMat<u8> = TriMat::new((vn, vn));
        let mut inc: TriMat<u8> = TriMat::new((vn, en));

        for (i, &(from, to)) in edges.iter().enumerate() {
            adj.add_triplet(from, to, 1);
            adj.add_triplet(to, from, 1);
            inc.add_triplet(from, i, 1);
            inc.add_triplet(to, i, 1);
        }

        let edges = edges
            .into_iter()
            .map(|_| Edge { base_length: 1.0 })
            .collect::<Vec<_>>();

        let graph = MatGraph {
            vertex_count: vertices.len(),
            edge_count: edges.len(),
            adj: adj.to_csc(),
            inc: inc.to_csr(),
            vertex: vertices,
            edge: edges,
        };

        Ok(SimpleGraph { index_map, graph })
    }
}

pub struct Simple2D {
    graph: SimpleGraph,

    // vertices:
    view: View2D,

    // node positions
    //
    shared: SharedState,
}

impl Simple2D {
    pub fn init_with_subgraph(
        state: &raving_wgpu::State,
        window: &WindowState,
        shared: &SharedState,
    ) -> Result<Self> {
        let graph = shared.graph;
        let shared = shared.clone();

        todo!();
    }
}

impl AppWindow for Simple2D {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        dt: f32,
    ) {
        egui_ctx.begin_frame(&window.window);

        {
            let ctx = egui_ctx.ctx();

            let main_area = egui::Area::new("main_area_2d")
                .fixed_pos([0f32, 0.0])
                .movable(false)
                .constrain(true);

            let screen_rect = ctx.available_rect();
            let dims = Vec2::new(screen_rect.width(), screen_rect.height());

            main_area.show(ctx, |ui| {
                ui.set_width(screen_rect.width());
                ui.set_height(screen_rect.height());

                let area_rect = ui
                    .allocate_rect(screen_rect, egui::Sense::click_and_drag());

                if area_rect.dragged_by(egui::PointerButton::Primary) {
                    let delta =
                        Vec2::from(mint::Vector2::from(area_rect.drag_delta()));
                    let mut norm_delta = -1.0 * (delta / dims);
                    norm_delta.y *= -1.0;
                    self.view.translate_size_rel(norm_delta);
                }

                let painter = ui.painter();
                // painter.extend(annot_shapes);
            });
        }

        egui_ctx.end_frame(&window.window);

        // todo!();
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        // TODO: only using egui for now
        Ok(())
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        false
        // todo!()
    }

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
