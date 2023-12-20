// use crate::annotations::{AnnotationId, GlobalAnnotationId};
// use crate::app::settings_menu::SettingsWindow;
// use crate::app::{AppWindow, SharedState};
use crate::color::ColorMap;
use crate::context::{ContextQuery, ContextState};
use crate::{ArrowGFAWrapped, RavingCtx, SegmentPositions, SharedState};
// use crate::gui::annotations::AnnotationListWidget;
use crate::util::BufferDesc;
// use crate::viewer_2d::config::Config;

use raving_wgpu::egui;
use raving_wgpu::wgpu;
use waragraph_core::arrow_graph::ArrowGFA;
use web_sys::OffscreenCanvas;

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use egui_winit::winit;
use raving_wgpu::camera::DynamicCamera2d;
use raving_wgpu::texture::Texture;

use wgpu::BufferUsages;
use winit::event::WindowEvent;

use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State, WindowState};

use wgpu::util::{BufferInitDescriptor, DeviceExt};

use anyhow::Result;

use ultraviolet::*;

use waragraph_core::graph::{Bp, Node, PathIndex};

use wasm_bindgen::{prelude::*, Clamped};

pub mod render;

// pub mod annotations;
// pub mod config;
pub mod control;
// pub mod gui;
pub mod layout;
// pub mod util;
pub mod view;

// pub mod lyon_path_renderer;

use control::ViewControlWidget;

use layout::NodePositions;

// use self::annotations::AnnotationLayer;
use self::render::PolylineRenderer;
use self::view::View2D;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub tsv: PathBuf,
    pub annotations: Option<PathBuf>,
}

#[wasm_bindgen]
pub struct GraphViewer {
    renderer: PolylineRenderer,

    geometry_buffers: GeometryBuffers,
    surface: Option<wgpu::Surface>,
    sampler: wgpu::Sampler,

    viewport: View2D,
}

#[wasm_bindgen]
pub struct GraphViewerData {
    buffers: render::PagedBuffers,
}

#[wasm_bindgen]
impl GraphViewer {
    pub fn new_with_color_data(
        raving: &RavingCtx,
        graph: &ArrowGFAWrapped,
        pos: &SegmentPositions,
        canvas: web_sys::HtmlCanvasElement,
        segment_colors: &[u32],
    ) -> Result<GraphViewer, JsValue> {
        if segment_colors.len() != graph.segment_count() {
            return Err(format!(
                "Expected {} values in color buffer, was {}",
                graph.segment_count(),
                segment_colors.len()
            )
            .into());
        }

        // let mut node_data = vec![0f32; graph.segment_count()];
        log::warn!("creating node data");

        let seg_count = graph.0.segment_count();
        log::warn!("seg count??? {seg_count}");
        // let mut node_data = vec![1f32; graph.segment_count()];

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        log::warn!("initializing graph viewer");
        let mut viewer = GraphViewer::initialize(
            &raving.gpu_state,
            // format,
            raving.surface_format,
            &graph.0,
            &pos.xs,
            &pos.ys,
            segment_colors,
        )
        .map_err(|err| -> JsValue {
            format!("Error initializing GraphViewer: {err:?}").into()
        })?;

        // let canvas = OffscreenCanvas::new(800, 600)?;
        log::warn!("initializing surface");
        // let canvas = offscreen_canvas;
        let surface = raving
            .gpu_state
            .instance
            // .create_surface_from_offscreen_canvas(canvas)
            .create_surface_from_canvas(canvas)
            .map_err(|err| {
                JsValue::from(format!("Error initializing surface: {err:?}"))
            })?;

        log::warn!("configuring surface");
        surface.configure(
            &raving.gpu_state.device,
            &surface
                .get_default_config(&raving.gpu_state.adapter, 800, 600)
                .expect("Error configuring surface"),
        );

        viewer.surface = Some(surface);
        // viewer.offscreen_canvas = Some(OffscreenCanvas::new(300, 150)?);

        Ok(viewer)
    }

