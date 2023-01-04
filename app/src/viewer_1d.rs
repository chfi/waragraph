use crate::annotations::AnnotationStore;
use crate::gui::{FlexLayout, GuiElem};
use wgpu::BufferUsages;

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::PathBuf;

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

// pub mod sampling;
pub mod util;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub init_range: Option<std::ops::Range<u64>>,
}

struct Viewer1D {
    render_graph: Graph,
    egui: EguiCtx,
    path_index: PathIndex,
    draw_path_slot: NodeId,

    pangenome_len: u64,
    view: std::ops::Range<u64>,
    rendered_view: std::ops::Range<u64>,

    depth_data: PathDepthData,

    // vertices: wgpu::Buffer,
    vertices: BufferDesc,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,

    path_viz_cache: PathVizCache,
    // data_uniform: wgpu::Buffer,
    // data_size: usize,
    // color_uniform: wgpu::Buffer,
    // color_size: usize,
    slot_layout: FlexLayout<GuiElem>,
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
        path_index: PathIndex,
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

            let data = [0.7f32, 0.1, 0.85, 1.0];
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

        let egui = EguiCtx::init(event_loop, state, None);
        let pangenome_len = path_index.pangenome_len().0;

        let (color, data) = path_frag_example_uniforms(&state.device)?;

        let depth_data = PathDepthData::new(&path_index);

        let len = pangenome_len as u64;
        let view_range = init_range.unwrap_or(0..(len / 10));

        // let depth = path_viz_buffer_test(&state.device, 200)?;

        let paths = 0..(path_index.path_names.len().min(64));

        let depth = path_depth_data_viz_buffer(
            &state.device,
            &path_index,
            &depth_data,
            paths,
            view_range.clone(),
            1024,
        )?;

        let mut path_viz_cache = PathVizCache::default();
        path_viz_cache.insert("color", color);
        path_viz_cache.insert("data", data);
        path_viz_cache.insert("depth", depth);

        let mut slot_layout = {
            use taffy::prelude::*;

            let mut rows = Vec::new();

            let mk_entry =
                |perc: f32, elem: GuiElem| (elem, Dimension::Percent(perc));

            rows.push(vec![mk_entry(1.0, GuiElem::Label { id: "view_range" })]);

            for (slot_id, (path_id, _path_name)) in
                path_index.path_names.iter().enumerate()
            {
                let path_id = *path_id;

                rows.push(vec![
                    mk_entry(0.2, GuiElem::PathName { path_id }),
                    mk_entry(
                        0.8,
                        GuiElem::PathSlot {
                            slot_id,
                            path_id,
                            data: "depth",
                        },
                    ),
                ]);
            }

            FlexLayout::from_rows_iter(rows)?
        };

        // let vertices = util::path_slot_vertex_buffer(&state.device, 0..10)?;

        let (vertices, vxs, insts) = {
            let size =
                ultraviolet::Vec2::new(win_dims[0] as f32, win_dims[1] as f32);
            let (buffer, insts) =
                Self::slot_vertices(&state.device, size, &mut slot_layout)?;
            let vxs = 0..6;
            let insts = 0..insts;
            // println!("slot_count: {slot_count}");

            (buffer, vxs, insts)
        };

        graph.set_node_preprocess_fn(draw_node, move |_ctx, op_state| {
            // op_state.vertices = Some(0..6);
            // op_state.instances = Some(0..10);
            op_state.vertices = Some(vxs.clone());
            op_state.instances = Some(insts.clone());
        });

