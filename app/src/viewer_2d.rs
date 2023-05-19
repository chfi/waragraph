use crate::annotations::{AnnotationId, GlobalAnnotationId};
use crate::app::settings_menu::SettingsWindow;
use crate::app::{AppWindow, SharedState};
use crate::color::ColorMap;
use crate::context::{ContextQuery, ContextState};
use crate::util::BufferDesc;
use crate::viewer_2d::config::Config;

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use raving_wgpu::camera::DynamicCamera2d;
use raving_wgpu::texture::Texture;
use tokio::sync::RwLock;
use wgpu::BufferUsages;
use winit::event::WindowEvent;

use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State, WindowState};

use wgpu::util::{BufferInitDescriptor, DeviceExt};

use anyhow::Result;

use ultraviolet::*;

use waragraph_core::graph::{Bp, Node, PathIndex};

pub mod annotations;
pub mod config;
pub mod control;
pub mod gui;
pub mod layout;
pub mod util;
pub mod view;

pub mod lyon_path_renderer;

use control::ViewControlWidget;

use layout::NodePositions;

use self::annotations::AnnotationLayer;
use self::view::View2D;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub tsv: PathBuf,
    pub annotations: Option<PathBuf>,
}

pub struct Viewer2D {
    node_positions: Arc<NodePositions>,
    vertex_buffer: wgpu::Buffer,
    instance_count: usize,

    view: View2D,

    transform_uniform: wgpu::Buffer,
    vert_config: wgpu::Buffer,

    geometry_bufs: GeometryBuffers,

    render_graph: Graph,
    draw_node: NodeId,

    shared: SharedState,

    annotation_layer: AnnotationLayer,

    active_viz_data_key: String,
    color_mapping: crate::util::Uniform<ColorMap, 16>,
    data_buffer: wgpu::Buffer,

    view_control_widget: control::ViewControlWidget,

    pub msg_tx: crossbeam::channel::Sender<control::Msg>,
    msg_rx: crossbeam::channel::Receiver<control::Msg>,

    cfg: Config,
}

