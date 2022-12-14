use anyhow::Result;
use lyon::path::EndpointId;
use lyon::path::PathCommands;
use std::io::prelude::*;
use std::io::BufReader;
use ultraviolet::Vec2;

use waragraph_core::graph::PathIndex;

pub struct GraphPaths {
    endpoints: Vec<Vec2>,
    gfa_paths: Vec<PathCommands>,
}

impl GraphPaths {
    pub fn pos_for_node(&self, node: usize) -> Option<(Vec2, Vec2)> {
        let ix = node / 2;
        let a = *self.endpoints.get(ix)?;
        let b = *self.endpoints.get(ix + 1)?;
        Some((a, b))
    }

    pub fn from_path_index_and_layout_tsv(
        path_index: &PathIndex,
        tsv_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        use std::fs::File;
        // use std::io::{prelude::*, BufReader};
        let mut lines = File::open(tsv_path).map(BufReader::new)?.lines();

        let _header = lines.next();
        let mut positions = Vec::new();

        fn parse_row(line: &str) -> Option<Vec2> {
            let mut fields = line.split('\t');
            let _idx = fields.next();
            let x = fields.next()?.parse::<f32>().ok()?;
            let y = fields.next()?.parse::<f32>().ok()?;
            Some(Vec2::new(x, y))
        }

        for line in lines {
            let line = line?;
            if let Some(v) = parse_row(&line) {
                positions.push(v);
            }
        }

        let mut gfa_paths = Vec::with_capacity(path_index.path_names.len());

        for steps in path_index.path_steps.iter() {
            let mut builder = PathCommands::builder();

            for (ix, &step) in steps.iter().enumerate() {
                let seg = step.node();
                let rev = step.is_reverse();
                let ix = seg.ix();
                let a = ix * 2;
                let b = a + 1;
                let mut pts = [a as u32, b as u32];
                if rev {
                    pts.reverse();
                }

                if ix == 0 {
                    builder.begin(EndpointId(pts[0]));
                }
                pts.into_iter().for_each(|b| {
                    builder.line_to(EndpointId(b));
                });
            }
            builder.end(false);

            gfa_paths.push(builder.build());
        }

        let endpoints = positions;

        Ok(GraphPaths {
            endpoints,
            gfa_paths,
        })
    }
}
