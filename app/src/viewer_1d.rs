use crate::annotations::AnnotationStore;
use crate::app::AppWindow;
use crate::gui::FlexLayout;
use crate::list::ListView;
use waragraph_core::graph::PathId;
use wgpu::BufferUsages;

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;

use winit::event::WindowEvent;
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::Window;

use raving_wgpu::camera::{DynamicCamera2d, TouchHandler, TouchOutput};
use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use anyhow::Result;

use waragraph_core::graph::{sampling::PathDepthData, PathIndex};

use self::util::path_depth_data_viz_buffer;
use self::view::View1D;

pub mod events;
pub mod gui;

// pub mod sampling;
pub mod util;
pub mod view;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub init_range: Option<std::ops::Range<u64>>,
}

struct Viewer1D {
    render_graph: Graph,
    egui: EguiCtx,
    path_index: Arc<PathIndex>,
    draw_path_slot: NodeId,

    view: View1D,
    rendered_view: std::ops::Range<u64>,

    force_resample: bool,

    depth_data: Arc<PathDepthData>,

    vertices: BufferDesc,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,

    path_viz_cache: PathVizCache,

    // slot_layout: FlexLayout<GuiElem>,
    // gui_layout: FlexLayout<gui::GuiElem>,
    slot_layout: FlexLayout<gui::SlotElem>,

    path_list_view: ListView<PathId>,

    sample_handle:
        Option<tokio::task::JoinHandle<(std::ops::Range<u64>, Vec<u8>)>>,
}

#[derive(Debug)]
struct BufferDesc {
    buffer: wgpu::Buffer,
    size: usize,
    deleted: bool,
}

impl BufferDesc {
    fn new(buffer: wgpu::Buffer, size: usize) -> Self {
        Self {
            buffer,
            size,
            deleted: false,
        }
    }
}

// TODO this should be more general/shared across the entire app
#[derive(Debug, Default)]
struct PathVizCache {
    buffer_names: HashMap<String, usize>,
    buffers: Vec<BufferDesc>,
}

impl PathVizCache {
    fn get(&self, name: &str) -> Option<&BufferDesc> {
        let ix = self.buffer_names.get(name)?;
        self.buffers.get(*ix)
    }

    fn insert(&mut self, name: &str, desc: BufferDesc) {
        let ix = self.buffers.len();
        self.buffer_names.insert(name.into(), ix);
        self.buffers.push(desc);
    }
}

fn path_frag_example_uniforms(
    device: &wgpu::Device,
) -> Result<(BufferDesc, BufferDesc)> {
    let usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;

    let color = {
        let len = 256;
        let colors = (0..len)
            .flat_map(|i| {
                let gradient = colorous::MAGMA;
                // let gradient = colorous::SPECTRAL;
                let color = gradient.eval_rational(i, len);
                let [r, g, b] = color.as_array();
                let max = u8::MAX as f32;
                [r as f32 / max, g as f32 / max, b as f32 / max, 1.0]
            })
            .collect::<Vec<_>>();

        let rgba = |r: u8, g: u8, b: u8| {
            let max = u8::MAX as f32;
            [r as f32 / max, g as f32 / max, b as f32 / max, 1.0]
        };

        let colors = [
            rgba(255, 255, 255),
            rgba(196, 196, 196),
            rgba(128, 128, 128),
            rgba(158, 1, 66),
            rgba(213, 62, 79),
            rgba(244, 109, 67),
            rgba(253, 174, 97),
            rgba(254, 224, 139),
            rgba(255, 255, 191),
            rgba(230, 245, 152),
            rgba(171, 221, 164),
            rgba(102, 194, 165),
            rgba(50, 136, 189),
            rgba(94, 79, 162),
        ];
        let len = colors.len();

        let mut data: Vec<u8> = vec![];
        data.extend(bytemuck::cast_slice(&[len, 0, 0, 0]));
        data.extend(bytemuck::cast_slice(&colors));

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: data.as_slice(),
            usage,
        });

        BufferDesc::new(buffer, data.len())
    };

    let data = {
        let values = (0..100).map(|i| i / 10).collect::<Vec<u32>>();
        let len = values.len();

        let mut data: Vec<u8> = vec![];
        data.extend(bytemuck::cast_slice(&[len, 0, 0, 0]));
        data.extend(bytemuck::cast_slice(&values));

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&data),
            usage,
        });

        BufferDesc::new(buffer, data.len())
    };

    Ok((color, data))
}

