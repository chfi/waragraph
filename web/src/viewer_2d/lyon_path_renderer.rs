use anyhow::Result;

use lyon::lyon_tessellation::{
    BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexBuffers,
};
use lyon::math::{point, Point};
use lyon::path::{EndpointId, PathCommands};

use raving_wgpu::camera::DynamicCamera2d;
use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State, WindowState};

use std::collections::HashMap;
use std::io::{prelude::*, BufReader};
use std::sync::Arc;

use ultraviolet::*;

use wgpu::util::DeviceExt;

use egui_winit::winit;
use winit::event_loop::EventLoopWindowTarget;

use waragraph_core::graph::{Node, PathIndex};

use crate::app::AppWindow;
use crate::context::ContextState;

pub struct PathRenderer {
    render_graph: Graph,

    path_index: Arc<PathIndex>,
    graph_curves: GraphPathCurves,
    // layout: GfaLayout,
    camera: DynamicCamera2d,

    graph_scalars: rhai::Map,

    uniform_buf: wgpu::Buffer,

    // annotations: AnnotationStore,
    // annotation_cache: Vec<(Vec2, String)>,
    path_curve_buffers: PathCurveBuffers,
    draw_node: NodeId,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct GpuVertex {
    pos: [f32; 2],
    // tex_coord: [f32; 2],
}

impl PathRenderer {
    pub fn init(
        event_loop: &EventLoopWindowTarget<()>,
        state: &State,
        window: &WindowState,
        path_index: Arc<PathIndex>,
        layout_tsv: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        let graph_curves = GraphPathCurves::from_path_index_and_layout_tsv(
            &path_index,
            layout_tsv,
        )?;

        let mut graph = Graph::new();

        let draw_schema = {
            let vert_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/lyon.vert.spv"
            ));
            let frag_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/flat.frag.spv"
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
                wgpu::VertexStepMode::Vertex,
                ["vertex_in"],
                Some("indices"),
                &[wgpu::ColorTargetState {
                    format: window.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::all(),
                }],
            )?
        };

        let camera = {
            let center = Vec2::zero();
            let size = Vec2::new(4.0, 3.0);
            let (min, max) = graph_curves.aabb;
            let mut camera = DynamicCamera2d::new(center, size);
            camera.fit_region_keep_aspect(min, max);
            camera
        };

        let egui =
            EguiCtx::init(state, window.surface_format, event_loop, None);

        let uniform_data = camera.to_matrix();

        let uniform_buf = state.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniform_data]),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST,
            },
        );

        let draw_node = graph.add_node(draw_schema);

        graph.add_link_from_transient("vertices", draw_node, 0);
        graph.add_link_from_transient("indices", draw_node, 1);
        graph.add_link_from_transient("swapchain", draw_node, 2);

        // set 0, binding 0, transform matrix
        graph.add_link_from_transient("transform", draw_node, 3);

        let path_ids = 0..path_index.path_names.len();
        let path_curve_buffers =
            graph_curves.tessellate_paths(&state.device, path_ids)?;

        // let annotations = AnnotationStore::default();

        Ok(Self {
            render_graph: graph,

            path_index,

            camera,
            graph_scalars: rhai::Map::default(),
            uniform_buf,
            // annotations,
            // annotation_cache: Vec::new(),
            path_curve_buffers,
            draw_node,

            graph_curves,
            // layout,
        })
    }
}

