use crate::annotations::AnnotationStore;
use crate::{PathIndex};
use egui::epaint::tessellator::path;
use egui_winit::EventResponse;


use std::collections::HashMap;
use std::path::PathBuf;

use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::window::Window;

use raving_wgpu::camera::{DynamicCamera2d, TouchHandler, TouchOutput};
use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State};
// use raving_wgpu as wgpu;
use wgpu::util::DeviceExt;

use anyhow::Result;

use ultraviolet::*;


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
}

impl Viewer1D {
    fn init(
        event_loop: &EventLoopWindowTarget<()>,
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
                Some("indices"),
                &[state.surface_format],
            )?
        };

        todo!();
    }
}

struct SlotBuffer {
    buffer: wgpu::Buffer,
}

struct SlotDataCache {
    buffers: Vec<wgpu::Buffer>,
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