impl Viewer2D {
    pub fn init(
        state: &State,
        window: &WindowState,
        path_index: Arc<PathIndex>,
        layout_tsv: impl AsRef<std::path::Path>,
        shared: &SharedState,
        settings_window: &mut SettingsWindow,
    ) -> Result<Self> {
        let (node_positions, vertex_buffer, instance_count) = {
            let pos = NodePositions::from_layout_tsv(layout_tsv)?;

            // TODO: ideally the node IDs and positions would be
            // stored in different buffers
            let vertex_data = pos
                .iter_nodes()
                .enumerate()
                .map(|(ix, p)| {
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
                "/shaders/path_2d_color_map_g.frag.spv"
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

            let color_targets = [
                wgpu::ColorTargetState {
                    format: window.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::all(),
                },
                wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R32Uint,
                    blend: None,
                    write_mask: wgpu::ColorWrites::all(),
                },
                wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rg32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::all(),
                },
            ];

            graph.add_graphics_schema_custom(
                state,
                vert_src,
                frag_src,
                primitive,
                wgpu::VertexStepMode::Instance,
                ["vertex_in"],
                None,
                color_targets.as_slice(),
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

        graph.add_link_from_transient("node_id_fb", draw_node, 2);
        graph.add_link_from_transient("node_uv_fb", draw_node, 3);

        graph.add_link_from_transient("transform", draw_node, 4);
        graph.add_link_from_transient("vert_cfg", draw_node, 5);

        graph.add_link_from_transient("node_data", draw_node, 6);

        graph.add_link_from_transient("sampler", draw_node, 7);
        graph.add_link_from_transient("color_texture", draw_node, 8);
        graph.add_link_from_transient("color_map", draw_node, 9);

        // graph.add_link_from_transient("color", draw_node, 5);
        // graph.add_link_from_transient("color_mapping", draw_node, 6);

        let instances = instance_count as u32;
        println!("instance count: {instances}");
        println!("node count: {}", path_index.node_count);

        graph.set_node_preprocess_fn(draw_node, move |_ctx, op_state| {
            op_state.vertices = Some(0..6);
            op_state.instances = Some(0..instances);
        });

        // let active_viz_data_key = "node_id".to_string();
        let active_viz_data_key = "depth".to_string();

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

        let color_mapping = ColorMap {
            value_range: [0.0, 13.0],
            color_range: [0.0, 1.0],
        };

        let color_mapping = crate::util::Uniform::new(
            &state,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "Viewer 1D Color Mapping",
            color_mapping,
            |cm| {
                let data: [u8; 16] = bytemuck::cast(*cm);
                data
            },
        )?;

        let geometry_bufs = GeometryBuffers::allocate(
            state,
            window.window.inner_size().into(),
        )?;

        let (msg_tx, msg_rx) = crossbeam::channel::unbounded();

        let view_control_widget =
            ViewControlWidget::new(shared, msg_tx.clone());

        let mut annotation_layer = AnnotationLayer::default();

        {
            let annotations = shared
                .annotations
                .blocking_read()
                .annotation_sets
                .iter()
                .flat_map(|(set_id, set)| {
                    (0..set.annotations.len()).map(|i| GlobalAnnotationId {
                        set_id: *set_id,
                        annot_id: AnnotationId(i),
                    })
                })
                .collect::<Vec<_>>();

            annotation_layer.load_annotations(
                shared,
                &node_positions,
                annotations,
            );
        }

        let cfg = {
            let cfg = Config::default();

            let widget = config::ConfigWidget { cfg: cfg.clone() };

            settings_window.register_widget(
                "2D Viewer",
                "Configuration",
                Arc::new(RwLock::new(widget)),
            );

            cfg
        };

        Ok(Self {
            node_positions: Arc::new(node_positions),

            vertex_buffer,
            instance_count,

            view,

            transform_uniform,
            vert_config,

            geometry_bufs,

            render_graph: graph,
            draw_node,

            shared: shared.clone(),

            color_mapping,
            active_viz_data_key,
            data_buffer,

            msg_tx,
            msg_rx,

            cfg,

            view_control_widget,

            annotation_layer,
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
        window_dims: [f32; 2],
    ) {
        // not in pixels (not even sure what it is)
        let node_width = 120.0;

        let [w, h] = window_dims;

        let nw = node_width / w.max(h);

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
        context_state: &mut ContextState,
        dt: f32,
    ) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                control::Msg::View(cmd) => cmd.apply(
                    &self.shared,
                    &self.node_positions,
                    &mut self.view,
                ),
            }
        }

        egui_ctx.begin_frame(&window.window);

        let [width, height]: [u32; 2] = window.window.inner_size().into();
        let dims = ultraviolet::Vec2::new(width as f32, height as f32);

        let scale_dims = dims * egui_ctx.ctx().pixels_per_point();

        let screen_rect = egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(dims.x, dims.y),
        );

        let (main_panel_rect, side_panel_rect) = {
            let y_range = screen_rect.y_range();
            let (xl, _xr) = screen_rect.x_range().into_inner();

            let side_panel = egui::SidePanel::right("Viewer2D-side-panel")
                .max_width(screen_rect.width() * 0.5)
                .show(egui_ctx.ctx(), |ui| {
                    self.view_control_widget.show(ui);

                    ui.separator();

                    util::node_context_side_panel_info(
                        &self.shared.graph,
                        context_state,
                        ui,
                    );
                });

            let side_panel_rect = side_panel.response.rect;

            let xmid = side_panel_rect.left();

            let main_panel = egui::Rect::from_x_y_ranges(xl..=xmid, y_range);

            // let main_panel = main_panel.shrink(2.0);

            (main_panel, side_panel_rect)
        };

        let dims = dims / egui_ctx.ctx().pixels_per_point();

        let mut annot_shapes = Vec::new();

        let hovered_node_1d = context_state
            // .query_get_cast::<_, Node>(Some("Viewer1D"), ["hover"])
            .query_get_cast::<_, Node>(None, ["hover"])
            .copied();

        let goto_node_1d = context_state
            .query_get_cast::<_, Node>(Some("Viewer1D"), ["goto"])
            .copied();

        if let Some(node) = hovered_node_1d {
            let (n0, n1) = self.node_positions.node_pos(node);
            let mid = n0 + (n1 - n0) * 0.5;

            // a bit hacky but its fine
            if goto_node_1d.is_some() {
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

            let node_len = self.shared.graph.node_length(node);

            egui::containers::popup::show_tooltip(
                egui_ctx.ctx(),
                egui::Id::new("Viewer2D-Node-Tooltip"),
                |ui| {
                    ui.label(format!("Node {}", node.ix()));
                    ui.label(format!("Length {} bp", node_len.0));
                },
            );
        }

        let mut hover_pos: Option<[f32; 2]> = None;

        {
            let ctx = egui_ctx.ctx();

            let main_area = egui::Area::new("main_area_2d")
                .order(egui::Order::Background)
                .interactable(true)
                .movable(false)
                .constrain(true);

            let mut multi_touch_active = false;

            if let Some(touch) = egui_ctx.ctx().multi_touch() {
                multi_touch_active = true;
                let t = touch.translation_delta;
                let z = 2.0 - touch.zoom_delta;
                let t_ = ultraviolet::Vec2::new(-t.x / dims.x, t.y / dims.y);

                self.view.translate_size_rel(t_);
                self.view.size *= z;
            }

            main_area.show(ctx, |ui| {
                // ui.set_width(main_panel_rect.width());
                // ui.set_height(main_panel_rect.height());

                let area_rect = ui.allocate_rect(
                    main_panel_rect,
                    egui::Sense::click_and_drag(),
                );

                if area_rect.dragged_by(egui::PointerButton::Primary)
                    && !multi_touch_active
                {
                    let delta =
                        Vec2::from(mint::Vector2::from(area_rect.drag_delta()));
                    let mut norm_delta = -1.0 * (delta / dims);
                    norm_delta.y *= -1.0;
                    self.view.translate_size_rel(norm_delta);
                }

                let painter = ui.painter();

                painter.extend(annot_shapes);

                if self.cfg.show_annotation_labels.load() {
                    self.annotation_layer.draw(
                        &self.node_positions,
                        &self.view,
                        dims,
                        &painter,
                    );
                }
            });

            let scroll = ctx.input(|i| i.scroll_delta);

            if let Some(pos) = ctx.pointer_hover_pos() {
                hover_pos = Some([pos.x, pos.y]);
            }

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
        }

        egui_ctx.end_frame(&window.window);

        if let Some(hover_pos) = hover_pos {
            // look up in geometry buffer
            let node = self.geometry_bufs.lookup(&state.device, hover_pos);
            if let Some((node, u)) = node {
                if node.ix() < self.shared.graph.node_count {
                    let (node_offset, node_len) =
                        self.shared.graph.node_offset_length(node);
                    let local_pos =
                        (u as f64 * node_len.0 as f64).round() as u64;
                    let pos = Bp(node_offset.0 + local_pos);

                    let clicked = egui_ctx.ctx().input(|i| {
                        i.pointer.button_down(egui::PointerButton::Secondary)
                    });

                    if clicked {
                        context_state.set("Viewer2D", ["goto"], node);
                    }

                    context_state.set("Viewer2D", ["hover"], node);
                    context_state.set("Viewer2D", ["hover"], pos);
                }
            }
        }

        let width = window.window.inner_size().width as f32;
        let height = window.window.inner_size().height as f32;

        self.update_transform_uniform(&state.queue);
        self.update_vert_config_uniform(&state.queue, [width, height]);
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

        log::info!("reallocating geometry buffers");
        self.geometry_bufs = GeometryBuffers::allocate(state, new_window_dims)?;

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

        self.geometry_bufs.use_as_resource(&mut transient_res);

        let v_stride = std::mem::size_of::<[f32; 5]>();
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

        let data_buf_size =
            self.shared.graph.node_count * std::mem::size_of::<[f32; 5]>();
        // println!("data_buf

        transient_res.insert(
            "node_data".to_string(),
            InputResource::Buffer {
                size: data_buf_size,
                stride: None,
                buffer: &self.data_buffer,
            },
        );

        // transient_res.insert(
        //     "color".to_string(),
        //     InputResource::Buffer {
        //         size: color_buf_size,
        //         stride: None,
        //         buffer: &color_buf,
        //     },
        // );

        // transient_res.insert(
        //     "color_mapping".to_string(),
        //     InputResource::Buffer {
        //         size: 24,
        //         stride: None,
        //         buffer: &color_map_buf,
        //     },
        // );

        ////

        let (sampler, tex, tex_size) = {
            let colors = self.shared.colors.blocking_read();

            let sampler = colors.linear_sampler.clone();

            let id = self
                .shared
                .data_color_schemes
                .blocking_read()
                .get(&self.active_viz_data_key)
                .copied()
                .unwrap();

            let scheme = colors.get_color_scheme(id);
            let size = [scheme.colors.len() as u32, 1];

            (sampler, colors.get_color_scheme_texture(id).unwrap(), size)
        };

        let texture = &tex.0;
        let view = &tex.1;

        transient_res.insert(
            "color_texture".to_string(),
            InputResource::Texture {
                size: tex_size,
                format,
                sampler: None,
                texture: Some(texture),
                view: Some(view),
            },
        );

        transient_res.insert(
            "sampler".to_string(),
            InputResource::Texture {
                size: tex_size,
                format,
                sampler: Some(&sampler),
                texture: None,
                view: None,
            },
        );

        transient_res.insert(
            "color_map".into(),
            InputResource::Buffer {
                size: self.color_mapping.buffer_size(),
                stride: None,
                buffer: self.color_mapping.buffer(),
            },
        );

        /////

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

        self.geometry_bufs.download_textures(encoder);

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

fn parse_path(
    s: &std::ffi::OsStr,
) -> std::result::Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}

struct GeometryBuffers {
    dims: [u32; 2],

    node_id_tex: Texture,
    node_uv_tex: Texture,

    node_id_copy_dst_tex: Texture,
    node_uv_copy_dst_tex: Texture,

    node_id_buf: BufferDesc,
    node_uv_buf: BufferDesc,
}

impl GeometryBuffers {
    fn dims(&self) -> [u32; 2] {
        self.dims
    }

    fn aligned_dims(&self) -> [u32; 2] {
        let [w, h] = self.dims;
        let w = Self::aligned_image_width(w);
        [w, h]
    }

    fn lookup(
        &self,
        device: &wgpu::Device,
        pos: [f32; 2],
    ) -> Option<(Node, f32)> {
        let x = pos[0].round() as usize;
        let y = pos[1].round() as usize;

        let dims = self.dims();

        if x >= dims[0] as usize || y >= dims[1] as usize {
            return None;
        }

        let [aligned_width, _] = self.aligned_dims();

        self.node_id_buf
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, Result::unwrap);
        self.node_uv_buf
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, Result::unwrap);
        device.poll(wgpu::Maintain::Wait);

        let node = {
            let stride = std::mem::size_of::<u32>() as u64;
            let row_size = aligned_width as u64 * stride;

            let row_start = (y as u64 * row_size) as u64;
            let row_end = row_start + row_size;

            let row = self
                .node_id_buf
                .buffer
                .slice(row_start..row_end)
                .get_mapped_range();

            let row_u32: &[u32] = bytemuck::cast_slice(&row);

            let data = row_u32[x];

            data.checked_sub(1).map(Node::from)
        };

        let pos = {
            let stride = std::mem::size_of::<[f32; 2]>() as u64;
            let row_size = aligned_width as u64 * stride;

            let row_start = (y as u64 * row_size) as u64;
            let row_end = row_start + row_size;

            let row = self
                .node_uv_buf
                .buffer
                .slice(row_start..row_end)
                .get_mapped_range();

            let row_u32: &[[f32; 2]] = bytemuck::cast_slice(&row);

            let [pos, _] = row_u32[x];

            pos
        };

        self.node_id_buf.buffer.unmap();
        self.node_uv_buf.buffer.unmap();

        node.map(|n| (n, pos))
    }

