use crate::app::resource::GraphPathData;
use crate::app::settings_menu::SettingsWindow;
use crate::app::{AppWindow, SharedState, VizInteractions};
use crate::color::widget::{ColorMapWidget, ColorMapWidgetShared};
use crate::color::ColorMap;
use crate::gui::list::DynamicListLayout;
use crate::gui::FlexLayout;
use crate::list::ListView;
use crate::util::BufferDesc;
use crossbeam::atomic::AtomicCell;
use taffy::style::Dimension;
use tokio::sync::RwLock;
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

use self::cache::SlotCache;
use self::render::VizModeConfig;
// use self::util::path_sampled_data_viz_buffer;
use self::view::View1D;
use self::widgets::VisualizationModesWidget;

pub mod gui;
pub mod widgets;

pub mod cache;
pub mod render;

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

    slot_cache: SlotCache,

    // vertices: BufferDesc,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,

    dyn_slot_layout: DynamicListLayout<Vec<gui::SlotElem>, gui::SlotElem>,

    path_list_view: ListView<PathId>,

    // sample_handle:
    //     Option<tokio::task::JoinHandle<(std::ops::Range<u64>, Vec<u8>)>>,
    pub self_viz_interact: Arc<AtomicCell<VizInteractions>>,
    pub connected_viz_interact: Option<Arc<AtomicCell<VizInteractions>>>,

    shared: SharedState,

    // active_viz_data_key: String,
    active_viz_data_key: Arc<RwLock<String>>,

    // color_mapping: crate::util::Uniform<ColorMap, 16>,
    color_mapping: crate::util::Uniform<Arc<AtomicCell<ColorMap>>, 16>,
    // color_map_widget: Arc<RwLock<ColorMapWidgetShared>>,

    // NB: very temporary, hopefully
    viz_mode_config: HashMap<String, VizModeConfig>,
}

impl Viewer1D {
    pub fn init(
        win_dims: [u32; 2],
        state: &State,
        window: &WindowState,
        path_index: Arc<PathIndex>,
        shared: &SharedState,
        settings_window: &mut SettingsWindow,
    ) -> Result<Self> {
        let t0 = std::time::Instant::now();

        let mut graph = Graph::new();

        let draw_schema = {
            let vert_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d.vert.spv"
            ));
            let frag_src = include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/path_slot_1d_color_map.frag.spv"
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
        graph.add_link_from_transient("color_map", draw_node, 6);
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
                        gui::SlotElem::Annotations {
                            path,
                            annotation_id,
                        } => {
                            // TODO this should be dynamic
                            mk_h(slot_height)
                        }
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

        let color_mapping_val = Arc::new(AtomicCell::new(ColorMap {
            value_range: [0.0, 1.0],
            color_range: [0.0, 1.0],
        }));

        let color_mapping = crate::util::Uniform::new(
            &state,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "Viewer 1D Color Mapping",
            color_mapping_val.clone(),
            |color_map| {
                let data: [u8; 16] = bytemuck::cast(color_map.load());
                data
            },
        )?;

        /*
        let color_map_widget = {
            let scheme_id =
                shared.data_color_schemes.get(&active_viz_data_key).unwrap();

            let stats = shared
                .graph_data_cache
                .fetch_path_data_blocking(&active_viz_data_key)
                .as_ref()
                .unwrap()
                .global_stats;

            let mut map = color_mapping_val.load();
            map.value_range = [stats.min, stats.max];
            color_mapping_val.store(map);

            let color_map_widget = ColorMapWidgetShared::new(
                shared.colors.clone(),
                "Viewer1D-ColorMapWidget".into(),
                stats,
                &active_viz_data_key,
                *scheme_id,
                color_mapping_val.clone(),
            );

            let widget = Arc::new(RwLock::new(color_map_widget));

            settings_window.register_widget(
                "1D Viewer",
                "Color Map",
                widget.clone(),
            );

            widget
        };
        */

        let active_viz_data_key = Arc::new(RwLock::new(active_viz_data_key));

        {
            let viz_mode_widget = VisualizationModesWidget {
                shared: shared.clone(),
                active_viz_data_key: active_viz_data_key.clone(),
            };

            settings_window.register_widget(
                "1D Viewer",
                "Visualization Modes",
                Arc::new(RwLock::new(viz_mode_widget)),
            );
        }

        let viz_mode_config = {
            let colors = shared.colors.blocking_read();

            let mut cfg: HashMap<String, VizModeConfig> = HashMap::new();

            let depth = VizModeConfig {
                name: "depth".to_string(),
                data_key: "depth".to_string(),
                color_scheme: colors.get_color_scheme_id("spectral").unwrap(),
                default_color_map: ColorMap {
                    value_range: [1.0, 12.0],
                    color_range: [0.0, 1.0],
                },
            };

            let strand = VizModeConfig {
                name: "strand".to_string(),
                data_key: "strand".to_string(),
                color_scheme: colors.get_color_scheme_id("black_red").unwrap(),
                default_color_map: ColorMap {
                    value_range: [0.0, 1.0],
                    color_range: [0.0, 1.0],
                },
            };

            for c in [depth, strand] {
                cfg.insert(c.name.clone(), c);
            }

            cfg
        };

