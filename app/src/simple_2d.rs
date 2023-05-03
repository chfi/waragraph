/*
A simpler 2D graph viewer, designed for viewing subgraphs
*/

use std::collections::HashSet;

use bimap::{BiBTreeMap, BiHashMap};
use raving_wgpu::{gui::EguiCtx, WindowState};
use sprs::{TriMat, TriMatI};
use ultraviolet::Vec2;

use anyhow::Result;
use waragraph_core::graph::{matrix::MatGraph, Node, OrientedNode, PathIndex};

use crate::{
    app::{AppWindow, SharedState},
    context::ContextState,
    viewer_2d::view::View2D,
};

pub struct SimpleLayout {
    positions: Vec<Vec2>,

    aabb: (Vec2, Vec2),
    // incoming_angles: Vec<Vec<f32>>,
    // outgoing_angles: Vec<Vec<f32>>,
    // seg_vx_map: BTreeM
    // M
}

impl SimpleLayout {
    fn place_outgoing(
        p0: Vec2,
        existing: &[(Vec2, f32)],
        to_place: &[(usize, f32)],
    ) -> Vec<f32> {
        let mut placed = Vec::with_capacity(to_place.len());

        /*
         */

        for (i, &(vx, vx_d)) in to_place.iter().enumerate() {
            //
        }

        placed
    }
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
    pub fn initialize_layout(&self) -> SimpleLayout {
        // lay the nodes out according to a path-weighted DFS

        // let mut positions = Vec::with_capacity(self.graph.vertex_count);
        let mut positions: Vec<Option<Vec2>> =
            vec![None; self.graph.vertex_count];

        let mut init_nodes = self
            .graph
            .vertex
            .iter()
            .filter(|v| v.incoming.is_empty())
            .map(|v| v.vx_id)
            .collect::<Vec<_>>();

        let base_dist = 100f32;

        let mut aabb_min = Vec2::broadcast(f32::MAX);
        let mut aabb_max = Vec2::broadcast(f32::MIN);

        let mut update_aabb = |p: Vec2| {
            aabb_min = aabb_min.min_by_component(p);
            aabb_max = aabb_max.max_by_component(p);
        };

        // place the initial nodes first
        for (row, &vxi) in init_nodes.iter().enumerate() {
            let x = 0f32;
            let y = (row as f32) * base_dist;
            let p = Vec2::new(x, y);
            update_aabb(p);
            positions[vxi] = Some(p);
        }

        // let mut vx_by_depth = self
        //     .graph
        //     .vertex
        //     .iter()
        //     .map(|v| v.depth)
        //     .enumerate()
        //     .collect::<Vec<_>>();
        // vx_by_depth.sort_by(|(_, di), (_, dj)| di.partial_cmp(dj).unwrap());

        let mut stack: Vec<usize> = Vec::new();
        let mut visited: HashSet<usize> = HashSet::default();

        for vxi in init_nodes {
            stack.push(vxi);

            while let Some(vx) = stack.pop() {
                if !visited.contains(&vx) {
                    visited.insert(vx);

                    // let vx_pos
                    let vx_pos = if let Some(p) = positions[vx] {
                        p
                    } else {
                        unreachable!();
                    };

                    let neighbors = self
                        .graph
                        .neighbors(vx)
                        .into_iter()
                        .filter(|n| !visited.contains(n));

                    let mut to_add: Vec<(usize, f32)> = Vec::new();
                    let mut to_follow: Vec<(usize, f32)> = Vec::new();

                    let mut placed_neighbors: Vec<(Vec2, f32)> = Vec::new();

                    for next in neighbors {
                        let depth = self.graph.vertex[next].depth;
                        // place the neighbors (if not placed)
                        if let Some(p) = positions[next] {
                            placed_neighbors.push((p, depth));
                        } else {
                            to_add.push((next, depth));
                        }
                        // & recurse (if not visited)
                        if !visited.contains(&next) {
                            to_follow.push((next, depth));
                        }
                    }

                    to_follow.sort_by(|(_, di), (_, dj)| {
                        di.partial_cmp(dj).unwrap()
                    });

                    //
                }
                //
            }
        }

        /*
        for (vx_id, vertex) in self.graph.vertex.iter().enumerate() {
        }
        */

        let pos_len = positions.len();

        let positions =
            positions.into_iter().filter_map(|p| p).collect::<Vec<_>>();
        assert_eq!(positions.len(), pos_len);

        let _ = update_aabb;

        let aabb = (aabb_min, aabb_max);

        let layout = SimpleLayout { positions, aabb };

        layout
    }

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
        let graph = &shared.graph;
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
        context_state: &mut ContextState,
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
