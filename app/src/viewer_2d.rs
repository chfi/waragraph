use crate::annotations::AnnotationStore;

use std::collections::HashMap;
use std::path::PathBuf;

use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::Window;

use raving_wgpu::camera::DynamicCamera2d;
use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State};

use wgpu::util::DeviceExt;

use anyhow::Result;

use ultraviolet::*;

use waragraph_core::graph::PathIndex;

use self::layout::{GraphPathCurves, PathCurveBuffers};

pub mod layout;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub tsv: PathBuf,
    pub annotations: Option<PathBuf>,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct GpuVertex {
    pos: [f32; 2],
    // tex_coord: [f32; 2],
}

struct PathRenderer {
    render_graph: Graph,
    egui: EguiCtx,

    path_index: PathIndex,
    graph_curves: layout::GraphPathCurves,
    // layout: GfaLayout,
    camera: DynamicCamera2d,

    graph_scalars: rhai::Map,

    uniform_buf: wgpu::Buffer,

    annotations: AnnotationStore,
    annotation_cache: Vec<(Vec2, String)>,

    path_curve_buffers: PathCurveBuffers,
    draw_node: NodeId,
}

fn draw_annotations(
    cache: &[(Vec2, String)],
    painter: &egui::Painter,
    window_dims: Vec2,
    camera: &DynamicCamera2d,
) {
    for (pos, text) in cache.iter() {
        let norm_p = camera.transform_world_to_screen(*pos);
        let size = window_dims;
        let p = norm_p * size;

        let anchor = egui::Align2::CENTER_CENTER;
        let font = egui::FontId::proportional(16.0);
        painter.text(
            egui::pos2(p.x, p.y),
            anchor,
            text,
            font,
            egui::Color32::WHITE,
        );
    }
}

impl PathRenderer {
    fn init(
        event_loop: &EventLoopWindowTarget<()>,
        state: &State,
        path_index: PathIndex,
        graph_curves: GraphPathCurves,
    ) -> Result<Self> {
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
                &[state.surface_format],
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

        let egui = EguiCtx::init(event_loop, state, None);

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

        let annotations = AnnotationStore::default();

        Ok(Self {
            render_graph: graph,
            egui,

            path_index,

            camera,
            graph_scalars: rhai::Map::default(),
            uniform_buf,
            annotations,
            annotation_cache: Vec::new(),
            path_curve_buffers,
            draw_node,

            graph_curves,
            // layout,
        })
    }
}

impl crate::AppWindow for PathRenderer {
    fn update(
        &mut self,
        _state: &raving_wgpu::State,
        window: &winit::window::Window,
        dt: f32,
    ) {
        // let touches = self
        //     .touch
        //     .take()
        //     .map(TouchOutput::flip_y)
        //     .collect::<Vec<_>>();

        self.egui.run(window, |ctx| {
            let painter = ctx.debug_painter();

            let origin = Vec2::new(40000.0, 180000.0);
            let norm_p = self.camera.transform_world_to_screen(origin);

            let size = window.inner_size();
            let size = Vec2::new(size.width as f32, size.height as f32);
            let p = norm_p * size;

            let stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
            let p = egui::pos2(p.x, p.y);

            let size = window.inner_size();
            let window_dims = Vec2::new(size.width as f32, size.height as f32);
            draw_annotations(
                &self.annotation_cache,
                &painter,
                window_dims,
                &self.camera,
            );
        });

        let any_touches = self.egui.ctx().input().any_touches();

        if any_touches {
            self.camera.stop();
        }

        // if self.egui.ctx().
        // if self.egui.ctx().
        // if !touches.is_empty() {
        //     self.camera.stop();
        // }

        self.camera.update(dt);

        let (delta, primary_down) = {
            let pointer = &self.egui.ctx().input().pointer;
            let delta = pointer.delta();
            let primary_down = pointer.primary_down();

            (delta, primary_down)
        };

        let win_size = {
            let s = window.inner_size();
            ultraviolet::Vec2::new(s.width as f32, s.height as f32)
        };

        let pos = self.egui.pointer_interact_pos();

        if let Some(touch) = self.egui.multi_touch() {
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
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        let mut consume = false;

        let resp = self.egui.on_event(event);

        // if self.touch.on_event(window_dims, event) {
        //     consume = true;
        // }

        consume
    }

    fn resize(
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

    fn render(&mut self, state: &mut raving_wgpu::State) -> anyhow::Result<()> {
        let dims = state.size;
        let size = [dims.width, dims.height];

        let mut transient_res: HashMap<String, InputResource<'_>> =
            HashMap::default();

        let buffers = &self.path_curve_buffers;

        if let Ok(output) = state.surface.get_current_texture() {
            {
                let uniform_data = self.camera.to_matrix();
                state.queue.write_buffer(
                    &self.uniform_buf,
                    0,
                    bytemuck::cast_slice(&[uniform_data]),
                );
            }

            let output_view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let format = state.surface_format;

            transient_res.insert(
                "swapchain".into(),
                InputResource::Texture {
                    size,
                    format,
                    texture: None,
                    view: Some(&output_view),
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

            let _sub_index = self
                .render_graph
                .execute(&state, &transient_res, &self.graph_scalars)
                .unwrap();

            let mut encoder = state.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("egui render"),
                },
            );

            self.egui.render(state, &output_view, &mut encoder);

            state.queue.submit(Some(encoder.finish()));

            // probably shouldn't be polling here, but the render graph
            // should probably not be submitting by itself, either:
            //  better to return the encoders that will be submitted
            state.device.poll(wgpu::MaintainBase::Wait);

            output.present();
        } else {
            state.resize(state.size);
        }

        Ok(())
    }
}

pub fn init(
    event_loop: &EventLoop<()>,
    window: &Window,
    state: &State,
    args: Args,
) -> Result<Box<dyn crate::AppWindow>> {
    let path_index = PathIndex::from_gfa(&args.gfa)?;
    let graph_curves = GraphPathCurves::from_path_index_and_layout_tsv(
        &path_index,
        &args.tsv,
    )?;

    let mut app =
        PathRenderer::init(&event_loop, &state, path_index, graph_curves)?;

    if let Some(bed) = args.annotations.as_ref() {
        app.annotations.fill_from_bed(bed)?;
        let cache = app
            .annotations
            .layout_positions(&app.path_index, &app.graph_curves);
        app.annotation_cache = cache;
    }

    Ok(Box::new(app))
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.free_from_os_str(parse_path)?,
        annotations: pargs.opt_value_from_os_str("--bed", parse_path)?,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}
