use anyhow::Result;
use lyon::lyon_tessellation::{
    BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexBuffers,
};
use lyon::math::{point, Point};
use lyon::path::{EndpointId, PathCommands};
use std::collections::HashMap;
use std::io::{prelude::*, BufReader};
use ultraviolet::Vec2;
use wgpu::util::DeviceExt;

use waragraph_core::graph::{Node, PathIndex};

pub struct NodePositions {
    pub bounds: (Vec2, Vec2),
    positions: Vec<Vec2>,
}

impl NodePositions {
    pub fn iter_nodes<'a>(&'a self) -> impl Iterator<Item = [Vec2; 2]> + 'a {
        self.positions.chunks_exact(2).map(|w| {
            if let [start, end] = w {
                [*start, *end]
            } else {
                unreachable!();
            }
        })
    }

    pub fn node_pos(&self, node: Node) -> (Vec2, Vec2) {
        let ix = node.ix();
        let ix0 = ix * 2;
        let ix1 = ix0 + 1;
        (self.positions[ix0], self.positions[ix1])
    }

    pub fn from_layout_tsv(
        // path_index: &PathIndex,
        tsv_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        use std::fs::File;
        // use std::io::{prelude::*, BufReader};
        let mut lines = File::open(tsv_path).map(BufReader::new)?.lines();

        let _header = lines.next();
        let mut positions = Vec::new();

        fn parse_row(line: &str) -> Option<(usize, Vec2)> {
            let mut fields = line.split('\t');
            let idx = fields.next()?.parse::<usize>().ok()?;
            let x = fields.next()?.parse::<f32>().ok()?;
            let y = fields.next()?.parse::<f32>().ok()?;
            Some((idx, Vec2::new(x, y)))
        }

        let mut min = Vec2::broadcast(f32::MAX);
        let mut max = Vec2::broadcast(f32::MIN);

        for line in lines {
            let line = line?;
            if let Some((i, v)) = parse_row(&line) {
                min = min.min_by_component(v);
                max = max.max_by_component(v);
                positions.push((i, v));
            }
        }
        let bounds = (min, max);

        positions.sort_by_key(|(i, _)| *i);
        let positions =
            positions.into_iter().map(|(_, p)| p).collect::<Vec<_>>();

        Ok(Self { positions, bounds })
    }
}