        log::error!("Initialized in {} seconds", t0.elapsed().as_secs_f32());

        let row_count = 128;
        let bin_count = 1024;
        let slot_cache = SlotCache::new(
            state,
            path_index.clone(),
            shared.graph_data_cache.clone(),
            row_count,
            bin_count,
        )?;

        Ok(Viewer1D {
            render_graph: graph,
            draw_path_slot: draw_node,

            view: view.clone(),
            rendered_view: view.range().clone(),
            force_resample: false,

            slot_cache,

            // vertices,
            vert_uniform,
            frag_uniform,

            dyn_slot_layout,
            path_list_view,

            // sample_handle: None,
            self_viz_interact,
            connected_viz_interact,

            shared: shared.clone(),

            active_viz_data_key,

            color_mapping,
            // color_map_widget,
            viz_mode_config,
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
                    let rrect = crate::gui::layout_egui_rect(&layout);
                    let v_pos = rrect.left_bottom().to_vec2();
                    let v_size = rrect.size();

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
        let mut laid_out_slots = Vec::new();
        let layout_result =
            self.dyn_slot_layout.layout().visit_layout(|layout, elem| {
                if let gui::SlotElem::PathData { slot_id, data_id } = elem {
                    if let Some(path_id) =
                        self.path_list_view.get_in_view(*slot_id)
                    {
                        let slot_key = (*path_id, data_id.clone());
                        let rect = crate::gui::layout_egui_rect(&layout);
                        laid_out_slots.push((slot_key, rect));
                    }
                }
            });

        // let layout = self.dyn_slot_layout.layout().visit_layout

        let update_result = self.slot_cache.sample_and_update(
            state,
            tokio_rt,
            &self.view,
            laid_out_slots,
        );

        // NB: disabling the color map widget for the time being
        /*
        {
            let mut color_map_widget = self.color_map_widget.blocking_write();

            let active_viz_data_key = self.active_viz_data_key.blocking_read();

            let data_cache = &self.shared.graph_data_cache;
            let stats_getter = |key: &str| {
                let data = data_cache.fetch_path_data_blocking(key)?;
                Some(data.global_stats)
            };

            let scheme_id = self
                .shared
                .data_color_schemes
                .get(active_viz_data_key.as_str())
                .unwrap();

            color_map_widget.update(
                stats_getter,
                active_viz_data_key.as_str(),
                *scheme_id,
            );

            self.color_mapping.write_buffer(state);
        }
        */

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
            let data_id = self.active_viz_data_key.blocking_read().clone();

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

            if let Err(e) = update_result {
                log::error!("Slot cache update error: {e:?}");
            }

            let insts = 0u32..self.slot_cache.vertex_count as u32;
            self.render_graph.set_node_preprocess_fn(
                self.draw_path_slot,
                move |_ctx, op_state| {
                    op_state.vertices = Some(0..6);
                    op_state.instances = Some(insts.clone());
                },
            );

            let uniform_data = [dims.x, dims.y];

            state.queue.write_buffer(
                &self.vert_uniform,
                0,
                bytemuck::cast_slice(uniform_data.as_slice()),
            );
        }

        // update uniform
        {
            let data = self.slot_cache.get_view_transform(&self.view);

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
                    gui::SlotElem::Annotations {
                        path,
                        annotation_id,
                    } => {
                        // TODO
                        //
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
        self.color_mapping.write_buffer(&state);

        let has_vertices = self.slot_cache.vertex_buffer.is_some();

        if !has_vertices {
            return Ok(());
        }

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

        let data_buffer = &self.slot_cache.data_buffer;
        transient_res.insert(
            "viz_data_buffer".into(),
            InputResource::Buffer {
                size: data_buffer.size,
                stride: None,
                buffer: &data_buffer.buffer,
            },
        );

        if let Some(vertices) = self.slot_cache.vertex_buffer.as_ref() {
            let v_stride = std::mem::size_of::<[f32; 5]>();
            transient_res.insert(
                "vertices".into(),
                InputResource::Buffer {
                    size: vertices.size,
                    stride: Some(v_stride),
                    buffer: &vertices.buffer,
                },
            );
        }

        transient_res.insert(
            "vert_cfg".into(),
            InputResource::Buffer {
                size: 2 * 4,
                stride: None,
                buffer: &self.vert_uniform,
            },
        );

        let (sampler, tex, tex_size) = {
            let colors = self.shared.colors.blocking_read();

            let sampler = colors.linear_sampler.clone();

            let data_key = self.active_viz_data_key.blocking_read().clone();

            let id = self.shared.data_color_schemes.get(&data_key).unwrap();

            let scheme = colors.get_color_scheme(*id);
            let size = [scheme.colors.len() as u32, 1];

            (sampler, colors.get_color_scheme_texture(*id).unwrap(), size)
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
            log::error!("Render graph validation error");
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