    fn download_textures(&self, encoder: &mut wgpu::CommandEncoder) {
        // first copy the attachments to the `copy_dst` textures

        let origin = wgpu::Origin3d::default();

        let extent = wgpu::Extent3d {
            width: self.dims[0],
            height: self.dims[1],
            depth_or_array_layers: 1,
        };

        let aligned_width = Self::aligned_image_width(self.dims[0]);
        let aligned_extent = wgpu::Extent3d {
            width: aligned_width,
            ..extent
        };

        let src_tex = wgpu::ImageCopyTexture {
            texture: &self.node_id_tex.texture,
            mip_level: 0,
            origin,
            aspect: wgpu::TextureAspect::All,
        };

        let dst_tex = wgpu::ImageCopyTexture {
            texture: &self.node_id_copy_dst_tex.texture,
            mip_level: 0,
            origin,
            aspect: wgpu::TextureAspect::All,
        };

        encoder.copy_texture_to_texture(src_tex, dst_tex, extent);

        let src_tex = wgpu::ImageCopyTexture {
            texture: &self.node_uv_tex.texture,
            ..src_tex
        };

        let dst_tex = wgpu::ImageCopyTexture {
            texture: &self.node_uv_copy_dst_tex.texture,
            ..dst_tex
        };

        encoder.copy_texture_to_texture(src_tex, dst_tex, extent);

        // then copy the aligned textures to the destination buffers

        let src_tex = wgpu::ImageCopyTexture {
            texture: &self.node_id_copy_dst_tex.texture,
            ..src_tex
        };

        let stride = std::mem::size_of::<u32>() as u32;
        let dst_buf = wgpu::ImageCopyBuffer {
            buffer: &self.node_id_buf.buffer,
            layout: wgpu::ImageDataLayout {
                bytes_per_row: NonZeroU32::new(aligned_width * stride),
                ..wgpu::ImageDataLayout::default()
            },
        };

        encoder.copy_texture_to_buffer(src_tex, dst_buf, extent);

        let src_tex = wgpu::ImageCopyTexture {
            texture: &self.node_uv_copy_dst_tex.texture,
            ..src_tex
        };

        let stride = std::mem::size_of::<[f32; 2]>() as u32;
        let dst_buf = wgpu::ImageCopyBuffer {
            buffer: &self.node_uv_buf.buffer,
            layout: wgpu::ImageDataLayout {
                bytes_per_row: NonZeroU32::new(aligned_width * stride),
                ..wgpu::ImageDataLayout::default()
            },
        };

        encoder.copy_texture_to_buffer(src_tex, dst_buf, extent);
    }