impl AppWindow for PathRenderer {
    fn update(
        &mut self,
        _handle: &tokio::runtime::Handle,
        _state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        context_state: &mut ContextState,
        dt: f32,
    ) {
        /*
        dbg!();
        egui_ctx.run(&window.window, |ctx| {
            dbg!();
            let painter = ctx.debug_painter();

            let origin = Vec2::new(40000.0, 180000.0);
            let norm_p = self.camera.transform_world_to_screen(origin);

            let size = window.window.inner_size();
            let size = Vec2::new(size.width as f32, size.height as f32);
            let p = norm_p * size;

            let stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
            let p = egui::pos2(p.x, p.y);

            let window_dims = size;
            draw_annotations(
                &self.annotation_cache,
                &painter,
                window_dims,
                &self.camera,
            );
        });

        dbg!();

        let any_touches = egui_ctx.ctx().input().any_touches();

        if any_touches {
            self.camera.stop();
        }

        self.camera.update(dt);

        let (scroll, delta, primary_down) = {
            let input = &egui_ctx.ctx().input();
            let scroll = input.scroll_delta;
            let pointer = &input.pointer;
            let delta = pointer.delta();
            let primary_down = pointer.primary_down();

            (scroll, delta, primary_down)
        };

        let win_size = {
            let s = window.window.inner_size();
            ultraviolet::Vec2::new(s.width as f32, s.height as f32)
        };

        let pos = egui_ctx.pointer_interact_pos();

        if let Some(touch) = egui_ctx.ctx().multi_touch() {
            let t = touch.translation_delta;
            let z = 2.0 - touch.zoom_delta;
            let t = ultraviolet::Vec2::new(-t.x / win_size.x, t.y / win_size.y);

            self.camera.blink(t);
            self.camera.size *= z;
        } else if primary_down {
            let delta = ultraviolet::Vec2::new(
                -delta.x / win_size.x,
                delta.y / win_size.y,
            );
            self.camera.blink(delta);
        }
        dbg!();
        */
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        // TODO do stuff; currently handled in update() via egui

        false
    }

    fn on_resize(
        &mut self,
        _state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        let [ow, oh] = old_window_dims;
        let [nw, nh] = new_window_dims;

        let old = Vec2::new(ow as f32, oh as f32);
        let new = Vec2::new(nw as f32, nh as f32);

        let div = new / old;
        self.camera.resize_relative(div);

        Ok(())
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &WindowState,
        // output: &wgpu::SurfaceTexture,
        // window_dims: PhysicalSize<u32>,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let size: [u32; 2] = window.window.inner_size().into();

        let mut transient_res: HashMap<String, InputResource<'_>> =
            HashMap::default();

        let buffers = &self.path_curve_buffers;

        {
            let uniform_data = self.camera.to_matrix();
            state.queue.write_buffer(
                &self.uniform_buf,
                0,
                bytemuck::cast_slice(&[uniform_data]),
            );
        }

        let format = window.surface_format;

        transient_res.insert(
            "swapchain".into(),
            InputResource::Texture {
                size,
                format,
                texture: None,
                view: Some(&swapchain_view),
                sampler: None,
            },
        );

        let stride = 8;
        let v_size = stride * buffers.total_vertices;
        let i_size = 4 * buffers.total_indices;

        transient_res.insert(
            "vertices".into(),
            InputResource::Buffer {
                size: v_size,
                stride: Some(stride),
                buffer: &buffers.vertex_buffer,
            },
        );

        transient_res.insert(
            "indices".into(),
            InputResource::Buffer {
                size: i_size,
                stride: Some(4),
                buffer: &buffers.index_buffer,
            },
        );

        transient_res.insert(
            "transform".into(),
            InputResource::Buffer {
                size: 16 * 4,
                stride: None,
                buffer: &self.uniform_buf,
            },
        );

        self.render_graph.update_transient_cache(&transient_res);

        // log::warn!("validating graph");
        let valid = self
            .render_graph
            .validate(&transient_res, &self.graph_scalars)
            .unwrap();

        if !valid {
            log::error!("graph validation error");
        }

        /*
        self.render_graph.execute_with_encoder(
            &state,
            &transient_res,
            &self.graph_scalars,
            encoder,
        )?;
        */

        Ok(())
    }
}

pub struct GraphPathCurves {
    pub aabb: (Vec2, Vec2),
    endpoints: Vec<Point>,
    gfa_paths: Vec<PathCommands>,
}

