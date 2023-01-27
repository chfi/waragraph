use crate::app::resource::GraphPathData;
use crate::app::{AppWindow, SharedState, VizInteractions};
use crate::color::ColorMapping;
use crate::gui::list::DynamicListLayout;
use crate::gui::FlexLayout;
use crate::list::ListView;
use crate::util::BufferDesc;
use crossbeam::atomic::AtomicCell;
use taffy::style::Dimension;
use waragraph_core::graph::{Bp, PathId};
use wgpu::BufferUsages;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use winit::event::WindowEvent;

use raving_wgpu::graph::dfrog::{Graph, InputResource};
use raving_wgpu::gui::EguiCtx;
use raving_wgpu::{NodeId, State, WindowState};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use anyhow::Result;

use waragraph_core::graph::PathIndex;

use self::util::path_sampled_data_viz_buffer;
use self::view::View1D;

pub mod gui;

pub mod util;
pub mod view;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
}

pub struct Viewer1D {
    render_graph: Graph,
    draw_path_slot: NodeId,

    view: View1D,
    rendered_view: std::ops::Range<u64>,

    force_resample: bool,

    vertices: BufferDesc,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,

    gpu_buffers: HashMap<String, BufferDesc>,

    dyn_slot_layout: DynamicListLayout<Vec<gui::SlotElem>, gui::SlotElem>,

    path_list_view: ListView<PathId>,

    sample_handle:
        Option<tokio::task::JoinHandle<(std::ops::Range<u64>, Vec<u8>)>>,

    pub self_viz_interact: Arc<AtomicCell<VizInteractions>>,
    pub connected_viz_interact: Option<Arc<AtomicCell<VizInteractions>>>,

    shared: SharedState,

    active_viz_data_key: String,
    data_color_mappings: HashMap<String, ColorMapping>,

    color_sampler: wgpu::Sampler,

    new_color_mapping: crate::util::Uniform<NewColorMap, 16>,
}

#[derive(
    Clone, Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
struct NewColorMap {
    value_range: [f32; 2],
    color_range: [f32; 2],
}

impl Viewer1D {
    pub fn init(
        win_dims: [u32; 2],
        state: &State,
        window: &WindowState,
        path_index: Arc<PathIndex>,
        shared: &SharedState,
    ) -> Result<Self> {
        let mut graph = Graph::new();

        let draw_schema = {
            let vert_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d.vert.spv"
            ));
            let frag_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                // "/shaders/path_slot_1d_color_map.frag.spv"
                "/shaders/path_slot_1d_color_map_new.frag.spv"
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
                &[window.surface_format],
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

        graph.add_link_from_transient("viz_data_buffer", draw_node, 3);
        graph.add_link_from_transient("sampler", draw_node, 4);
        graph.add_link_from_transient("color_texture", draw_node, 5);
        graph.add_link_from_transient("color_mapping", draw_node, 6);
        graph.add_link_from_transient("transform", draw_node, 7);

        /*
        graph.add_link_from_transient("color", draw_node, 4);
        graph.add_link_from_transient("color_mapping", draw_node, 5);
        graph.add_link_from_transient("transform", draw_node, 6);
        */

        let pangenome_len = path_index.pangenome_len().0;

        let len = pangenome_len as u64;
        let view = View1D::new(len);

        let paths = 0..path_index.path_names.len();

        // TODO: instead of setting the initial rows to 256, make the data/sampling
        // buffer reallocate when needed!!
        let path_list_view =
            ListView::new(paths.clone().map(PathId::from), Some(256));

        let graph_data_cache = shared.graph_data_cache.clone();

        // let active_viz_data_key = "strand".to_string();
        let active_viz_data_key = "depth".to_string();

        let viz_data_buffer = {
            let paths =
                path_list_view.visible_iter().copied().collect::<Vec<_>>();

            let view_range = view.range().clone();

            let path_index = path_index.clone();
            let data = graph_data_cache
                .fetch_path_data_blocking(&active_viz_data_key)
                .unwrap();

            path_sampled_data_viz_buffer(
                &state.device,
                &path_index,
                &data,
                paths,
                view.range().clone(),
                1024,
            )?
        };

