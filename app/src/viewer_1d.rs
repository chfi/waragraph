use crate::annotations::GlobalAnnotationId;
use crate::app::settings_menu::SettingsWindow;
use crate::app::{AppWindow, SharedState};
use crate::color::ColorMap;
use crate::context::{ContextQuery, ContextState};
use crate::gui::{GridEntry, RowEntry, RowGridLayout};
use crate::list::ListView;
use crate::viewer_1d::annotations::AnnotSlot;
use crossbeam::atomic::AtomicCell;
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

use self::cache::{SlotCache, SlotState};
use self::control::ViewControlWidget;
use self::render::VizModeConfig;
// use self::util::path_sampled_data_viz_buffer;
use self::view::View1D;
use self::widgets::VisualizationModesWidget;

pub mod annotations;
pub mod cache;
pub mod control;
pub mod gui;
pub mod render;
pub mod sampler;
pub mod util;
pub mod view;
pub mod widgets;

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
}

pub struct Viewer1D {
    render_graph: Graph,
    draw_path_slot: NodeId,

    view: View1D,

    force_resample: bool,

    slot_cache: SlotCache,

    // vertices: BufferDesc,
    vert_uniform: wgpu::Buffer,
    frag_uniform: wgpu::Buffer,

    path_list_view: ListView<PathId>,

    shared: SharedState,

    // active_viz_data_key: String,
    active_viz_data_key: Arc<RwLock<String>>,
    use_linear_sampler: Arc<AtomicCell<bool>>,

    color_mapping: crate::util::Uniform<Arc<AtomicCell<ColorMap>>, 16>,

    annotations: annotations::Annots1D,

    pub msg_tx: crossbeam::channel::Sender<control::Msg>,
    msg_rx: crossbeam::channel::Receiver<control::Msg>,

    // NB: very temporary, hopefully; bits are spread all over...
    viz_mode_config: HashMap<String, VizModeConfig>,
    viz_samplers: HashMap<String, Arc<dyn sampler::Sampler + 'static>>,

    // NB: also temporary, hopefully
    view_control_widget: ViewControlWidget,
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
                &[wgpu::ColorTargetState {
                    format: window.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::all(),
                }],
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

        let active_viz_data_key = "path_name".to_string();

        graph.set_node_preprocess_fn(draw_node, move |_ctx, op_state| {
            op_state.vertices = Some(0..6);
            op_state.instances = Some(0..0);
        });

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
        let use_linear_sampler = Arc::new(AtomicCell::new(false));

        {
            let viz_mode_widget = VisualizationModesWidget {
                shared: shared.clone(),
                active_viz_data_key: active_viz_data_key.clone(),
                use_linear_sampler: use_linear_sampler.clone(),
            };

            settings_window.register_widget(
                "1D Viewer",
                "Visualization Modes",
                Arc::new(RwLock::new(viz_mode_widget)),
            );
        }

        let mut viz_samplers = HashMap::default();

        {
            let sampler = sampler::PathDataSampler::new(
                shared.graph.clone(),
                shared.graph_data_cache.clone(),
                "depth",
            );

            viz_samplers.insert(
                "depth".to_string(),
                Arc::new(sampler) as Arc<dyn sampler::Sampler + 'static>,
            );
        }