pub struct PathCurveBuffers {
    pub(super) total_vertices: usize,
    pub(super) total_indices: usize,
    pub(super) vertex_buffer: wgpu::Buffer,
    pub(super) index_buffer: wgpu::Buffer,

    pub(super) path_indices: HashMap<usize, std::ops::Range<usize>>,
}

impl GraphPathCurves {
    pub(super) fn tessellate_paths(
        &self,
        device: &wgpu::Device,
        path_ids: impl IntoIterator<Item = usize>,
    ) -> Result<PathCurveBuffers> {
        let mut geometry: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
        let tolerance = 10.0;

        let opts = StrokeOptions::tolerance(tolerance).with_line_width(150.0);

        let mut stroke_tess = StrokeTessellator::new();

        let mut buf_build =
            BuffersBuilder::new(&mut geometry, |vx: StrokeVertex| GpuVertex {
                pos: vx.position().to_array(),
            });

        let mut path_indices = HashMap::default();

        for path_id in path_ids {
            let path = &self.gfa_paths[path_id];
            let slice = path.path_slice(&self.endpoints, &self.endpoints);

            let ixs_start = buf_build.buffers().indices.len();

            stroke_tess.tessellate_with_ids(
                path.iter(),
                &slice,
                None,
                &opts,
                &mut buf_build,
            )?;

            let ixs_end = buf_build.buffers().indices.len();

            path_indices.insert(path_id, ixs_start..ixs_end);
        }

        let vertices = geometry.vertices.len();
        let indices = geometry.indices.len();

        let vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&geometry.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&geometry.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        Ok(PathCurveBuffers {
            total_vertices: vertices,
            total_indices: indices,

            vertex_buffer,
            index_buffer,

            path_indices,
        })
    }

    pub fn pos_for_node(&self, node: usize) -> Option<(Vec2, Vec2)> {
        let ix = node / 2;
        let a = *self.endpoints.get(ix)?;
        let b = *self.endpoints.get(ix + 1)?;
        Some((a.to_array().into(), b.to_array().into()))
    }

    pub fn from_path_index_and_layout_tsv(
        path_index: &PathIndex,
        tsv_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        use std::fs::File;
        // use std::io::{prelude::*, BufReader};
        let mut lines = File::open(tsv_path).map(BufReader::new)?.lines();

        let _header = lines.next();
        let mut positions = Vec::new();

        fn parse_row(line: &str) -> Option<Vec2> {
            let mut fields = line.split('\t');
            let _idx = fields.next();
            let x = fields.next()?.parse::<f32>().ok()?;
            let y = fields.next()?.parse::<f32>().ok()?;
            Some(Vec2::new(x, y))
        }

        let mut min = Vec2::broadcast(f32::MAX);
        let mut max = Vec2::broadcast(f32::MIN);

        for line in lines {
            let line = line?;
            if let Some(v) = parse_row(&line) {
                min = min.min_by_component(v);
                max = max.max_by_component(v);
                positions.push(v);
            }
        }
        let aabb = (min, max);

        let mut gfa_paths = Vec::with_capacity(path_index.path_names.len());

        for steps in path_index.path_steps.iter() {
            let mut builder = PathCommands::builder();

            let mut started = false;

            for &step in steps.iter() {
                let seg = step.node();
                let rev = step.is_reverse();
                let ix = seg.ix();
                let a = ix * 2;
                let b = a + 1;
                let mut pts = [a as u32, b as u32];
                if rev {
                    pts.reverse();
                }

                if !started {
                    builder.begin(EndpointId(pts[0]));
                    started = true;
                }
                pts.into_iter().for_each(|b| {
                    builder.line_to(EndpointId(b));
                });
            }
            builder.end(false);

            gfa_paths.push(builder.build());
        }

        let endpoints =
            positions.into_iter().map(|p| point(p.x, p.y)).collect();

        Ok(GraphPathCurves {
            aabb,
            endpoints,
            gfa_paths,
        })
    }
}