impl Viewer1D {
    fn init(
        event_loop: &EventLoopWindowTarget<()>,
        win_dims: [u32; 2],
        state: &State,
        path_index: Arc<PathIndex>,
        init_range: Option<std::ops::Range<u64>>,
    ) -> Result<Self> {
        let mut graph = Graph::new();

        let draw_schema = {
            let vert_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d.vert.spv"
            ));
            let frag_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d.frag.spv"
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
                &[state.surface_format],
            )?
        };

        let (vert_uniform, frag_uniform) = {
            let data = [win_dims[0] as f32, win_dims[1] as f32];
            let usage = BufferUsages::UNIFORM | BufferUsages::COPY_DST;

            let vert_uniform =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[data]),
                    usage,
                });

            // let data = [0.7f32, 0.1, 0.85, 1.0];
            // let data = [1.0f32, 0.0, 0.0, 1.0];
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

        let draw_node = graph.add_node(draw_schema);
        graph.add_link_from_transient("vertices", draw_node, 0);
        graph.add_link_from_transient("swapchain", draw_node, 1);

        graph.add_link_from_transient("vert_cfg", draw_node, 2);
        // graph.add_link_from_transient("frag_cfg", draw_node, 3);

        // graph.add_link_from_transient("data", draw_node, 3);
        graph.add_link_from_transient("depth", draw_node, 3);
        graph.add_link_from_transient("color", draw_node, 4);
        graph.add_link_from_transient("transform", draw_node, 5);

        let egui = EguiCtx::init(event_loop, state, None);
        let pangenome_len = path_index.pangenome_len().0;

        let (color, data) = path_frag_example_uniforms(&state.device)?;

        let depth_data = Arc::new(PathDepthData::new(&path_index));

        let len = pangenome_len as u64;
        let mut view = View1D::new(len);

        let paths = 0..path_index.path_names.len();

        let path_list_view =
            ListView::new(paths.clone().map(PathId::from), Some(32));

        let paths = path_list_view.visible_iter().copied();

        let depth = path_depth_data_viz_buffer(
            &state.device,
            &path_index,
            &depth_data,
            paths,
            view.range().clone(),
            1024,
        )?;

        let mut path_viz_cache = PathVizCache::default();
        path_viz_cache.insert("color", color);
        path_viz_cache.insert("data", data);
        path_viz_cache.insert("depth", depth);

        let mut slot_layout = gui::create_slot_layout(32, "depth")?;

        let (vertices, vxs, insts) = {
            let size =
                ultraviolet::Vec2::new(win_dims[0] as f32, win_dims[1] as f32);
            let (buffer, insts) = Self::slot_vertex_buffer(
                &state.device,
                size,
                &mut slot_layout,
                &path_list_view,
            )?;
            let vxs = 0..6;
            let insts = 0..insts;

            (buffer, vxs, insts)
        };

        graph.set_node_preprocess_fn(draw_node, move |_ctx, op_state| {
            op_state.vertices = Some(vxs.clone());
            op_state.instances = Some(insts.clone());
        });

        view.set(0, 10_000);

        Ok(Viewer1D {
            render_graph: graph,
            egui,
            path_index,
            draw_path_slot: draw_node,

            view: view.clone(),
            rendered_view: view.range().clone(),
            force_resample: false,

            depth_data,

            vertices,
            vert_uniform,
            frag_uniform,

            path_viz_cache,

            slot_layout,
            // fixed_gui_layout,
            path_list_view,

            sample_handle: None,
        })
    }

    /// Returns a line equation that defines the transformation
    /// parameters used by the slot fragment shader
    ///
    /// `view0` corresponds to the view that has been sampled and is available
    /// in the data buffer, while `view1` is the current view.
    ///
    /// Usage: If the returned value is [a, b], the transformation
    /// is applied by a*t + b
    fn sample_index_transform(
        view0: &std::ops::Range<u64>,
        view1: &std::ops::Range<u64>,
    ) -> [f32; 2] {
        let l0 = view0.start as f32;
        let r0 = view0.end as f32;
        let l1 = view1.start as f32;
        let r1 = view1.end as f32;

        let v0 = r0 - l0;
        let v1 = r1 - l1;

        let a = v1 / v0;
        let b = (l1 - l0) / v0;

        [a, b]
    }

    fn sample_buffer_size(bins: usize, rows: usize) -> usize {
        let prefix_size = std::mem::size_of::<u32>() * 4;
        let elem_size = std::mem::size_of::<f32>();
        let size = prefix_size + elem_size * bins * rows;
        size
    }

    fn sample_into_vec(
        index: &PathIndex,
        data: &PathDepthData,
        paths: &[PathId],
        view_range: std::ops::Range<u64>,
        buffer: &mut Vec<u8>,
    ) -> Result<()> {
        let bins = 1024;
        let size = Self::sample_buffer_size(bins, paths.len());
        buffer.resize(size, 0u8);
        Self::sample_into_data_buffer(index, data, paths, view_range, buffer)
    }

    fn sample_into_data_buffer(
        index: &PathIndex,
        data: &PathDepthData,
        paths: &[PathId],
        view_range: std::ops::Range<u64>,
        buffer: &mut [u8],
    ) -> Result<()> {
        let bins = 1024;
        let size = Self::sample_buffer_size(bins, paths.len());

        assert!(buffer.len() >= size);

        waragraph_core::graph::sampling::sample_path_data_into_buffer(
            index,
            data,
            paths.into_iter().copied(),
            bins,
            view_range,
            buffer,
        );

        Ok(())
    }

    fn sample_into_gpu_buffer(
        state: &State,
        index: &PathIndex,
        data: &PathDepthData,
        paths: &[PathId],
        // path_list_view: &ListView<PathId>,
        view_range: std::ops::Range<u64>,
        gpu_buffer: &BufferDesc,
        // bins: usize,
    ) -> Result<()> {
        let bins = 1024;
        let size = Self::sample_buffer_size(bins, paths.len());
        let size = NonZeroU64::new(size as u64).unwrap();

        let mut buffer_view =
            state.queue.write_buffer_with(&gpu_buffer.buffer, 0, size);

        Self::sample_into_data_buffer(
            index,
            data,
            paths,
            view_range,
            buffer_view.as_mut(),
        )
    }

    // TODO there's no need to reallocate the buffer every time the list is scrolled...
    fn slot_vertex_buffer(
        device: &wgpu::Device,
        win_dims: ultraviolet::Vec2,
        layout: &mut FlexLayout<gui::SlotElem>,
        path_list_view: &ListView<PathId>,
    ) -> Result<(BufferDesc, u32)> {
        let mut data_buf: Vec<u8> = Vec::new();

        let stride = std::mem::size_of::<[f32; 5]>();

        layout.visit_layout(win_dims, |layout, elem| {
            if let gui::SlotElem::PathData { slot_id, data_id } = elem {
                if let Some(path_id) = path_list_view.get_in_view(*slot_id) {
                    let rect = crate::gui::layout_egui_rect(&layout);
                    let v_pos = rect.left_bottom().to_vec2();
                    let v_size = rect.size();

                    data_buf.extend(bytemuck::cast_slice(&[v_pos, v_size]));
                    data_buf.extend(bytemuck::cast_slice(&[*slot_id as u32]));
                }
            }
        })?;
        let slot_count = data_buf.len() / stride;

        let usage = wgpu::BufferUsages::VERTEX;

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: data_buf.as_slice(),
            usage,
        });

        let slots = BufferDesc::new(buffer, data_buf.len());

        Ok((slots, slot_count as u32))
    }
}