    fn aligned_image_width(width: u32) -> u32 {
        let div = width / 256;
        let rem = ((width % 256) != 0) as u32;
        256 * (div + rem)
    }

    fn allocate(state: &raving_wgpu::State, dims: [u32; 2]) -> Result<Self> {
        use wgpu::TextureUsages;

        let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;

        let width = dims[0] as usize;
        let height = dims[1] as usize;

        let node_id_tex = Texture::new(
            &state.device,
            &state.queue,
            width,
            height,
            wgpu::TextureFormat::R32Uint,
            usage,
            Some("Viewer2D Node ID Attch."),
        )?;

        let node_uv_tex = Texture::new(
            &state.device,
            &state.queue,
            width,
            height,
            wgpu::TextureFormat::Rg32Float,
            usage,
            Some("Viewer2D Node Position Attch."),
        )?;

        let usage = TextureUsages::COPY_DST | TextureUsages::COPY_SRC;

        // wgpu requires image widths to be a multiple of 256 to be
        // able to copy to a buffer
        let aligned_width = Self::aligned_image_width(dims[0]) as usize;

        let node_id_copy_dst_tex = Texture::new(
            &state.device,
            &state.queue,
            aligned_width,
            height,
            wgpu::TextureFormat::R32Uint,
            usage,
            Some("Viewer2D Node ID Copy Dst"),
        )?;

        let node_uv_copy_dst_tex = Texture::new(
            &state.device,
            &state.queue,
            aligned_width,
            height,
            wgpu::TextureFormat::Rg32Float,
            usage,
            Some("Viewer2D Node Position Copy Dst"),
        )?;

        let usage = BufferUsages::COPY_DST | BufferUsages::MAP_READ;

        let node_id_buf = {
            let buf_size = aligned_width * height * std::mem::size_of::<u32>();

            let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Viewer2D Node ID Output Buffer"),
                usage,
                size: buf_size as u64,
                mapped_at_creation: false,
            });