        let mut gpu_buffers = HashMap::default();

        let strand_color_map = {
            let mut colors = shared.colors.blocking_write();

            let id = colors.get_color_scheme_id("black_red").unwrap();
            let scheme = colors.get_color_scheme(id);

            let color_range = 2..=5u32;
            let val_range = 0f32..=1.0;

            let mapping = ColorMapping::new(
                id,
                color_range,
                val_range,
                1,
                (scheme.colors.len() - 1) as u32,
            );

            // not really necessary to do here, but ensures it's ready
            let _buffer =
                colors.get_color_mapping_gpu_buffer(state, mapping).unwrap();

            colors.create_color_scheme_texture(state, "black_red");

            mapping
        };

        let depth_color_map = {
            let mut colors = shared.colors.blocking_write();

            let id = colors.get_color_scheme_id("spectral").unwrap();
            let scheme = colors.get_color_scheme(id);

            let color_range = 0..=((scheme.colors.len() - 1) as u32);
            let val_range = 1f32..=14.0;

            let mapping = ColorMapping::new(
                id,
                color_range,
                val_range,
                0,
                (scheme.colors.len() - 1) as u32,
            );

            let _buffer =
                colors.get_color_mapping_gpu_buffer(state, mapping).unwrap();

            colors.create_color_scheme_texture(state, "spectral");

            mapping
        };

        let mut data_color_mappings = HashMap::default();
        data_color_mappings.insert("strand".to_string(), strand_color_map);
        data_color_mappings.insert("depth".to_string(), depth_color_map);

        gpu_buffers.insert("viz_data_buffer".to_string(), viz_data_buffer);

        let dyn_slot_layout = {
            let width = |p: f32| taffy::style::Dimension::Percent(p);

            let mut layout: DynamicListLayout<
                Vec<gui::SlotElem>,
                gui::SlotElem,
            > = DynamicListLayout::new(
                [width(0.2), width(0.8)],
                |row: &Vec<gui::SlotElem>, ix| {
                    //
                    let val = row.get(ix)?.clone();

                    let mk_h = |h: f32| taffy::style::Dimension::Points(h);

                    // TODO: grab this from some sort of config
                    let slot_height = 20.0;
                    // let slot_height = 80.0;

                    let height = match &val {
                        gui::SlotElem::Empty => taffy::style::Dimension::Auto,
                        gui::SlotElem::ViewRange => mk_h(slot_height),
                        gui::SlotElem::PathData { slot_id, data_id } => {
                            mk_h(slot_height)
                        }
                        gui::SlotElem::PathName { slot_id } => {
                            mk_h(slot_height)
                        }
                    };

                    Some((val, height))
                },
            );

            layout
        };