impl AppWindow for Viewer1D {
    fn update(
        &mut self,
        tokio_rt: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &winit::window::Window,
        dt: f32,
    ) {
        if self.sample_handle.is_none()
            && (&self.rendered_view != self.view.range() || self.force_resample)
        {
            let paths = self
                .path_list_view
                .visible_iter()
                .copied()
                .collect::<Vec<_>>();

            let view_range = self.view.range().clone();

            let path_index = self.path_index.clone();
            let data = self.depth_data.clone();

            let join = tokio_rt.spawn_blocking(move || {
                let mut buf: Vec<u8> = Vec::new();

                Self::sample_into_vec(
                    &path_index,
                    &data,
                    &paths,
                    view_range.clone(),
                    &mut buf,
                )
                .unwrap();

                (view_range, buf)
            });

            self.sample_handle = Some(join);
        }

        if let Some(true) = self.sample_handle.as_ref().map(|j| j.is_finished())
        {
            let handle = self.sample_handle.take().unwrap();

            if let Ok((view_range, data)) = tokio_rt.block_on(handle) {
                let gpu_buffer = self.path_viz_cache.get("depth").unwrap();
                state.queue.write_buffer(&gpu_buffer.buffer, 0, &data);

                self.rendered_view = view_range;
                self.force_resample = false;
            }
        }

        // update uniform
        {
            let data = Self::sample_index_transform(
                &self.rendered_view,
                self.view.range(),
            );

            state.queue.write_buffer(
                &self.frag_uniform,
                0,
                bytemuck::cast_slice(&data),
            );
        }

        // TODO debug FlexLayout rendering should use a render graph
        self.egui.run(window, |ctx| {
            let avail = ctx.available_rect();
            // println!("available: {avail:?}");
            // let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0),
            //                                     egui::pos2(
            let mut main_ui = egui::Ui::new(
                ctx.clone(),
                egui::LayerId::new(
                    egui::Order::Foreground,
                    "main_layer".into(),
                ),
                "main_ui".into(),
                avail,
                avail,
            );

            // egui::Area::new("

            let painter = ctx.debug_painter();

            let size = window.inner_size();
            let size =
                ultraviolet::Vec2::new(size.width as f32, size.height as f32);

            let stroke = egui::Stroke {
                width: 1.0,
                color: egui::Color32::RED,
            };

            let mut path_name_region = egui::Rect::NOTHING;
            let mut path_slot_region = egui::Rect::NOTHING;

            let screen_rect = main_ui.clip_rect();

            let result = self.slot_layout.visit_layout(size, |layout, elem| {
                let rect = crate::gui::layout_egui_rect(&layout);

                // hacky fix for rows that are laid out beyond the limits of the view
                if !screen_rect.intersects(rect) {
                    return;
                }

                painter.rect_stroke(rect, egui::Rounding::default(), stroke);

                match elem {
                    gui::SlotElem::PathData { slot_id, .. } => {
                        path_slot_region = path_slot_region.union(rect);
                    }
                    gui::SlotElem::PathName { slot_id } => {
                        path_name_region = path_name_region.union(rect);

                        let path_id = self.path_list_view.get_in_view(*slot_id);
                        if path_id.is_none() {
                            return;
                        }
                        let path_id = path_id.unwrap();

                        let path_name = self
                            .path_index
                            .path_names
                            .get_by_left(path_id)
                            .unwrap();
                        painter.text(
                            rect.left_center(),
                            egui::Align2::LEFT_CENTER,
                            path_name,
                            egui::FontId::monospace(16.0),
                            egui::Color32::WHITE,
                        );
                    }
                }
            });

            // check interactions against `path_name_region` and `path_slot_region`
            let pos = ctx.pointer_latest_pos();
            let (scroll, primary_down, pointer_delta) = {
                let input = ctx.input();
                let primary = input.pointer.primary_down();
                let delta = input.pointer.delta();
                (input.scroll_delta, primary, delta)
            };

            let path_name_interact = main_ui
                .allocate_rect(path_name_region, egui::Sense::click_and_drag());

            painter.rect_stroke(
                path_name_region,
                // path_name_interact.rect,
                0.0,
                egui::Stroke {
                    width: 2.0,
                    color: egui::Color32::BLUE,
                },
            );

            // if path_name_interact.clicked() {
            //     println!("path names clicked!");
            // }

            if let Some(pos) = pos {
                if path_name_region.contains(pos) {
                    // TODO: scroll path list
                    painter.text(
                        egui::pos2(10.0, size.y - 10.0),
                        egui::Align2::LEFT_CENTER,
                        scroll.y.to_string(),
                        egui::FontId::monospace(16.0),
                        egui::Color32::WHITE,
                    );
                }

                if path_slot_region.contains(pos) {
                    let (l, r) = path_slot_region.x_range().into_inner();
                    let rel_x = (pos.x - l) / (r - l);
                    let dx = pointer_delta.x / (r - l);

                    if primary_down {
                        self.view.translate_norm_f32(-dx);
                    }

                    // let min_scroll = 5.0;
                    let min_scroll = 1.0;
                    let factor = 0.01;
                    if scroll.y.abs() > min_scroll {
                        let dz = 1.0 - scroll.y * factor;
                        // self.view.zoom_around_norm_f32(rel_x, dz);
                        println!("rel_x: {rel_x}\tdz: {dz}");

                        self.view.zoom_with_focus(rel_x, dz);
                    }

                    // TODO: zoom path view
                    painter.text(
                        egui::pos2(size.x - 10.0, size.y - 10.0),
                        egui::Align2::RIGHT_CENTER,
                        scroll.y.to_string(),
                        egui::FontId::monospace(16.0),
                        egui::Color32::WHITE,
                    );
                }
            }

            if let Err(e) = result {
                eprintln!("draw layout error: {e:?}");
            }
        });
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        let mut consume = false;

        let resp = self.egui.on_event(event);

        // if let Some(touch) = self.egui.multi_touch() {
        //     println!("multitouch: {touch:?}");
        // }

        // if let Some(pos) = self.egui.pointer_latest_pos() {
        //     println!("pos: {pos:?}");
        // }

        // if self.touch.on_event(window_dims, event) {
        //     consume = true;
        // }

        if let WindowEvent::KeyboardInput { input, .. } = event {
            if let Some(key) = input.virtual_keycode {
                use winit::event::ElementState;
                use winit::event::VirtualKeyCode as Key;
                let pressed = matches!(input.state, ElementState::Pressed);

                // let mut l = self.view.start;
                // let mut r = self.view.end;
                // let len = r - l;

                if pressed {
                    match key {
                        Key::Right => {
                            self.view.translate_norm_f32(0.1);
                        }
                        Key::Left => {
                            self.view.translate_norm_f32(-0.1);
                        }
                        Key::Up => {
                            self.path_list_view.scroll_relative(-1);
                            self.force_resample = true;
                        }
                        Key::Down => {
                            self.path_list_view.scroll_relative(1);
                            self.force_resample = true;
                        }
                        Key::Space => {
                            self.view.reset();
                        }
                        _ => (),
                    }
                }

                // self.view = l..r;
            }
        }

        consume
    }