            BufferDesc {
                buffer,
                size: buf_size,
            }
        };

        let node_uv_buf = {
            let buf_size =
                aligned_width * height * std::mem::size_of::<[f32; 2]>();

            let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Viewer2D Node UV Output Buffer"),
                usage,
                size: buf_size as u64,
                mapped_at_creation: false,
            });

            BufferDesc {
                buffer,
                size: buf_size,
            }
        };

        Ok(Self {
            dims,
            node_id_tex,
            node_uv_tex,
            node_id_buf,
            node_uv_buf,
            node_id_copy_dst_tex,
            node_uv_copy_dst_tex,
        })
    }

    fn use_as_resource<'a: 'b, 'b>(
        &'a self,
        transient_res_map: &mut HashMap<String, InputResource<'b>>,
    ) {
        transient_res_map.insert(
            "node_id_fb".into(),
            InputResource::Texture {
                size: self.dims,
                format: wgpu::TextureFormat::R32Uint,
                texture: None,
                view: self.node_id_tex.view.as_ref(),
                sampler: None,
            },
        );

        transient_res_map.insert(
            "node_uv_fb".into(),
            InputResource::Texture {
                size: self.dims,
                format: wgpu::TextureFormat::Rg32Float,
                texture: None,
                view: self.node_uv_tex.view.as_ref(),
                sampler: None,
            },
        );
    }
}