    pub fn new_with_buffers(
        raving: &RavingCtx,
        positions: render::PagedBuffers,
        colors: render::PagedBuffers,
        canvas: web_sys::HtmlCanvasElement,
        view: View2D,
    ) -> Result<GraphViewer, JsValue> {
        let mut viewer = GraphViewer::initialize_with_buffers(
            &raving.gpu_state,
            raving.surface_format,
            positions,
            colors,
            view,
        )
        .map_err(|err| -> JsValue {
            format!("Error initializing GraphViewer: {err:?}").into()
        })?;

        log::warn!("initializing surface");
        // let canvas = offscreen_canvas;
        let surface = raving
            .gpu_state
            .instance
            // .create_surface_from_offscreen_canvas(canvas)
            .create_surface_from_canvas(canvas)
            .map_err(|err| {
                JsValue::from(format!("Error initializing surface: {err:?}"))
            })?;

        log::warn!("configuring surface");
        surface.configure(
            &raving.gpu_state.device,
            &surface
                .get_default_config(&raving.gpu_state.adapter, 800, 600)
                .expect("Error configuring surface"),
        );

        viewer.surface = Some(surface);

        Ok(viewer)
    }

    // pub fn new_depth_data(
    pub fn new_dummy_data(
        raving: &RavingCtx,
        graph: &ArrowGFAWrapped,
        pos: &SegmentPositions,
        canvas: web_sys::HtmlCanvasElement,
        // offscreen_canvas: OffscreenCanvas,
    ) -> Result<GraphViewer, JsValue> {
        // let mut node_data = vec![0f32; graph.segment_count()];
        log::warn!("creating node data");

        let seg_count = graph.0.segment_count();
        log::warn!("seg count??? {seg_count}");

        let color = 0xAAAAAAFFu32;
        // let color = 0xFF22BBFFu32;
        let mut node_data = vec![color; graph.segment_count()];

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        log::warn!("initializing graph viewer");
        let mut viewer = GraphViewer::initialize(
            &raving.gpu_state,
            // format,
            raving.surface_format,
            &graph.0,
            &pos.xs,
            &pos.ys,
            &node_data,
        )
        .map_err(|err| -> JsValue {
            format!("Error initializing GraphViewer: {err:?}").into()
        })?;

        // let canvas = OffscreenCanvas::new(800, 600)?;
        log::warn!("initializing surface");
        // let canvas = offscreen_canvas;
        let surface = raving
            .gpu_state
            .instance
            // .create_surface_from_offscreen_canvas(canvas)
            .create_surface_from_canvas(canvas)
            .map_err(|err| {
                JsValue::from(format!("Error initializing surface: {err:?}"))
            })?;

        log::warn!("configuring surface");
        surface.configure(
            &raving.gpu_state.device,
            &surface
                .get_default_config(&raving.gpu_state.adapter, 800, 600)
                .expect("Error configuring surface"),
        );

        viewer.surface = Some(surface);
        // viewer.offscreen_canvas = Some(OffscreenCanvas::new(300, 150)?);

        Ok(viewer)
    }

    pub fn resize(
        &mut self,
        raving: &RavingCtx,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        if let Some(surface) = self.surface.as_ref() {
            self.geometry_buffers = GeometryBuffers::allocate(
                &raving.gpu_state,
                [width, height],
                raving.surface_format,
            )
            .map_err(|err| {
                JsValue::from(format!(
                    "Error reallocating geometry buffers: {err:?}"
                ))
            })?;

            surface.configure(
                &raving.gpu_state.device,
                &surface
                    .get_default_config(
                        &raving.gpu_state.adapter,
                        width,
                        height,
                    )
                    .expect("Error configuring surface"),
            );
        }

        Ok(())
    }

    pub fn gbuffer_lookup(
        &self,
        raving: &RavingCtx,
        x: f32,
        y: f32,
    ) -> Result<u32, JsValue> {
        if let Some((node, offset)) = self
            .geometry_buffers
            .lookup(&raving.gpu_state.device, [x, y])
        {
            Ok(node.0)
        } else {
            Err(JsValue::NULL)
        }
    }

    pub fn get_view(&self) -> View2D {
        self.viewport
    }

    pub fn set_view(&mut self, view: &View2D) {
        self.viewport = *view;
    }

    pub fn get_view_matrix(
        &self,
        canvas_width: f32,
        canvas_height: f32,
    ) -> JsValue {
        self.viewport.to_js_mat3(canvas_width, canvas_height)
    }

    pub fn set_view_center(&mut self, x: f32, y: f32) {
        self.viewport.center = Vec2::new(x, y);
    }