        let mut viz_mode_config = {
            let colors = shared.colors.blocking_read();

            let mut cfg: HashMap<String, VizModeConfig> = HashMap::new();

            let depth = VizModeConfig {
                name: "depth".to_string(),
                data_key: "depth".to_string(),
                color_scheme: colors.get_color_scheme_id("spectral").unwrap(),
                default_color_map: ColorMap {
                    value_range: [0.0, 13.0],
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

        let row_count = 512;
        let bin_count = 1024;
        let slot_cache = SlotCache::new(
            state,
            path_index.clone(),
            shared.graph_data_cache.clone(),
            row_count,
            bin_count,
        )?;

        let annotations = annotations::Annots1D::default();

        util::init_path_name_hash_viz_mode(
            state,
            shared,
            &mut viz_samplers,
            &mut viz_mode_config,
        );

        let (msg_tx, msg_rx) = crossbeam::channel::unbounded();

        let view_control_widget =
            ViewControlWidget::new(shared, msg_tx.clone());

        Ok(Viewer1D {
            render_graph: graph,
            draw_path_slot: draw_node,

            view: view.clone(),
            force_resample: false,

            slot_cache,

            // vertices,
            vert_uniform,
            frag_uniform,

            path_list_view,

            // sample_handle: None,
            shared: shared.clone(),

            annotations,

            msg_tx,
            msg_rx,

            view_control_widget,

            viz_mode_config,
            viz_samplers,

            active_viz_data_key,
            use_linear_sampler,

            color_mapping,
            // color_map_widget,
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
}

impl Viewer1D {
    const COLUMN_SEPARATOR_ID: &'static str = "Viewer1D-Column-Separator";
}

impl AppWindow for Viewer1D {
    fn update(
        &mut self,
        tokio_rt: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        context_state: &mut ContextState,
        dt: f32,
    ) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                control::Msg::View(cmd) => {
                    cmd.apply(&self.shared, &mut self.view)
                }
            }
        }

        egui_ctx.begin_frame(&window.window);

        let time = egui_ctx.ctx().input(|i| i.time);

        /* >> TODO <<
          [x] Prepare annotation slots for relevant paths
          [x] Path name slots
          [x] View range slot
          [x] Annotation slots
          [ ] Fit left column to names
          [ ] Draggable name column separator
          [ ] Annotation highlight in path slot on hover
        */

        let [width, height]: [u32; 2] = window.window.inner_size().into();
        let pixels_per_point = egui_ctx.ctx().pixels_per_point();
        let dims = ultraviolet::Vec2::new(width as f32, height as f32)
            / pixels_per_point;

        let screen_rect = egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(dims.x, dims.y),
        );

        let mut shapes = Vec::new();

        let (main_panel_rect, side_panel_rect) = {
            // for now do the side panel stuff here, and use it to
            // derive the main panel size

            let y_range = screen_rect.y_range();
            let (xl, _xr) = screen_rect.x_range().into_inner();

            let side_panel = egui::SidePanel::right("Viewer1D-side-panel")
                .show(egui_ctx.ctx(), |ui| {
                    self.view_control_widget.show(ui);
                });

            let side_panel_rect = side_panel.response.rect;

            let xmid = side_panel_rect.left();

            let main_panel_rect =
                egui::Rect::from_x_y_ranges(xl..=xmid, y_range);

            (main_panel_rect, side_panel_rect)
        };

        // let main_view_rect = screen_rect.shrink(2.0);
        let main_view_rect = main_panel_rect.shrink(2.0);

        let row_grid_layout = {
            use taffy::prelude::*;
            let data_id = self.active_viz_data_key.blocking_read().clone();

            let mut row_grid_layout: RowGridLayout<gui::SlotElem> =
                RowGridLayout::new();

            let info_col_width = {
                let id = egui::Id::new(Self::COLUMN_SEPARATOR_ID);
                egui_ctx.ctx().memory_mut(|mem| {
                    let width = mem.data.get_temp_mut_or(id, 150f32);
                    *width
                })
            };

            let header_row = {
                RowEntry {
                    grid_template_columns: vec![
                        points(info_col_width),
                        fr(1.0),
                    ],
                    grid_template_rows: vec![points(20.0)],
                    column_data: vec![GridEntry::new(
                        [1, 2],
                        gui::SlotElem::ViewRange,
                    )],
                    ..RowEntry::default()
                }
            };
            let view_offset = self.path_list_view.offset();

            let layout_result = row_grid_layout.fill_from_slice_index(
                main_view_rect.height(),
                [header_row],
                &self.path_list_view.as_slice(),
                view_offset,
                |&(_list_ix, path_id)| {
                    let mut row_entry = RowEntry {
                        grid_template_columns: vec![
                            points(info_col_width),
                            fr(1.0),
                        ],
                        grid_template_rows: vec![points(20.0)],
                        column_data: vec![],
                        ..RowEntry::default()
                    };

                    let mut data_row = 1;

                    if let Some(a_slot_id) =
                        self.annotations.get_path_slot_id(path_id)
                    {
                        // println!("adding annot slot");
                        // if annotation slot is present, change the grid_template_row field
                        // and append the extra column data
                        row_entry.grid_template_rows.insert(0, points(50.0));

                        row_entry.column_data.push(GridEntry::new(
                            [1, 2],
                            gui::SlotElem::Annotations {
                                annotation_slot_id: a_slot_id,
                            },
                        ));

                        data_row = 2;
                    }

                    // add path name and path data
                    row_entry.column_data.extend([
                        GridEntry::new(
                            [data_row, 1],
                            gui::SlotElem::PathName { path_id },
                        ),
                        GridEntry::new(
                            [data_row, 2],
                            gui::SlotElem::PathData {
                                path_id,
                                data_id: data_id.clone(),
                            },
                        ),
                    ]);

                    row_entry
                },
            );

            if let Ok((range, _height_rem)) = layout_result {
                self.path_list_view.resize(range.len());
            }

            let layout_result = row_grid_layout.compute_layout(main_view_rect);

            if let Err(e) = layout_result {
                log::error!("{e:?}");
            }

            /*
            let _debug_layout_result = row_grid_layout.visit_layout(|layout, elem| {
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
            });
            */

            row_grid_layout
        };

        let mut data_slots: HashMap<_, Vec<_>> = HashMap::new();
        let mut viz_slot_rect_map = HashMap::new();

        let query = ContextQuery::from_source::<(PathId, GlobalAnnotationId)>(
            "Viewer1D",
        );

        let hovered_annot =
            context_state.get_cast::<_, (PathId, GlobalAnnotationId)>(&query);

        let mut annot_slots = Vec::new();

        let mut view_range_rect = None;

        let mut path_name_slots: HashMap<PathId, egui::Rect> =
            HashMap::default();

        let mut path_name_region = egui::Rect::NOTHING;
        let mut path_slot_region = egui::Rect::NOTHING;

        egui_ctx.ctx().fonts(|fonts| {
            let _ = row_grid_layout.visit_layout(|layout, elem| {
                let rect = crate::gui::layout_egui_rect(&layout);

                match elem {
                    gui::SlotElem::Empty => {}
                    gui::SlotElem::ViewRange => {
                        view_range_rect = Some(rect);
                    }
                    gui::SlotElem::PathData { path_id, data_id } => {
                        let rect = crate::gui::layout_egui_rect(&layout);
                        path_slot_region = path_slot_region.union(rect);

                        let key = data_id.to_string();
                        data_slots
                            .entry(key)
                            .or_default()
                            .push((*path_id, rect));
                        viz_slot_rect_map
                            .insert((*path_id, data_id.to_string()), rect);

                        if let Some((path, g_annot_id)) = hovered_annot {
                            if path == path_id {
                                // draw regions here
                                let annot_slot_id = self
                                    .annotations
                                    .get_path_slot_id(*path_id)
                                    .unwrap();

                                let regions = self
                                    .annotations
                                    .get(&annot_slot_id)
                                    .and_then(|slot| {
                                        slot.annotation_ranges
                                            .get(&g_annot_id.annot_id)
                                    });

                                let slot_x_range = rect.x_range();
                                let color = egui::Rgba::from_rgba_unmultiplied(
                                    0.8, 0.2, 0.2, 0.5,
                                );

                                shapes.extend(
                                    regions
                                        .into_iter()
                                        .flatten()
                                        .filter_map(|range| {
                                            self.view
                                                .map_bp_interval_to_screen_x(
                                                    range,
                                                    &slot_x_range,
                                                )
                                        })
                                        .map(|slot_space_range| {
                                            gui::fill_h_range_of_rect(
                                                color,
                                                rect,
                                                slot_space_range,
                                            )
                                        }),
                                );
                            }
                        }
                    }
                    gui::SlotElem::PathName { path_id } => {
                        path_name_slots.insert(*path_id, rect);
                        path_name_region = path_name_region.union(rect);

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
                        let text_shape = egui::Shape::Text(
                            egui::epaint::TextShape::new(text_pos, galley),
                        );

                        shapes.push(text_shape);
                    }
                    gui::SlotElem::Annotations { annotation_slot_id } => {
                        annot_slots.push((*annotation_slot_id, rect));
                    }
                }
            });
        });

        for (data_key, path_rects) in data_slots {
            let sampler = self.viz_samplers.get(&data_key).unwrap().clone();
            let result = self.slot_cache.sample_with(
                state,
                tokio_rt,
                &self.view,
                data_key.as_str(),
                path_rects.iter().map(|(path, _)| *path),
                sampler,
            );

            // let result = self.slot_cache.sample_for_data(
            //     state,
            //     tokio_rt,
            //     &self.view,
            //     data_key.as_str(),
            //     path_rects.iter().map(|(path, _)| *path),
            // );
        }

        {
            let _slot_update_result = self.slot_cache.update(
                state,
                tokio_rt,
                &self.view,
                &viz_slot_rect_map,
            );

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

        {
            let annotations = self.shared.annotations.blocking_read();

            for slot_key in viz_slot_rect_map.keys() {
                let (path, _data_key) = slot_key;
                let path = *path;
                // initialize annotation slots if necessary; this part is kinda janky

                let has_annot_slot =
                    self.annotations.get_path_slot_id(path).is_some();

                // TODO: annotations should be able to be sourced from
                // any annot. set for a slot, and not only be
                // initialized once and then forgotten about
                if let Some((set_id, set)) =
                    annotations.get_sets_for_path(path).next()
                {
                    if !has_annot_slot {
                        if let Some(annots) = set.path_annotations.get(&path) {
                            let annot_items = annots
                                .iter()
                                .filter_map(|&i| set.annotations.get(i))
                                .map(|(range, label)| {
                                    let shape_fn =
                                        annotations::text_shape(&label);
                                    (path, range.clone(), shape_fn)
                                });

                            let annot_slot = AnnotSlot::new_from_path_space(
                                &self.shared.graph,
                                set_id,
                                annot_items,
                            );

                            self.annotations.insert_slot(path, annot_slot);
                        }
                    }
                }

                // add spinners
                if self.slot_cache.slot_task_running(slot_key) {
                    if let Some(rect) = path_name_slots.get(&path) {
                        let right_center = rect.right_center();
                        let spin_offset = right_center - egui::pos2(9.0, 0.0);

                        let stroke =
                            egui::Stroke::new(2.0, egui::Color32::WHITE);

                        let t = time as f32;

                        shapes.push(crate::gui::util::spinner(
                            stroke,
                            spin_offset,
                            t,
                        ));
                    }
                }
            }
        }

        // add ruler
        {
            let query = ContextQuery::from_tags::<Bp>(["hover"]);

            let pan_pos = context_state.get_cast::<_, Bp>(&query);
            if let Some(pos) = pan_pos {
                let p = pos.0 as f32;
                let vrange = self.view.range();
                let l = vrange.start as f32;
                let r = vrange.end as f32;
                let t = (p - l) / (r - l);

                let (sl, sr) = path_slot_region.x_range().into_inner();
                let x = sl + t * (sr - sl);

                let (y0, y1) = path_slot_region.y_range().into_inner();

                shapes.push(egui::Shape::line_segment(
                    [egui::pos2(x, y0), egui::pos2(x, y1)],
                    egui::Stroke::new(1.5, egui::Color32::RED),
                ));
            }
        }

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

        // update uniform
        {
            let data = self.slot_cache.get_view_transform(&self.view);

            state.queue.write_buffer(
                &self.frag_uniform,
                0,
                bytemuck::cast_slice(&data),
            );
        }

        egui_ctx.ctx().fonts(|fonts| {
            let show_state = |state: &SlotState| {
                let msg = state.last_msg.as_ref()?;
                let rect = state.last_rect?;
                let _ = state.last_updated_view.is_none().then_some(())?;

                let pos = rect.left_center();
                let anchor = egui::Align2::LEFT_CENTER;
                Some(egui::Shape::text(
                    &fonts,
                    pos,
                    anchor,
                    msg,
                    egui::FontId::monospace(16.0),
                    egui::Color32::WHITE,
                ))
            };
            self.slot_cache.update_displayed_messages(show_state);
        });

        shapes.extend(self.slot_cache.msg_shapes.drain(..));

        {
            let ctx = egui_ctx.ctx();

            // let mut fg_shapes = Vec::new();

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

                let scroll = ui.input(|i| i.scroll_delta);

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

                    if let Some(node) = hovered_node {
                        context_state.set("Viewer1D", ["hover"], node);

                        if path_slots.clicked() {
                            context_state.set("Viewer1D", ["click"], node);
                        }
                    }
                    context_state.set("Viewer1D", ["hover"], Bp(pan_pos));

                    // searches through all the slots, which probably isn't a problem,
                    // but it's annoying
                    let hovered_path = viz_slot_rect_map.iter().find_map(
                        |((path, _), rect)| rect.contains(pos).then_some(*path),
                    );

                    if let Some((path, node)) = hovered_path.zip(hovered_node) {
                        let (n_start, _n_end) =
                            self.shared.graph.node_offset_length(node);

                        let add_offset =
                            n_start.0.checked_sub(pan_pos).unwrap_or_default();

                        let step_offset = self
                            .shared
                            .graph
                            .node_path_step_offsets(node, path)
                            .and_then(|mut iter| iter.next());

                        if let Some((step, offset)) = step_offset {
                            context_state.set(
                                "Viewer1D",
                                ["hover"],
                                (node, path, step, Bp(offset.0 + add_offset)),
                            );
                        }
                    }
                }

                //
                if let Some(rect) = view_range_rect {
                    let range = self.view.range();
                    let left = Bp(range.start);
                    let right = Bp(range.end);

                    let interact_pos = context_state
                        .query_get_cast::<_, Bp>(Some("Viewer1D"), ["hover"])
                        .copied();

                    ui.fonts(|fonts| {
                        shapes.extend(gui::view_range_shapes(
                            &fonts,
                            rect,
                            left,
                            right,
                            interact_pos,
                        ));
                    });
                }

                for &(slot_id, rect) in annot_slots.iter() {
                    if let Some(annot_slot) = self.annotations.get_mut(&slot_id)
                    {
                        let painter = ui.painter_at(rect);

                        let cursor_pos =
                            ui.input(|input| input.pointer.hover_pos());
                        let interacted =
                            annot_slot.draw(&painter, &self.view, cursor_pos);

                        if let Some(annot_id) = interacted {
                            let set = annot_slot.set_id;
                            let global_id =
                                GlobalAnnotationId { set, annot_id };

                            let path = self
                                .annotations
                                .get_annotation_slot_path(slot_id)
                                .unwrap();

                            let ctx_data = (path, global_id);

                            context_state.set("Viewer1D", ["hover"], ctx_data);
                        }
                    }
                }

                {
                    let left = path_name_region.right();
                    let right = path_slot_region.left();
                    let mid = left + (right - left) * 0.5;
                    let (top, btm) = path_name_region.y_range().into_inner();

                    let sep_rect = egui::Rect::from_min_max(
                        egui::pos2(mid - 1.0, top),
                        egui::pos2(mid + 1.0, btm),
                    );

                    let column_separator = ui
                        .allocate_rect(sep_rect, egui::Sense::click_and_drag())
                        .on_hover_cursor(egui::CursorIcon::ResizeColumn);

                    if column_separator.hovered {
                        let shape = egui::Shape::line_segment(
                            [sep_rect.center_top(), sep_rect.center_bottom()],
                            egui::Stroke::new(
                                1.0,
                                egui::Color32::from_white_alpha(180),
                            ),
                        );
                        shapes.push(shape);
                    }
                    let dx = column_separator.drag_delta().x;

                    if column_separator.dragged_by(egui::PointerButton::Primary)
                    {
                        let id = egui::Id::new(Self::COLUMN_SEPARATOR_ID);
                        ui.memory_mut(|mem| {
                            let width = mem.data.get_temp_mut_or(id, 150f32);
                            let max_width = main_view_rect.width() - 10.0;
                            *width = (*width + dx).clamp(50.0, max_width);
                        });
                    }
                };
            });

            // path position tooltip (temporary)
            {
                use waragraph_core::graph::Node;
                let query =
                    ContextQuery::from_source::<(Node, PathId, usize, Bp)>(
                        "Viewer1D",
                    );

                let step_ctx = context_state
                    .get_cast::<_, (Node, PathId, usize, Bp)>(&query);

                if let Some((node, path, _step, pos)) = step_ctx {
                    egui::containers::popup::show_tooltip(
                        egui_ctx.ctx(),
                        egui::Id::new("Viewer1D-PathPos-Tooltip"),
                        |ui| {
                            let path_name = self
                                .shared
                                .graph
                                .path_names
                                .get_by_left(path)
                                .map(|n| n.as_str())
                                .unwrap_or("ERROR");
                            ui.label(format!("Node {}", node.ix()));
                            ui.label(format!("Path {path_name}"));
                            ui.label(format!("Pos {} bp", pos.0));
                        },
                    );
                }
            }

            let painter =
                egui_ctx.ctx().layer_painter(egui::LayerId::background());
            painter.extend(shapes);

            let painter = egui_ctx.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                "main_area_fg".into(),
            ));

            painter.extend(self.slot_cache.msg_shapes.drain(..));
        }

        for (slot_id, rect) in annot_slots {
            if let Some(annot_slot) = self.annotations.get_mut(&slot_id) {
                annot_slot.update(tokio_rt, rect, &self.view, dt);
            }
        }

        egui_ctx.end_frame(&window.window);
    }

    fn on_event(
        &mut self,
        _window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        let consume = false;

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
        let data_id = self.active_viz_data_key.blocking_read().clone();
        let viz_mode_color = self
            .viz_mode_config
            .get(&data_id)
            .unwrap_or_else(|| panic!("Config not found for data {data_id}"));

        self.color_mapping.update_data(|cmap| {
            cmap.store(viz_mode_color.default_color_map);
        });
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

            let sampler = if self.use_linear_sampler.load() {
                colors.linear_sampler.clone()
            } else {
                colors.nearest_sampler.clone()
            };

            let data_key = self.active_viz_data_key.blocking_read().clone();

            let id = self
                .shared
                .data_color_schemes
                .blocking_read()
                .get(&data_key)
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

        if let Some(transforms) = self.slot_cache.transform_buffer.as_ref() {
            transient_res.insert(
                "transform".into(),
                InputResource::Buffer {
                    size: 2 * 4,
                    stride: None,
                    buffer: &transforms.buffer,
                },
            );
        }

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
