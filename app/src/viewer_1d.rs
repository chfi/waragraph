use crate::annotations::AnnotationStore;
use egui::epaint::tessellator::path;
use egui_winit::EventResponse;
use wgpu::BufferUsages;

use std::collections::HashMap;
use std::path::PathBuf;

use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::window::Window;

use raving_wgpu::camera::{DynamicCamera2d, TouchHandler, TouchOutput};
use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use anyhow::Result;

use waragraph_core::graph::PathIndex;

pub mod sampling;

#[derive(Debug)]
pub struct Args {
    gfa: PathBuf,
}

struct Viewer1D {
    render_graph: Graph,
    egui: EguiCtx,
    path_index: PathIndex,
    draw_path_slot: NodeId,

    pangenome_len: usize,
    view: std::ops::Range<usize>,

    vertices: wgpu::Buffer,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,
}

impl Viewer1D {
    fn init(
        event_loop: &EventLoopWindowTarget<()>,
        win_dims: [u32; 2],
        state: &State,
        path_index: PathIndex,
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
        graph.add_link_from_transient("frag_cfg", draw_node, 3);

        let vertices = {
            let data = [100.0f32, 100.0, 200.0, 100.0];
            let usage = BufferUsages::VERTEX | BufferUsages::COPY_DST;

            let buffer =
                state.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[data]),
                    usage,
                });

            graph.set_node_preprocess_fn(draw_node, move |_ctx, op_state| {
                op_state.vertices = Some(0..6);
                op_state.instances = Some(0..1);
            });

            buffer
        };

        let egui = EguiCtx::init(event_loop, state, None);
        let pangenome_len = path_index.pangenome_len();

        Ok(Viewer1D {
            render_graph: graph,
            egui,
            path_index,
            draw_path_slot: draw_node,
            pangenome_len,
            view: 0..pangenome_len,
            vertices,
            vert_uniform,
            frag_uniform,
        })
    }

    fn update(&mut self, window: &winit::window::Window, dt: f32) {
        // TODO
    }

    fn on_event(&mut self, window_dims: [u32; 2], event: &WindowEvent) -> bool {
        let mut consume = false;

        // if self.touch.on_event(window_dims, event) {
        //     consume = true;
        // }

        consume
    }

    fn render(&mut self, state: &mut State) -> Result<()> {
        let dims = state.size;
        let size = [dims.width, dims.height];

        let mut transient_res: HashMap<String, InputResource<'_>> =
            HashMap::default();

        // let Some(buffers) = self.path_buffers.as_ref() else {
        //     return Ok(());
        // };

        if let Ok(output) = state.surface.get_current_texture() {
            // {
            //     let uniform_data = self.camera.to_matrix();
            //     state.queue.write_buffer(
            //         &self.uniform_buf,
            //         0,
            //         bytemuck::cast_slice(&[uniform_data]),
            //     );
            // }

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

            let v_stride = std::mem::size_of::<[f32; 4]>();
            let v_size = 1 * v_stride;
            transient_res.insert(
                "vertices".into(),
                InputResource::Buffer {
                    size: v_size,
                    stride: Some(v_stride),
                    buffer: &self.vertices,
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

struct SlotBuffer {
    buffer: wgpu::Buffer,
}

struct SlotDataCache {
    buffers: Vec<wgpu::Buffer>,
}
pub async fn run(args: Args) -> Result<()> {
    let (event_loop, window, mut state) = raving_wgpu::initialize().await?;

    let path_index = PathIndex::from_gfa(&args.gfa)?;
    // let layout = GfaLayout::from_layout_tsv(&args.tsv)?;

    let dims = {
        let s = window.inner_size();
        [s.width, s.height]
    };

    let mut app = Viewer1D::init(&event_loop, dims, &state, path_index)?;

    /*
    if let Some(bed) = args.annotations.as_ref() {
        app.annotations.fill_from_bed(bed)?;
        let cache = app
            .annotations
            .layout_positions(&app.path_index, &app.layout);
        app.annotation_cache = cache;
    }
    */

    let mut first_resize = true;
    let mut prev_frame_t = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match &event {
            Event::WindowEvent { window_id, event } => {
                let mut consumed = false;

                let size = window.inner_size();
                let dims = [size.width, size.height];
                // consumed = app.on_event(dims, event);

                if !consumed {
                    match &event {
                        WindowEvent::KeyboardInput { input, .. } => {
                            use VirtualKeyCode as Key;
                            if let Some(code) = input.virtual_keycode {
                                if let Key::Escape = code {
                                    *control_flow = ControlFlow::Exit;
                                }
                            }
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit
                        }
                        WindowEvent::Resized(phys_size) => {
                            // for some reason i get a validation error if i actually attempt
                            // to execute the first resize
                            if first_resize {
                                first_resize = false;
                            } else {
                                state.resize(*phys_size);
                            }
                        }
                        WindowEvent::ScaleFactorChanged {
                            new_inner_size,
                            ..
                        } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }

            Event::RedrawRequested(window_id) if *window_id == window.id() => {
                app.render(&mut state).unwrap();
            }
            Event::MainEventsCleared => {
                let dt = prev_frame_t.elapsed().as_secs_f32();
                prev_frame_t = std::time::Instant::now();

                app.update(&window, dt);

                window.request_redraw();
            }

            _ => {}
        }
    })
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}