    pub fn draw_to_surface(&mut self, raving: &RavingCtx) {
        let Some(surface) = self.surface.as_ref() else {
            return;
        };

        let Ok(texture) = surface.get_current_texture() else {
            return;
        };

        let width = texture.texture.width();
        let height = texture.texture.height();

        let node_width = 30.0;

        self.renderer.update_uniforms(
            &raving.gpu_state.queue,
            self.viewport.to_matrix(),
            [width as f32, height as f32],
            node_width,
        );

        let tex_view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let dims = [texture.texture.width(), texture.texture.height()];

        let mut encoder = raving.gpu_state.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("2D viewer command encoder"),
            },
        );

        self.draw(&raving.gpu_state, dims, &tex_view, &mut encoder);

        self.geometry_buffers.download_textures(&mut encoder);

        raving.gpu_state.queue.submit([encoder.finish()]);

        texture.present();
    }
}

impl GraphViewer {
    pub fn initialize_with_buffers(
        state: &raving_wgpu::State,
        surface_format: wgpu::TextureFormat,
        position_buffers: render::PagedBuffers,
        color_buffers: render::PagedBuffers,
        view: View2D,
    ) -> anyhow::Result<Self> {
        // let seg_count = graph.segment_count();

        let mut renderer = PolylineRenderer::new_with_buffers(
            &state.device,
            surface_format,
            position_buffers,
            color_buffers,
        )?;

        // let view = {
        //     let size = max_p - min_p;
        //     let center = min_p + (max_p - min_p) * 0.5;
        //     let view = View2D::new(center, size);
        //     view
        // };

        // renderer.set_transform(&state.queue, view.to_matrix());

        renderer.update_uniforms(
            &state.queue,
            view.to_matrix(),
            [800., 600.],
            50.0,
        );

        let geometry_buffers =
            GeometryBuffers::allocate(state, [800, 600], surface_format)?;

        let sampler = crate::color::create_linear_sampler(&state.device);

        let dimension = wgpu::TextureDimension::D1;
        let format = wgpu::TextureFormat::Rgba8Unorm;

        let label = format!("Texture - Color Scheme <TEMP>");

        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST;

        let color_scheme = crate::color::spectral_color_scheme();

        let pixel_data: Vec<_> = color_scheme
            .iter()
            .map(|&[r, g, b, a]| {
                [
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    (a * 255.0) as u8,
                ]
            })
            .collect();

        let width = color_scheme.len() as u32;

        let size = wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        };

        Ok(Self {
            renderer,
            geometry_buffers,
            surface: None,
            sampler,
            // offscreen_canvas: None,
            viewport: view,
        })
    }

    pub fn initialize(
        state: &raving_wgpu::State,
        surface_format: wgpu::TextureFormat,
        graph: &ArrowGFA,
        pos_x: &[f32],
        pos_y: &[f32],
        segment_colors: &[u32],
    ) -> anyhow::Result<Self> {
        let seg_count = graph.segment_count();

        let mut renderer =
            PolylineRenderer::new(&state.device, surface_format, seg_count)?;

        let points = {
            let x_iter = pos_x.chunks(2);
            let y_iter = pos_y.chunks(2);
            x_iter.zip(y_iter).map(|(xs, ys)| match (xs, ys) {
                ([x0, x1], [y0, y1]) => {
                    [Vec2::new(*x0, *y0), Vec2::new(*x1, *y1)]
                }
                _ => unreachable!(),
            })
        };

        let mut min_p = Vec2::broadcast(std::f32::MAX);
        let mut max_p = Vec2::broadcast(std::f32::MIN);

        let vertex_data = points
            .enumerate()
            .map(|(ix, p)| {
                min_p = min_p.min_by_component(p[0]).min_by_component(p[1]);
                max_p = max_p.max_by_component(p[0]).max_by_component(p[1]);
                let mut out = [0u8; 4 * 5];
                out[0..(4 * 4)].clone_from_slice(bytemuck::cast_slice(&p));
                out[(4 * 4)..]
                    .clone_from_slice(bytemuck::cast_slice(&[ix as u32]));
                out
            })
            .collect::<Vec<_>>();

        let data = bytemuck::cast_slice(vertex_data.as_slice());

        println!("uploading vertex data");
        renderer.upload_vertex_data(state, data)?;
        renderer.upload_segment_colors(state, segment_colors)?;

        let view = {
            let size = max_p - min_p;
            let center = min_p + (max_p - min_p) * 0.5;
            let view = View2D::new(center, size);
            view
        };

        // renderer.set_transform(&state.queue, view.to_matrix());

        renderer.update_uniforms(
            &state.queue,
            view.to_matrix(),
            [800., 600.],
            50.0,
        );

        let geometry_buffers =
            GeometryBuffers::allocate(state, [800, 600], surface_format)?;

        let sampler = crate::color::create_linear_sampler(&state.device);

        let dimension = wgpu::TextureDimension::D1;
        let format = wgpu::TextureFormat::Rgba8Unorm;

        let label = format!("Texture - Color Scheme <TEMP>");

        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST;

        let color_scheme = crate::color::spectral_color_scheme();

        let pixel_data: Vec<_> = color_scheme
            .iter()
            .map(|&[r, g, b, a]| {
                [
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    (a * 255.0) as u8,
                ]
            })
            .collect();

        let width = color_scheme.len() as u32;

        let size = wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        };

        Ok(Self {
            renderer,
            geometry_buffers,
            surface: None,
            sampler,
            // offscreen_canvas: None,
            viewport: view,
        })
    }

    pub fn draw(
        &mut self,
        state: &raving_wgpu::State,
        tgt_dims: [u32; 2],
        tgt_attch: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        if let Err(e) = self.renderer.create_bind_groups(&state.device) {
            log::error!("2D viewer render error: {e:?}");
        }

        let state = self.renderer.state.read();

        let node_id_attch =
            self.geometry_buffers.node_id_tex.view.as_ref().unwrap();

        let mut pass = self.renderer.graphics_node.interface.render_pass(
            &[
                (
                    tgt_attch,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                ),
                (
                    node_id_attch,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: true,
                    },
                ),
            ],
            encoder,
        )?;

        let [w, h] = tgt_dims;

        let viewport = egui::Rect::from_min_max(
            egui::pos2(0., 0.),
            egui::pos2(w as f32, h as f32),
        );

        PolylineRenderer::draw_in_pass_impl(&state, &mut pass, viewport);

        Ok(())
    }
}