        let (vertices, vxs, insts) = {
            let size =
                ultraviolet::Vec2::new(win_dims[0] as f32, win_dims[1] as f32);
            let (buffer, insts) = Self::slot_vertex_buffer(
                &state.device,
                size,
                dyn_slot_layout.layout(),
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

        let self_viz_interact =
            Arc::new(AtomicCell::new(VizInteractions::default()));
        let connected_viz_interact = None;

        let color_sampler = crate::color::create_linear_sampler(&state.device);

        let color_mapping = NewColorMap {
            value_range: [0.0, 13.0],
            color_range: [0.0, 1.0],
        };

        let new_color_mapping = crate::util::Uniform::new(
            &state,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "Viewer 1D Color Mapping",
            color_mapping,
            |cm| {
                let data: [u8; 16] = bytemuck::cast(*cm);
                data
            },
        )?;

        Ok(Viewer1D {
            render_graph: graph,
            draw_path_slot: draw_node,

            view: view.clone(),
            rendered_view: view.range().clone(),
            force_resample: false,

            vertices,
            vert_uniform,
            frag_uniform,

            gpu_buffers,

            dyn_slot_layout,
            path_list_view,

            sample_handle: None,

            self_viz_interact,
            connected_viz_interact,

            shared: shared.clone(),

            active_viz_data_key,
            data_color_mappings,

            color_sampler,
            new_color_mapping,
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
        data: &GraphPathData<f32>,
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
        data: &GraphPathData<f32>,
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

    // TODO there's no need to reallocate the buffer every time the list is scrolled...
    fn slot_vertex_buffer(
        device: &wgpu::Device,
        win_dims: ultraviolet::Vec2,
        layout: &FlexLayout<gui::SlotElem>,
        path_list_view: &ListView<PathId>,
    ) -> Result<(BufferDesc, u32)> {
        let mut data_buf: Vec<u8> = Vec::new();

        let stride = std::mem::size_of::<[f32; 5]>();

        layout.visit_layout(|layout, elem| {
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

        if self.dyn_slot_layout.layout().computed_size() != Some(dims) {
            let data_id = std::sync::Arc::new(self.active_viz_data_key.clone());

            let rows_iter =
                self.path_list_view.offset_to_end_iter().enumerate().map(
                    |(ix, _path)| {
                        // TODO: should get slot id via a cache keyed to paths;
                        // right now the entire set of rows gets resampled every change
                        let name = gui::SlotElem::PathName { slot_id: ix };
                        let data = gui::SlotElem::PathData {
                            slot_id: ix,
                            data_id: data_id.clone(),
                        };
                        vec![name, data]
                    },
                );

            let view_range_row =
                vec![gui::SlotElem::Empty, gui::SlotElem::ViewRange];

            let rows_iter = [view_range_row].into_iter().chain(rows_iter);

            let inner_offset = ultraviolet::Vec2::new(0.0, 4.0);
            let inner_dims = dims - inner_offset;

            match self.dyn_slot_layout.build_layout(
                inner_offset,
                inner_dims,
                rows_iter,
            ) {
                // TODO: prepend rows to fill out when scrolled all the way down
                // (need to add the "reverse" row_iter)
                // .and_then(|(rows_added, avail_height)| {
                //     self.dyn_slot_layout.prepend_rows
                //     todo!();
                //     //
                // }) {
                Ok((rows_added, avail_height)) => {
                    self.path_list_view.resize(rows_added);
                }
                Err(e) => {
                    log::error!("Slot layout error: {e:?}");
                }
            }

            let (vertices, vxs, insts) = {
                let (buffer, insts) = Self::slot_vertex_buffer(
                    &state.device,
                    dims,
                    self.dyn_slot_layout.layout(),
                    &self.path_list_view,
                )
                .expect("Unrecoverable error when creating slot vertex buffer");
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

            let uniform_data = [dims.x, dims.y];

            state.queue.write_buffer(
                &self.vert_uniform,
                0,
                bytemuck::cast_slice(uniform_data.as_slice()),
            );
        }

        if self.sample_handle.is_none()
            && (&self.rendered_view != self.view.range() || self.force_resample)
        {
            let paths = self
                .path_list_view
                .visible_iter()
                .copied()
                .collect::<Vec<_>>();

            let view_range = self.view.range().clone();

            let path_index = self.shared.graph.clone();
            let data = self
                .shared
                .graph_data_cache
                .fetch_path_data_blocking(&self.active_viz_data_key)
                .unwrap();

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
                let gpu_buffer =
                    self.gpu_buffers.get("viz_data_buffer").unwrap();
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

        egui_ctx.begin_frame(&window.window);

        let mut path_name_region = egui::Rect::NOTHING;
        let mut path_slot_region = egui::Rect::NOTHING;

        let mut shapes = Vec::new();

        let mut view_range_rect = None;

        let layout_result = {
            let fonts = egui_ctx.ctx().fonts();

            self.dyn_slot_layout.visit_layout(|layout, elem| {
                let rect = crate::gui::layout_egui_rect(&layout);

                let stroke = egui::Stroke {
                    width: 1.0,
                    color: egui::Color32::RED,
                };
                let dbg_rect = egui::Shape::rect_stroke(
                    rect,
                    egui::Rounding::default(),
                    stroke,
                );
                shapes.push(dbg_rect);

                // hacky fix for rows that are laid out beyond the limits of the view
                if !screen_rect.intersects(rect) {
                    return;
                }

                match elem {
                    gui::SlotElem::Empty => (),
                    gui::SlotElem::ViewRange => {
                        view_range_rect = Some(rect);
                    }
                    gui::SlotElem::PathData { .. } => {
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
                            .shared
                            .graph
                            .path_names
                            .get_by_left(path_id)
                            .unwrap();

                        let galley = crate::gui::util::fit_text_ellipsis(
                            &fonts,
                            path_name,
                            egui::FontId::monospace(16.0),
                            egui::Color32::WHITE,
                            rect.size().x,
                        );

                        let text_pos = rect.left_top();
                        let text_shape =
                            egui::epaint::TextShape::new(text_pos, galley);

                        let shape = egui::Shape::Text(text_shape);

                        shapes.push(shape);
                    }
                }
            })
        };

        if let Err(e) = layout_result {
            log::error!("GUI layout error: {e:?}");
        }

        {
            let ctx = egui_ctx.ctx();

            let mut fg_shapes = Vec::new();

            let main_area = egui::Area::new("main_area_1d")
                .order(egui::Order::Background)
                .interactable(true)
                .movable(false)
                .constrain(true);

            main_area.show(ctx, |ui| {
                let path_names =
                    ui.allocate_rect(path_name_region, egui::Sense::hover());

                let path_slots = ui.allocate_rect(
                    path_slot_region,
                    egui::Sense::click_and_drag(),
                );

                let sep_rect = {
                    let (top, btm) = path_name_region.y_range().into_inner();
                    let left = path_name_region.right();
                    let right = path_slot_region.left();

                    egui::Rect::from_min_max(
                        egui::pos2(left, top),
                        egui::pos2(right, btm),
                    )
                };

                let column_separator = ui
                    .allocate_rect(sep_rect, egui::Sense::click_and_drag())
                    .on_hover_cursor(egui::CursorIcon::ResizeColumn);

                if column_separator.hovered() {
                    let shape = egui::Shape::rect_filled(
                        sep_rect,
                        egui::Rounding::same(2.0),
                        egui::Color32::from_white_alpha(180),
                    );
                    fg_shapes.push(shape);
                }

                if column_separator.dragged_by(egui::PointerButton::Primary) {
                    let old_width = path_name_region.width();
                    let dx = column_separator.drag_delta().x;
                    let new_width = old_width + dx;
                    let new_p = new_width / old_width;

                    if let Dimension::Percent(p) =
                        &mut self.dyn_slot_layout.column_widths_mut()[0]
                    {
                        *p *= new_p;
                        *p = p.clamp(0.05, 0.95);
                    }

                    self.dyn_slot_layout.clear_layout();
                }

                let scroll = ui.input().scroll_delta;

                if path_names.hovered() {
                    // hardcoded row height for now; should be stored/fetched
                    let rows = (scroll.y / 20.0).round() as isize;

                    if rows != 0 {
                        self.path_list_view.scroll_relative(-rows);
                        self.force_resample = true;
                    }
                }

                if path_slots.dragged_by(egui::PointerButton::Primary) {
                    let dx =
                        path_slots.drag_delta().x / path_slot_region.width();
                    self.view.translate_norm_f32(-dx);
                }

                let mut interact = VizInteractions::default();

                if let Some(pos) = path_slots.hover_pos() {
                    let left = path_slot_region.left();
                    let width = path_slot_region.width();
                    let rel_x = (pos.x - left) / width;

                    let min_scroll = 1.0;
                    let factor = 0.01;
                    if scroll.y.abs() > min_scroll {
                        let dz = 1.0 - scroll.y * factor;
                        self.view.zoom_with_focus(rel_x, dz);
                    }

                    let pan_pos = self.view.offset()
                        + (rel_x * self.view.len() as f32) as u64;
                    let hovered_node =
                        self.shared.graph.node_at_pangenome_pos(Bp(pan_pos));

                    if path_slots.clicked() {
                        interact.clicked = true;
                    }

                    interact.interact_pan_pos = Some(Bp(pan_pos));
                    interact.interact_node = hovered_node;
                }

                if let Some(rect) = view_range_rect {
                    let fonts = ui.fonts();
                    let range = self.view.range();
                    let left = Bp(range.start);
                    let right = Bp(range.end);
                    shapes.extend(gui::view_range_shapes(
                        &fonts,
                        rect,
                        left,
                        right,
                        interact.interact_pan_pos,
                    ));
                }

                self.self_viz_interact.store(interact);
            });

            egui::Window::new("Visualization Modes").show(
                egui_ctx.ctx(),
                |ui| {
                    let mut path_data_sources = self
                        .shared
                        .graph_data_cache
                        .path_data_source_names()
                        .collect::<Vec<_>>();
                    path_data_sources.sort();

                    let mut current_key = self.active_viz_data_key.clone();

                    for key in path_data_sources {
                        if !self.data_color_mappings.contains_key(key) {
                            continue;
                        }

                        if ui
                            .add_enabled(
                                key != &current_key,
                                egui::Button::new(key),
                            )
                            .clicked()
                        {
                            current_key = key.to_string();
                        }
                    }

                    self.active_viz_data_key = current_key;
                },
            );

            egui::Window::new("Color Mapping").show(egui_ctx.ctx(), |ui| {
                let mut color_map = *self.new_color_mapping.data_ref();

                let [min_v, max_v] = color_map.value_range;

                let val_range = 0f32..=max_v;

                {
                    let s_min_v = egui::Slider::new(
                        &mut color_map.value_range[0],
                        val_range,
                    );

                    ui.add(s_min_v);
                }

                {
                    let val_range = min_v..=(max_v + 1.0);
                    let s_max_v = egui::Slider::new(
                        &mut color_map.value_range[1],
                        val_range,
                    );

                    ui.add(s_max_v);
                }

                {
                    let col_range = 0f32..=1f32;
                    let s_min_v = egui::Slider::new(
                        &mut color_map.color_range[0],
                        col_range,
                    );

                    ui.add(s_min_v);
                }

                {
                    let col_range = 0f32..=1f32;
                    let s_max_v = egui::Slider::new(
                        &mut color_map.color_range[1],
                        col_range,
                    );

                    ui.add(s_max_v);
                }

                // let val_range = 0f32..=max_v;
                // let s_max_v = egui::Slider::new(&mut max_v, val_range);

                self.new_color_mapping.update_data_maybe_write(|cm| {
                    let changed = *cm != color_map;
                    *cm = color_map;
                    changed
                });
            });

            let painter =
                egui_ctx.ctx().layer_painter(egui::LayerId::background());
            painter.extend(shapes);

            let painter = egui_ctx.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                "main_area_fg".into(),
            ));
            painter.extend(fg_shapes);
        }

        egui_ctx.end_frame(&window.window);
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
            }
        }

        consume
    }

    fn on_resize(
        &mut self,
        _state: &raving_wgpu::State,
        _old_window_dims: [u32; 2],
        _new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        self.new_color_mapping.write_buffer(&state);

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

        let (tex, tex_size) = {
            let colors = self.shared.colors.blocking_read();
            let mapping = self
                .data_color_mappings
                .get(&self.active_viz_data_key)
                .unwrap();
            let id = mapping.color_scheme;

            let scheme = colors.get_color_scheme(id);
            let size = [scheme.colors.len() as u32, 1];

            (colors.get_color_scheme_texture(id).unwrap(), size)
        };

        let sampler = &self.color_sampler;

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
                sampler: Some(sampler),
                texture: None,
                view: None,
            },
        );

        let color_map_buf = self.new_color_mapping.buffer();
        let buf_size = self.new_color_mapping.buffer_size();

        transient_res.insert(
            "color_mapping".into(),
            InputResource::Buffer {
                size: buf_size,
                stride: None,
                buffer: color_map_buf,
            },
        );

        for name in ["viz_data_buffer"] {
            if let Some(desc) = self.gpu_buffers.get(name) {
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
