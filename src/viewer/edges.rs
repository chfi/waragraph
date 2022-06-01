use raving::compositor::SublayerDrawData;

use rustc_hash::FxHashSet;

use anyhow::{anyhow, Result};

use std::sync::Arc;

use crate::graph::{Node, Path, Strand, Waragraph};

use super::ViewDiscrete1D;

pub fn edge_vertices(
    // still grabbing the layout from here
    graph: &Arc<Waragraph>,
    path: Path,
    view: ViewDiscrete1D,
    slot_x_offset: f32,
    slot_y_offset: f32,
    slot_width: f32,
    slot_height: f32,
    // should be a "line-rgb"-type sublayer
    sublayer_data: &mut SublayerDrawData,
) -> Result<()> {
    let mut edge_endpoints: FxHashSet<(usize, usize)> = FxHashSet::default();

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

        if diff < min_dist {
            edge_endpoints.insert((p_a, p_b));
        }

        prev_step = step;
    }

    let edge_endpoints: Vec<(usize, usize)> =
        edge_endpoints.into_iter().collect();

    let screen_x = |pos: usize| view.screen_x(slot_x_offset, slot_width, pos);

    // now we have a set of (bp, bp) pairs that we can map to screen X
    // coordinates, so we need to compute the Y coordinate for each edge

    let edge_y = |p0: usize, p1: usize| {
        let diff = (p0 as isize).abs_diff(p1 as isize) as f32;
        let t = diff / max_dist as f32;
        t * slot_height
    };

    sublayer_data.update_vertices_array(
        edge_endpoints.iter().copied().flat_map(|(p_a, p_b)| {
            let mut vx0 = [0u8; 40];
            let mut vx1 = [0u8; 40];
            let mut vx2 = [0u8; 40];

            let x0 = screen_x(p_a);
            let x1 = screen_x(p_b);
            let y0 = slot_y_offset;
            let y1 = y0 + edge_y(p_a, p_b);

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

            [vx0, vx1, vx2].into_iter()
        }),
    )?;

    Ok(())
}