pub struct Viewer2D {
    segment_renderer: PolylineRenderer,

    node_positions: Arc<NodePositions>,
    vertex_buffer: wgpu::Buffer,
    instance_count: usize,

    view: View2D,

    transform_uniform: wgpu::Buffer,
    // vert_config: wgpu::Buffer,
    pub(crate) geometry_bufs: GeometryBuffers,

    // render_graph: Graph,
    // draw_node: NodeId,
    shared: SharedState,

    // annotation_layer: AnnotationLayer,
    active_viz_data_key: String,
    color_mapping: crate::util::Uniform<ColorMap, 16>,
    data_buffer: wgpu::Buffer,

    view_control_widget: control::ViewControlWidget,

    pub msg_tx: flume::Sender<control::Msg>,
    msg_rx: flume::Receiver<control::Msg>,

    // cfg: Config,
    // annotation_list_widget: AnnotationListWidget,
    color_format: wgpu::TextureFormat,
}

/*
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
*/

pub(crate) struct GeometryBuffers {
    dims: [u32; 2],

    // pub(crate) node_color_tex: Arc<Texture>,
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

    // TODO: just have the geometry buffer store a CPU vector copy
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
                bytes_per_row: Some(aligned_width * stride),
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
                bytes_per_row: Some(aligned_width * stride),
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

    fn allocate(
        state: &raving_wgpu::State,
        dims: [u32; 2],
        color_format: wgpu::TextureFormat,
    ) -> Result<Self> {
        use wgpu::TextureUsages;

        let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;

        let width = dims[0] as usize;
        let height = dims[1] as usize;

        // let node_color_tex = Arc::new(Texture::new(
        //     &state.device,
        //     width,
        //     height,
        //     color_format,
        //     // wgpu::TextureFormat::Rgba8UnormSrgb,
        //     usage | wgpu::TextureUsages::TEXTURE_BINDING,
        //     Some("Viewer2D Node ID Attch."),
        // )?);

        let node_id_tex = Texture::new(
            &state.device,
            width,
            height,
            wgpu::TextureFormat::R32Uint,
            usage,
            Some("Viewer2D Node ID Attch."),
        )?;

        let node_uv_tex = Texture::new(
            &state.device,
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
            aligned_width,
            height,
            wgpu::TextureFormat::R32Uint,
            usage,
            Some("Viewer2D Node ID Copy Dst"),
        )?;

        let node_uv_copy_dst_tex = Texture::new(
            &state.device,
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
            // node_color_tex,
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
