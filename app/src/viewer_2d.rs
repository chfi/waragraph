use crate::app::{AppWindow, SharedState, VizInteractions};
use crate::color::ColorMapping;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use raving_wgpu::camera::DynamicCamera2d;
use wgpu::BufferUsages;
use winit::event::WindowEvent;
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::Window;

// use raving_wgpu::camera::DynamicCamera2d;
use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State, WindowState};

use wgpu::util::{BufferInitDescriptor, DeviceExt};

use anyhow::Result;

use ultraviolet::*;

use waragraph_core::graph::PathIndex;

pub mod layout;
pub mod view;

pub mod lyon_path_renderer;

use layout::NodePositions;

use self::view::View2D;

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

pub struct Viewer2D {
    path_index: Arc<PathIndex>,

    node_positions: Arc<NodePositions>,
    vertex_buffer: wgpu::Buffer,
    instance_count: usize,

    // camera: DynamicCamera2d,
    view: View2D,

    transform_uniform: wgpu::Buffer,
    vert_config: wgpu::Buffer,

    render_graph: Graph,
    draw_node: NodeId,

    pub self_viz_interact: Arc<AtomicCell<VizInteractions>>,
    pub connected_viz_interact: Option<Arc<AtomicCell<VizInteractions>>>,

    shared: SharedState,

    active_viz_data_key: String,
    color_mapping: ColorMapping,
    data_buffer: wgpu::Buffer,
}

impl Viewer2D {
    pub fn init(
        state: &State,
        window: &WindowState,
        path_index: Arc<PathIndex>,
        layout_tsv: impl AsRef<std::path::Path>,
        shared: &SharedState,
    ) -> Result<Self> {
        let (node_positions, vertex_buffer, instance_count) = {
            let pos = NodePositions::from_layout_tsv(layout_tsv)?;

            // TODO: ideally the node IDs and positions would be
            // stored in different buffers
            let vertex_data = pos
                .iter_nodes()
                .enumerate()
                .map(|(ix, p)| {
                    let p = [p];
                    let ix = [ix as u32];
                    let pos: &[u8] = bytemuck::cast_slice(&p);
                    let id: &[u8] = bytemuck::cast_slice(&ix);
                    let mut out = [0u8; 4 * 5];
                    out[0..(4 * 4)].clone_from_slice(pos);
                    out[(4 * 4)..].clone_from_slice(id);
                    out
                })
                .collect::<Vec<_>>();

            let instance_count = vertex_data.len();

            let buffer = state.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Viewer2D Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertex_data),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            );

