use bimap::BiHashMap;
use raving::compositor::SublayerDrawData;

use rustc_hash::{FxHashMap, FxHashSet};

use anyhow::{anyhow, Result};

use std::sync::Arc;

use crate::graph::{Node, Path, Strand, Waragraph};

use super::ViewDiscrete1D;

pub struct EdgeCache {
    // all edges as endpoints (pangenome positions)
    edge_endpoints: Vec<(usize, usize)>,

    // bitmap is a set of indices into the edge_endpoints
    path_edges: FxHashMap<Path, roaring::RoaringBitmap>,
}

impl EdgeCache {
    pub fn new(graph: &Waragraph) -> Self {
        let mut seen_edges: BiHashMap<(usize, usize), usize> =
            BiHashMap::default();

        let mut all_path_edges: FxHashMap<Path, roaring::RoaringBitmap> =
            FxHashMap::default();

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

                if diff > min_dist && !seen_edges.contains_left(&(p_a, p_b)) {
                    let id = seen_edges.len();
                    seen_edges.insert((p_a, p_b), id);
                    path_edges.insert(id as u32);
                }

                prev_step = step;
            }

            all_path_edges.insert(*path, path_edges);
        }

        let mut edges = seen_edges.into_iter().collect::<Vec<_>>();
        edges.sort_by_key(|(_, i)| *i);
        let edge_endpoints =
            edges.into_iter().map(|(e, _)| e).collect::<Vec<_>>();

        Self {
            edge_endpoints,
            path_edges: all_path_edges,
        }
    }
}

pub fn edge_vertices(
    // still grabbing the layout from here
    graph: &Arc<Waragraph>,
    // path: Path,
    path_name: &str,
    view: ViewDiscrete1D,
    slot_x_offset: f32,
    slot_y_offset: f32,
    slot_width: f32,
    slot_height: f32,
    // should be a "line-rgb"-type sublayer
    sublayer_data: &mut SublayerDrawData,
) -> Result<()> {
    dbg!();
    let path = graph.path_index(path_name).unwrap();

    let mut edge_endpoints: FxHashSet<(usize, usize)> = FxHashSet::default();
    dbg!();

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

        let (p_a, p_b) = match (prev_step.is_reverse(), step.is_reverse()) {
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
            edge_endpoints.insert((p_a, p_b));
        }

        prev_step = step;
    }

    log::warn!("found {} edges", edge_endpoints.len());

    let edge_endpoints: Vec<(usize, usize)> =
        edge_endpoints.into_iter().collect();

    let screen_x = |pos: usize| {
        view.screen_x(slot_x_offset as f64, slot_width as f64, pos)
    };

    // now we have a set of (bp, bp) pairs that we can map to screen X
    // coordinates, so we need to compute the Y coordinate for each edge

    let edge_y = |p0: usize, p1: usize| {
        let diff = (p0 as isize).abs_diff(p1 as isize) as f64;
        let t = diff / max_dist as f64;
        let h = slot_height as f64;
        (t * h)
    };

    // let edge_y = |x0: f32, x1: f32| {
    //     let diff = (x0 - x1).abs();
    //     diff
    // };

    /*
    let mut vertices: Vec<[u8; 40]> = Vec::new();

    let mut vx0 = [0u8; 40];
    let mut vx1 = [0u8; 40];
    let mut vx2 = [0u8; 40];

    let x0 = 100f32;
    let x1 = 300f32;
    let y0 = 400f32;
    let y1 = 600f32;

    let color = [1f32, 0.0, 0.0, 1.0];

    vx0[0..12].clone_from_slice(bytemuck::cast_slice(&[x0, y0, 1.0]));
    vx0[12..24].clone_from_slice(bytemuck::cast_slice(&[x0, y1, 1.0]));
    vx0[24..40].clone_from_slice(bytemuck::cast_slice(&color));

    vx1[0..12].clone_from_slice(bytemuck::cast_slice(&[x0, y1, 1.0]));
    vx1[12..24].clone_from_slice(bytemuck::cast_slice(&[x1, y1, 1.0]));
    vx1[24..40].clone_from_slice(bytemuck::cast_slice(&color));

    vx2[0..12].clone_from_slice(bytemuck::cast_slice(&[x1, y1, 1.0]));
    vx2[12..24].clone_from_slice(bytemuck::cast_slice(&[x1, y0, 1.0]));
    vx2[24..40].clone_from_slice(bytemuck::cast_slice(&color));

    vertices.push(vx0);
    vertices.push(vx1);
    vertices.push(vx2);

    sublayer_data.update_vertices_array(vertices)?;
    */

    let mut too_small = 0;

    sublayer_data.update_vertices_array(
        edge_endpoints
            .iter()
            // .take(4)
            .copied()
            .flat_map(|(p_a, p_b)| {
                let mut vx0 = [0u8; 40];
                let mut vx1 = [0u8; 40];
                let mut vx2 = [0u8; 40];

                let x0 = screen_x(p_a);
                let x1 = screen_x(p_b);
                let y0 = slot_y_offset as f64;

                let mut yd = edge_y(p_a, p_b);
                // let mut yd = edge_y(x0, x1);

                if yd < 1.0 {
                    yd = 1.0;
                    too_small += 1;
                }

                let y1 = y0 + yd;

                log::warn!(
                    "p_a: {}, p_b: {}\tx0: {}, x1: {}\ty0: {}, y1: {}",
                    p_a,
                    p_b,
                    x0,
                    x1,
                    y0,
                    y1
                );

                let x0 = x0 as f32;
                let x1 = x1 as f32;
                let y0 = y0 as f32;
                let y1 = y1 as f32;

                let color = [1f32, 0.0, 0.0, 1.0];

                vx0[0..12]
                    .clone_from_slice(bytemuck::cast_slice(&[x0, y0, 1.0]));
                vx0[12..24]
                    .clone_from_slice(bytemuck::cast_slice(&[x0, y1, 1.0]));
                vx0[24..40].clone_from_slice(bytemuck::cast_slice(&color));

                vx1[0..12]
                    .clone_from_slice(bytemuck::cast_slice(&[x0, y1, 1.0]));
                vx1[12..24]
                    .clone_from_slice(bytemuck::cast_slice(&[x1, y1, 1.0]));
                vx1[24..40].clone_from_slice(bytemuck::cast_slice(&color));

                vx2[0..12]
                    .clone_from_slice(bytemuck::cast_slice(&[x1, y1, 1.0]));
                vx2[12..24]
                    .clone_from_slice(bytemuck::cast_slice(&[x1, y0, 1.0]));
                vx2[24..40].clone_from_slice(bytemuck::cast_slice(&color));

                [vx0, vx1, vx2].into_iter()
            }),
    )?;

    log::warn!("{} edges were tiny", too_small);

    log::warn!("max dist (bp): {}", max_dist);

    Ok(())
}
