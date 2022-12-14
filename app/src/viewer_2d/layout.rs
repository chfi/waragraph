use anyhow::Result;
use lyon::lyon_tessellation::BuffersBuilder;
use lyon::lyon_tessellation::StrokeOptions;
use lyon::lyon_tessellation::StrokeTessellator;
use lyon::lyon_tessellation::StrokeVertex;
use lyon::lyon_tessellation::VertexBuffers;
use lyon::math::point;
use lyon::math::Point;
use lyon::path::EndpointId;
use lyon::path::PathCommands;
use wgpu::util::DeviceExt;
use std::io::prelude::*;
use std::io::BufReader;
use ultraviolet::Vec2;

use waragraph_core::graph::PathIndex;

pub struct GraphPaths {
    pub aabb: (Vec2, Vec2),
    endpoints: Vec<Point>,
    gfa_paths: Vec<PathCommands>,
}

impl GraphPaths {

    pub(super) fn tessellate_paths(
        &self,
        device: &wgpu::Device,
        path_ids: impl IntoIterator<Item = usize>,
    ) -> Result<super::LyonBuffers> {
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

        for path in path_ids {
            let path = &self.gfa_paths[path];
            let slice: lyon::path::commands::CommandsPathSlice<
                lyon::geom::euclid::Point2D<
                    f32,
                    lyon::geom::euclid::UnknownUnit,
                >,
                lyon::geom::euclid::Point2D<
                    f32,
                    lyon::geom::euclid::UnknownUnit,
                >,
            > = path.path_slice(&self.endpoints, &self.endpoints);

            stroke_tess.tessellate_with_ids(
                path.iter(),
                &slice,
                None,
                &opts,
                &mut buf_build,
            )?;
        }
        
        let vertices = geometry.vertices.len();
        let indices = geometry.indices.len();

        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&geometry.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );

        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&geometry.indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        );

        Ok(super::LyonBuffers {
            vertices,
            indices,
            vertex_buffer,
            index_buffer,
        })
    }

    // pub fn pos_for_node(&self, node: usize) -> Option<(Vec2, Vec2)> {
    //     let ix = node / 2;
    //     let a = *self.endpoints.get(ix)?;
    //     let b = *self.endpoints.get(ix + 1)?;
    //     Some((a, b))
    // }

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

        for (ix, steps) in path_index.path_steps.iter().enumerate() {
            let mut builder = PathCommands::builder();

            let mut started = false;

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

        let endpoints = positions.into_iter().map(|p| point(p.x, p.y)).collect();

        Ok(GraphPaths {
            aabb,
            endpoints,
            gfa_paths,
        })
    }
}