    fn resize(
        &mut self,
        state: &raving_wgpu::State,
        _old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        let [w, h] = new_window_dims;
        let new_size = ultraviolet::Vec2::new(w as f32, h as f32);

        let (vertices, vxs, insts) = {
            let (buffer, insts) = Self::slot_vertex_buffer(
                &state.device,
                new_size,
                &mut self.slot_layout,
                &self.path_list_view,
            )?;
            let vxs = 0..6;
            let insts = 0..insts;

            (buffer, vxs, insts)
        };

        self.render_graph.set_node_preprocess_fn(
            self.draw_path_slot,
            move |_ctx, op_state| {
                op_state.vertices = Some(vxs.clone());
                op_state.instances = Some(insts.clone());
            },
        );

        self.vertices = vertices;

        let uniform_data = [new_size.x, new_size.y];

        state.queue.write_buffer(
            &self.vert_uniform,
            0,
            bytemuck::cast_slice(uniform_data.as_slice()),
        );

        Ok(())
    }

    fn render(&mut self, state: &mut raving_wgpu::State) -> anyhow::Result<()> {
        let dims = state.size;
        let size = [dims.width, dims.height];

        let mut transient_res: HashMap<String, InputResource<'_>> =
            HashMap::default();

        if let Ok(output) = state.surface.get_current_texture() {
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

            let v_stride = std::mem::size_of::<[f32; 5]>();
            transient_res.insert(
                "vertices".into(),
                InputResource::Buffer {
                    size: self.vertices.size,
                    stride: Some(v_stride),
                    buffer: &self.vertices.buffer,
                },
            );

            transient_res.insert(
                "vert_cfg".into(),
                InputResource::Buffer {
                    size: 2 * 4,
                    stride: None,
                    buffer: &self.vert_uniform,
                },
            );

            for name in ["data", "color", "depth"] {
                if let Some(desc) = self.path_viz_cache.get(name) {
                    transient_res.insert(
                        name.into(),
                        InputResource::Buffer {
                            size: desc.size,
                            stride: None,
                            buffer: &desc.buffer,
                        },
                    );
                }
            }

            transient_res.insert(
                "transform".into(),
                InputResource::Buffer {
                    size: 2 * 4,
                    stride: None,
                    buffer: &self.frag_uniform,
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

            let _sub_index = self
                .render_graph
                .execute(&state, &transient_res, &rhai::Map::default())
                .unwrap();

            let mut encoder = state.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("egui render"),
                },
            );

            self.egui.render(state, &output_view, &mut encoder);

            state.queue.submit(Some(encoder.finish()));

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
    path_index: Arc<PathIndex>,
    args: Args,
) -> Result<Box<dyn AppWindow>> {
    // let path_index = PathIndex::from_gfa(&args.gfa)?;

    let dims = {
        let s = window.inner_size();
        [s.width, s.height]
    };

    let app = Viewer1D::init(
        &event_loop,
        dims,
        &state,
        path_index,
        args.init_range.clone(),
    )?;

    Ok(Box::new(app))
}
