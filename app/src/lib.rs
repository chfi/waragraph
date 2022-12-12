use anyhow::Result;
use roaring::RoaringTreemap;
use ultraviolet::Vec2;
use std::collections::BTreeMap;
use std::io::prelude::*;
use std::io::BufReader;

pub mod viewer_1d;
pub mod viewer_2d;

pub mod annotations;

pub mod gpu_cache;


pub struct GfaLayout {
    pub positions: Vec<Vec2>,
}

impl GfaLayout {
    pub fn pos_for_node(&self, node: usize) -> Option<(Vec2, Vec2)> {
        let ix = node / 2;
        let a = *self.positions.get(ix)?;
        let b = *self.positions.get(ix + 1)?;
        Some((a, b))
    }

    pub fn from_layout_tsv(tsv_path: impl AsRef<std::path::Path>) -> Result<Self> {
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

        Ok(GfaLayout { positions })
    }
}