            (pos, buffer, instance_count)
        };

        let win_dims = {
            let [w, h]: [u32; 2] = window.window.inner_size().into();
            Vec2::new(w as f32, h as f32)
        };

        let (tl, br) = node_positions.bounds;
        let center = tl + 0.5 * (br - tl);
        let total_size = br - tl;

        let aspect = win_dims.x / win_dims.y;

        let cam_width = total_size.y * aspect;
        let size = Vec2::new(cam_width, total_size.y);

        let view = View2D::new(center, size);

        let mut graph = Graph::new();

        let draw_node_schema = {
            let vert_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/2d_rects.vert.spv"
            ));
            let frag_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_2d_color_map.frag.spv"
            ));

            let primitive = wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None, // TODO fix
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
                &[window.surface_format],
            )?
        };

        let (transform_uniform, vert_config) = {
            let usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST;

            let data = view.to_matrix();

            let transform =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[data]),
                    usage,
                });

            let node_width = 50f32;
            let data = [node_width, 0.0, 0.0, 0.0];

            let vert_config =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[data]),
                    usage,
                });

            (transform, vert_config)
        };

        let draw_node = graph.add_node(draw_node_schema);

        graph.add_link_from_transient("vertices", draw_node, 0);
        graph.add_link_from_transient("swapchain", draw_node, 1);

        graph.add_link_from_transient("transform", draw_node, 2);
        graph.add_link_from_transient("vert_cfg", draw_node, 3);

        graph.add_link_from_transient("node_data", draw_node, 4);
        graph.add_link_from_transient("color", draw_node, 5);
        graph.add_link_from_transient("color_mapping", draw_node, 6);

        let instances = instance_count as u32;
        println!("instance count: {instances}");
        println!("node count: {}", path_index.node_count);

        graph.set_node_preprocess_fn(draw_node, move |_ctx, op_state| {
            op_state.vertices = Some(0..6);
            op_state.instances = Some(0..instances);
        });

        let self_viz_interact =
            Arc::new(AtomicCell::new(VizInteractions::default()));
        let connected_viz_interact = None;

        let color_mapping = {
            let mut colors = shared.colors.blocking_write();

            let id = colors.get_color_scheme_id("spectral").unwrap();
            let scheme = colors.get_color_scheme(id);

            let color_range = 1..=(scheme.colors.len() as u32);
            let val_range = 0f32..=13.0;

            let mapping = ColorMapping::new(
                id,
                color_range,
                val_range,
                0,
                (scheme.colors.len() - 1) as u32,
            );

            // not really necessary to do here, but ensures it's ready
            let _buffer =
                colors.get_color_mapping_gpu_buffer(state, mapping).unwrap();

            mapping
        };

        let active_viz_data_key = "node_id".to_string();

        let data = shared
            .graph_data_cache
            .fetch_graph_data_blocking(&active_viz_data_key)
            .unwrap();

        let data_buffer = {
            let buffer_usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
            state.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Viewer 2D TEMPORARY data buffer"),
                contents: bytemuck::cast_slice(&data.node_data),
                usage: buffer_usage,
            })
        };

        Ok(Self {
            path_index,
            node_positions: Arc::new(node_positions),

            vertex_buffer,
            instance_count,

            view,

            transform_uniform,
            vert_config,

            render_graph: graph,
            draw_node,

            self_viz_interact,
            connected_viz_interact,

            shared: shared.clone(),

            color_mapping,
            active_viz_data_key,
            data_buffer,
        })
    }

    fn update_transform_uniform(&self, queue: &wgpu::Queue) {
        let data = self.view.to_matrix();
        queue.write_buffer(
            &self.transform_uniform,
            0,
            bytemuck::cast_slice(&[data]),
        );
    }

    fn update_vert_config_uniform(
        &self,
        queue: &wgpu::Queue,
        window_height: f32,
    ) {
        // in pixels
        let node_width = 80.0;

        let nw = node_width / window_height;

        let data: [f32; 4] = [nw, 0.0, 0.0, 0.0];
        queue.write_buffer(&self.vert_config, 0, bytemuck::cast_slice(&[data]));
    }
}

