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

use waragraph_core::graph::PathIndex;

pub struct GraphPathCurves {
    pub aabb: (Vec2, Vec2),
    endpoints: Vec<Point>,
    gfa_paths: Vec<PathCommands>,
}

pub struct PathCurveBuffers {
    pub(super) total_vertices: usize,
    pub(super) total_indices: usize,
    pub(super) vertex_buffer: wgpu::Buffer,
    pub(super) index_buffer: wgpu::Buffer,

    pub(super) path_indices: HashMap<usize, std::ops::Range<usize>>,
}

impl GraphPathCurves {
    pub(super) fn tessellate_paths(
        &self,
        device: &wgpu::Device,
        path_ids: impl IntoIterator<Item = usize>,
    ) -> Result<PathCurveBuffers> {
        let mut geometry: VertexBuffers<super::GpuVertex, u32> =
            VertexBuffers::new();
        let tolerance = 10.0;

        let opts = StrokeOptions::tolerance(tolerance).with_line_width(150.0);

        let mut stroke_tess = StrokeTessellator::new();

        let mut buf_build =
            BuffersBuilder::new(&mut geometry, |vx: StrokeVertex| {
                super::GpuVertex {
                    pos: vx.position().to_array(),
                }
            });

        let mut path_indices = HashMap::default();

        for path_id in path_ids {
            let path = &self.gfa_paths[path_id];
            let slice = path.path_slice(&self.endpoints, &self.endpoints);

            let ixs_start = buf_build.buffers().indices.len();

            stroke_tess.tessellate_with_ids(
                path.iter(),
                &slice,
                None,
                &opts,
                &mut buf_build,
            )?;

            let ixs_end = buf_build.buffers().indices.len();

            path_indices.insert(path_id, ixs_start..ixs_end);
        }

        let vertices = geometry.vertices.len();
        let indices = geometry.indices.len();

        let vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&geometry.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&geometry.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        Ok(PathCurveBuffers {
            total_vertices: vertices,
            total_indices: indices,

            vertex_buffer,
            index_buffer,

            path_indices,
        })
    }

    pub fn pos_for_node(&self, node: usize) -> Option<(Vec2, Vec2)> {
        let ix = node / 2;
        let a = *self.endpoints.get(ix)?;
        let b = *self.endpoints.get(ix + 1)?;
        Some((a.to_array().into(), b.to_array().into()))
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

        let mut min = Vec2::broadcast(f32::MAX);
        let mut max = Vec2::broadcast(f32::MIN);

        for line in lines {
            let line = line?;
            if let Some(v) = parse_row(&line) {
                min = min.min_by_component(v);
                max = max.max_by_component(v);
                positions.push(v);
            }
        }
        let aabb = (min, max);

        let mut gfa_paths = Vec::with_capacity(path_index.path_names.len());

        for steps in path_index.path_steps.iter() {
            let mut builder = PathCommands::builder();

            let mut started = false;

            for &step in steps.iter() {
                let seg = step.node();
                let rev = step.is_reverse();
                let ix = seg.ix();
                let a = ix * 2;
                let b = a + 1;
                let mut pts = [a as u32, b as u32];
                if rev {
                    pts.reverse();
                }

                if !started {
                    builder.begin(EndpointId(pts[0]));
                    started = true;
                }
                pts.into_iter().for_each(|b| {
                    builder.line_to(EndpointId(b));
                });
            }
            builder.end(false);

            gfa_paths.push(builder.build());
        }

        let endpoints =
            positions.into_iter().map(|p| point(p.x, p.y)).collect();

        Ok(GraphPathCurves {
            aabb,
            endpoints,
            gfa_paths,
        })
    }
}