        Ok(Viewer1D {
            render_graph: graph,
            egui,
            path_index,
            draw_path_slot: draw_node,
            pangenome_len,

            view: view_range.clone(),
            rendered_view: view_range,

            depth_data,

            vertices,
            vert_uniform,
            frag_uniform,

            path_viz_cache,

            slot_layout,
        })
    }

    fn sample_into_data_buffer(
        state: &State,
        index: &PathIndex,
        data: &PathDepthData,
        paths: impl IntoIterator<Item = usize>,
        view_range: std::ops::Range<u64>,
        gpu_buffer: &BufferDesc,
        // bins: usize,
    ) -> Result<()> {
        let bins = 1024;
        // let gpu_buf =
        let paths = paths.into_iter().collect::<Vec<_>>();
        let prefix_size = std::mem::size_of::<u32>() * 4;
        let elem_size = std::mem::size_of::<f32>();
        let size = prefix_size + elem_size * bins * paths.len();

        let size = NonZeroU64::new(size as u64).unwrap();

        let mut view =
            state.queue.write_buffer_with(&gpu_buffer.buffer, 0, size);

        waragraph_core::graph::sampling::sample_path_data_into_buffer(
            index,
            data,
            paths,
            bins,
            view_range,
            view.as_mut(),
        );

        Ok(())
    }

    fn slot_vertices(
        device: &wgpu::Device,
        win_dims: ultraviolet::Vec2,
        layout: &mut FlexLayout<GuiElem>,
    ) -> Result<(BufferDesc, u32)> {
        let mut data_buf: Vec<u8> = Vec::new();

        let stride = std::mem::size_of::<[f32; 5]>();

        layout.visit_layout(win_dims, |layout, elem| {
            if let GuiElem::PathSlot {
                slot_id,
                path_id,
                data,
            } = elem
            {
                let rect = crate::gui::layout_egui_rect(&layout);
                let v_pos = rect.left_bottom().to_vec2();
                let v_size = rect.size();

                data_buf.extend(bytemuck::cast_slice(&[v_pos, v_size]));
                data_buf.extend(bytemuck::cast_slice(&[*slot_id as u32]));
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

impl crate::AppWindow for Viewer1D {
    fn update(
        &mut self,
        state: &raving_wgpu::State,
        window: &winit::window::Window,
        dt: f32,
    ) {
        if self.rendered_view != self.view {
            let paths = 0..(self.path_index.path_names.len().min(64));
            let gpu_buffer = self.path_viz_cache.get("depth").unwrap();

            Self::sample_into_data_buffer(
                state,
                &self.path_index,
                &self.depth_data,
                paths,
                self.view.clone(),
                gpu_buffer,
            )
            .unwrap();

            self.rendered_view = self.view.clone();
        }

        // TODO debug FlexLayout rendering should use a render graph
        self.egui.run(window, |ctx| {
            let painter = ctx.debug_painter();

            let size = window.inner_size();
            let size =
                ultraviolet::Vec2::new(size.width as f32, size.height as f32);

            let stroke = egui::Stroke {
                width: 1.0,
                color: egui::Color32::RED,
            };

            let result = self.slot_layout.visit_layout(size, |layout, elem| {
                let rect = crate::gui::layout_egui_rect(&layout);

                painter.rect_stroke(rect, egui::Rounding::default(), stroke);

                match elem {
                    GuiElem::PathSlot {
                        slot_id,
                        path_id,
                        data,
                    } => {
                        // TODO
                    }
                    GuiElem::PathName { path_id } => {
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
                    GuiElem::Label { id } => {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            id,
                            egui::FontId::monospace(16.0),
                            egui::Color32::WHITE,
                        );
                    }
                }
            });
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

        // if self.touch.on_event(window_dims, event) {
        //     consume = true;
        // }

        if let WindowEvent::KeyboardInput { input, .. } = event {
            if let Some(key) = input.virtual_keycode {
                use winit::event::ElementState;
                use winit::event::VirtualKeyCode as Key;
                let pressed = matches!(input.state, ElementState::Pressed);

                let mut l = self.view.start;
                let mut r = self.view.end;
                let len = r - l;

                if pressed {
                    match key {
                        Key::Right => {
                            r = (r + len / 10).min(self.pangenome_len);
                            l = r.checked_sub(len).unwrap_or_default();
                        }
                        Key::Left => {
                            l = l.checked_sub(len / 10).unwrap_or_default();
                            r = l + len;
                        }
                        _ => (),
                    }
                }

                self.view = l..r;
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
            let (buffer, insts) = Self::slot_vertices(
                &state.device,
                new_size,
                &mut self.slot_layout,
            )?;
            let vxs = 0..6;
            let insts = 0..insts;

            (buffer, vxs, insts)
        };

        self.render_graph.set_node_preprocess_fn(
            self.draw_path_slot,
            move |_ctx, op_state| {
                // op_state.vertices = Some(0..6);
                // op_state.instances = Some(0..10);
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
                "frag_cfg".into(),
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
    args: Args,
) -> Result<Box<dyn crate::AppWindow>> {
    let path_index = PathIndex::from_gfa(&args.gfa)?;

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