impl AppWindow for Viewer2D {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        dt: f32,
    ) {
        let other_interactions = self
            .connected_viz_interact
            .as_ref()
            .map(|i| i.take())
            .unwrap_or_default();

        let [width, height]: [u32; 2] = window.window.inner_size().into();
        let dims = ultraviolet::Vec2::new(width as f32, height as f32);

        let screen_rect = egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(dims.x, dims.y),
        );

        let mut annot_shapes = Vec::new();

        if let Some(node) = other_interactions.interact_node {
            let (n0, n1) = self.node_positions.node_pos(node);
            let mid = n0 + (n1 - n0) * 0.5;

            if other_interactions.clicked {
                self.view.center = mid;
            }

            let mat = self.view.to_viewport_matrix(dims);

            let p0 = mat * n0.into_homogeneous_point();
            let p1 = mat * n1.into_homogeneous_point();
            let pmid = mat * mid.into_homogeneous_point();

            let dist = (p1.xy() - p0.xy()).mag();

            let p0 = egui::pos2(p0.x, p0.y);
            let p1 = egui::pos2(p1.x, p1.y);
            let pmid = egui::pos2(pmid.x, pmid.y);

            if dist > 2.0 {
                let stroke = egui::Stroke::new(5.0, egui::Color32::RED);
                annot_shapes.push(egui::Shape::line(vec![p0, p1], stroke));
            } else {
                let stroke = egui::Stroke::new(2.0, egui::Color32::RED);
                annot_shapes
                    .push(egui::Shape::circle_stroke(pmid, 5.0, stroke));
            }
        }

        egui_ctx.begin_frame(&window.window);

        {
            let ctx = egui_ctx.ctx();

            let main_area = egui::Area::new("main_area_2d")
                .fixed_pos([0f32, 0.0])
                .movable(false)
                .constrain(true);

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
                painter.extend(annot_shapes);
            });

            if let Some(node) = other_interactions.interact_node {
                let text = format!("Node: {}", node.ix());
                egui::Window::new("Information")
                    .fixed_pos([20.0f32, 20.0])
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(text).size(20.0))
                    });
            }

            let scroll = ctx.input().scroll_delta;

            if let Some(pos) = ctx.pointer_interact_pos() {
                let min_scroll = 1.0;
                let factor = 0.01;

                if scroll.y.abs() > min_scroll {
                    let dz = 1.0 - scroll.y * factor;
                    let uvp = Vec2::new(pos.x, pos.y);
                    let mut norm = uvp / dims;
                    norm.y = 1.0 - norm.y;
                    self.view.zoom_with_focus(norm, dz);
                }
            }

            if let Some(touch) = egui_ctx.ctx().multi_touch() {
                let t = touch.translation_delta;
                let z = 2.0 - touch.zoom_delta;
                let t = ultraviolet::Vec2::new(-t.x / dims.x, t.y / dims.y);

                self.view.translate_size_rel(t);
                self.view.size *= z;
            }
        }

        egui_ctx.end_frame(&window.window);

        let height = window.window.inner_size().height as f32;

        self.update_transform_uniform(&state.queue);
        self.update_vert_config_uniform(&state.queue, height);
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        let mut consume = false;

        if let WindowEvent::KeyboardInput { input, .. } = event {
            if let Some(key) = input.virtual_keycode {
                use winit::event::ElementState;
                use winit::event::VirtualKeyCode as Key;
                let pressed = matches!(input.state, ElementState::Pressed);

                let mut translation = Vec2::zero();

                if pressed {
                    match key {
                        Key::Right => {
                            translation.x += 0.1;
                        }
                        Key::Left => {
                            translation.x -= 0.1;
                        }
                        Key::Up => {
                            translation.y += 0.1;
                        }
                        Key::Down => {
                            translation.y -= 0.1;
                        }
                        Key::Space => {
                            println!("resetting view");
                            // self.view.reset();
                            let (tl, br) = self.node_positions.bounds;
                            let center = tl + 0.5 * (br - tl);
                            let total_size = br - tl;

                            let [w, h] = window_dims;
                            let aspect = w as f32 / h as f32;

                            let cam_width = total_size.y * aspect;
                            let size = Vec2::new(cam_width, total_size.y);

                            self.view = View2D::new(center, size);
                        }
                        _ => (),
                    }
                }

                if translation.mag() > 0.0 {
                    self.view.translate_size_rel(translation);
                }
            }
        }

        consume
    }

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        let aspect = new_window_dims[0] as f32 / new_window_dims[1] as f32;
        self.view.set_aspect(aspect);
        Ok(())
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let size: [u32; 2] = window.window.inner_size().into();

        let mut transient_res: HashMap<String, InputResource<'_>> =
            HashMap::default();

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

        let v_stride = std::mem::size_of::<[f32; 4]>();
        transient_res.insert(
            "vertices".into(),
            InputResource::Buffer {
                size: self.instance_count * v_stride,
                stride: Some(v_stride),
                buffer: &self.vertex_buffer,
            },
        );

        transient_res.insert(
            "transform".into(),
            InputResource::Buffer {
                size: 16 * 4,
                stride: None,
                buffer: &self.transform_uniform,
            },
        );

        let (color_buf, color_buf_size, color_map_buf) = {
            let mut colors = self.shared.colors.blocking_write();
            let mapping = self.color_mapping;
            let id = mapping.color_scheme;

            let scheme = colors.get_color_scheme(id);
            let color_buf_size = scheme.required_buffer_size();
            let color_buf = colors.get_color_scheme_gpu_buffer(id).unwrap();

            let map_buf = colors
                .get_color_mapping_gpu_buffer(&state, mapping)
                .unwrap();

            (color_buf, color_buf_size, map_buf)
        };

        let data_buf_size =
            self.shared.graph.node_count * std::mem::size_of::<[f32; 4]>();

        transient_res.insert(
            "node_data".to_string(),
            InputResource::Buffer {
                size: data_buf_size,
                stride: None,
                buffer: &self.data_buffer,
            },
        );

        transient_res.insert(
            "color".to_string(),
            InputResource::Buffer {
                size: color_buf_size,
                stride: None,
                buffer: &color_buf,
            },
        );

        transient_res.insert(
            "color_mapping".to_string(),
            InputResource::Buffer {
                size: 24,
                stride: None,
                buffer: &color_map_buf,
            },
        );

        transient_res.insert(
            "vert_cfg".into(),
            InputResource::Buffer {
                size: 1 * 4,
                stride: None,
                buffer: &self.vert_config,
            },
        );

        self.render_graph.update_transient_cache(&transient_res);

        let valid = self
            .render_graph
            .validate(&transient_res, &rhai::Map::default())
            .unwrap();

        if !valid {
            log::error!("graph validation error");
        }

        self.render_graph
            .execute_with_encoder(
                &state,
                &transient_res,
                &rhai::Map::default(),
                encoder,
            )
            .unwrap();

        Ok(())
    }
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
