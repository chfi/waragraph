use bimap::BiHashMap;
use euclid::Point2D;
use raving::compositor::SublayerDrawData;

use rustc_hash::FxHashMap;

use anyhow::Result;

use crate::{
    geometry::{view::PangenomeView, ScreenPoint},
    graph::{Path, Strand, Waragraph},
};

use super::gui::layer::line_width_rgba;

pub struct EdgeCache {
    // all edges as endpoints (pangenome positions)
    edge_endpoints: Vec<(usize, usize)>,

    // bitmap is a set of indices into the edge_endpoints
    path_edges: FxHashMap<Path, roaring::RoaringBitmap>,

    max_edge_length: FxHashMap<Path, usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ParamCache {
    view: PangenomeView,

    // u32 instead of f32 here since this will compared for equality
    slot_offset: [u32; 2],
    slot_dims: [u32; 2],

    path: Path,
}

pub struct EdgeVertexCache {
    inputs: Option<ParamCache>,

    pub edge_cache: EdgeCache,
}

impl EdgeVertexCache {
    pub fn new(edge_cache: EdgeCache) -> Self {
        Self {
            inputs: None,
            edge_cache,
        }
    }

    pub fn update_sublayer_data_with_path(
        &mut self,
        graph: &Waragraph,
        path: Path,
        view: PangenomeView,
        slot_x_offset: u32,
        slot_y_offset: u32,
        slot_width: u32,
        slot_height: u32,
        // should be a "line-rgb"-type sublayer
        sublayer_data: &mut SublayerDrawData,
    ) -> Result<()> {
        let slot_offset = [slot_x_offset, slot_y_offset];
        let slot_dims = [slot_width, slot_height];

        let new_params = ParamCache {
            view,
            slot_offset,
            slot_dims,
            path,
        };

        if let Some(old_params) = self.inputs {
            if old_params == new_params {
                return Ok(());
            }
        }

        self.inputs = Some(new_params);

        self.edge_cache.update_sublayer_data_with_path(
            graph,
            path,
            view,
            slot_x_offset as f32,
            slot_y_offset as f32,
            slot_width as f32,
            slot_height as f32,
            sublayer_data,
        )?;

        Ok(())
    }
}

impl EdgeCache {
    pub fn new(graph: &Waragraph) -> Self {
        let mut seen_edges: BiHashMap<(usize, usize), usize> =
            BiHashMap::default();

        let mut all_path_edges: FxHashMap<Path, roaring::RoaringBitmap> =
            FxHashMap::default();

        let mut max_edge_length = FxHashMap::default();

        for path in graph.path_names.left_values() {
            let mut path_edges = roaring::RoaringBitmap::new();

            let mut steps = graph.path_steps[path.ix()].iter().copied();

            let mut prev_step: Strand = steps.next().unwrap();

            let mut max_dist = std::usize::MIN;

            // map the edge to a pair of node endpoints, depending on
            // orientation, resulting in a (bp, bp) pair
            // walk path, for each step, use the two strands as an edge
            for step in steps {
                let offset_a = graph.node_pos(prev_step.node());
                let offset_b = graph.node_pos(step.node());

                let len_a = graph.node_lens[prev_step.node().ix()] as usize;
                let len_b = graph.node_lens[step.node().ix()] as usize;

                let (p_a, p_b) =
                    match (prev_step.is_reverse(), step.is_reverse()) {
                        (false, false) => {
                            // (a+, b+) := -a+ -> -b+
                            // use RHS of prev_step, LHS of step
                            (offset_a + len_a, offset_b)
                        }
                        (false, true) => {
                            // (a+, b-) := -a+ -> +b-
                            // use RHS of prev_step, RHS of step
                            (offset_a + len_a, offset_b + len_b)
                        }
                        (true, false) => {
                            // (a-, b+) := +a- -> -b+
                            // use LHS of prev_step, LHS of step
                            (offset_a, offset_b)
                        }
                        (true, true) => {
                            // (a-, b-) := +a- -> +b-
                            // use LHS of prev_step, RHS of step
                            (offset_a, offset_b + len_b)
                        }
                    };

                let diff = (p_a as isize).abs_diff(p_b as isize);

                let min_dist = 1;

                max_dist = max_dist.max(diff);

                if diff > min_dist {
                    let key = (p_a, p_b);

                    let id = if let Some(id) = seen_edges.get_by_left(&key) {
                        *id
                    } else {
                        let id = seen_edges.len();
                        seen_edges.insert((p_a, p_b), id);
                        id
                    };

                    path_edges.insert(id as u32);
                }

                prev_step = step;
            }

            all_path_edges.insert(*path, path_edges);
            max_edge_length.insert(*path, max_dist);
        }

        let mut edges = seen_edges.into_iter().collect::<Vec<_>>();
        edges.sort_by_key(|(_, i)| *i);
        let edge_endpoints =
            edges.into_iter().map(|(e, _)| e).collect::<Vec<_>>();

        Self {
            edge_endpoints,
            path_edges: all_path_edges,
            max_edge_length,
        }
    }

    pub fn update_sublayer_data_with_path(
        &self,
        graph: &Waragraph,
        path: Path,
        view: PangenomeView,
        slot_x_offset: f32,
        slot_y_offset: f32,
        slot_width: f32,
        slot_height: f32,
        // should be a "line-rgb"-type sublayer
        sublayer_data: &mut SublayerDrawData,
    ) -> Result<()> {
        let screen_x = |pos: usize| {
            let x = slot_x_offset as f64;
            let width = slot_width as f64;
            let fact = width / (view.len().0 as f64);
            x + (pos as f64 - view.offset().0 as f64) * fact
        };

        let max_dist = *self.max_edge_length.get(&path).unwrap();

        let edge_y = |p0: usize, p1: usize| {
            let diff = (p0 as isize).abs_diff(p1 as isize) as f64;
            let t = diff / max_dist as f64;
            let h = slot_height as f64;
            t * h
        };

        let edge_ids = self.path_edges.get(&path).unwrap();

        sublayer_data.update_vertices_array(
            edge_ids
                .iter()
                .map(|i| self.edge_endpoints[i as usize])
                .flat_map(|(p_a, p_b)| {
                    let x0 = screen_x(p_a);
                    let x1 = screen_x(p_b);
                    let y0 = slot_y_offset as f64;

                    let mut yd = edge_y(p_a, p_b);

                    if yd < 1.0 {
                        yd = 1.0;
                    }

                    let y1 = y0 + yd;

                    let x0 = x0 as f32;
                    let x1 = x1 as f32;
                    let y0 = y0 as f32;
                    let y1 = y1 as f32;

                    // let w = 0.5f32;
                    let w = 1f32;

                    let color = rgb::RGBA::new(0f32, 0.0, 0.0, 1.0);

                    let p0: ScreenPoint = Point2D::new(x0, y0);
                    let p1: ScreenPoint = Point2D::new(x0, y1);
                    let p2: ScreenPoint = Point2D::new(x1, y1);
                    let p3: ScreenPoint = Point2D::new(x1, y0);

                    let vx0 = line_width_rgba(p0, p1, w, w, color);
                    let vx1 = line_width_rgba(p1, p2, w, w, color);
                    let vx2 = line_width_rgba(p2, p3, w, w, color);

                    [vx0, vx1, vx2].into_iter()
                }),
        )?;

        Ok(())
    }
}
