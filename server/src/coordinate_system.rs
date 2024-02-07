use std::sync::Arc;

use arrow2::array::{
    Array, BinaryArray, Float32Array, Int32Array, StructArray, UInt32Array,
    Utf8Array,
};
use arrow2::chunk::Chunk;
use arrow2::datatypes::DataType;
use arrow2::io::ipc::write::{FileWriter, WriteOptions};
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{get, post, State};
use waragraph_core::arrow_graph::ArrowGFA;
use waragraph_core::coordinate_system::CoordSys;
use waragraph_core::graph_layout::GraphLayout;

#[get("/global/segment_at_offset?<pos_bp>")]
pub async fn global_segment_at_offset(
    coord_sys_cache: &State<crate::CoordSysCache>,
    pos_bp: u64,
) -> Option<Json<u32>> {
    let cs_map = coord_sys_cache.map.read().await;
    let global_cs = cs_map.get("<global>")?;
    let seg = global_cs.segment_at_pos(pos_bp);
    Some(Json(seg))
}

#[get("/global/segment_range/<segment>")]
pub async fn global_segment_range(
    coord_sys_cache: &State<crate::CoordSysCache>,
    segment: u32,
) -> Option<Vec<u8>> {
    let cs_map = coord_sys_cache.map.read().await;
    let global_cs = cs_map.get("<global>")?;
    let range = global_cs.segment_range(segment)?;
    let mut result: Vec<u8> = Vec::with_capacity(64 * 2);
    result.extend_from_slice(bytemuck::cast_slice(&[range.start, range.end]));
    Some(result)
}

#[get("/global")]
pub async fn global(coord_sys_cache: &State<crate::CoordSysCache>) -> Vec<u8> {
    let cs_map = coord_sys_cache.map.read().await;

    // NB this is generated before the server launches, for now
    let cs = cs_map.get("<global>").unwrap();

    let schema = CoordSys::arrow_schema();

    let mut buf: Vec<u8> = vec![];

    let mut writer = FileWriter::new(
        std::io::Cursor::new(&mut buf),
        schema,
        None,
        WriteOptions { compression: None },
    );

    let offsets = cs.step_offsets.buffer().clone();
    let offsets = Int32Array::new(DataType::Int32, offsets, None);

    let olen = offsets.len();
    let offsets = offsets.sliced(1, olen - 1);

    let chunk =
        Chunk::new(vec![cs.node_order.clone().boxed(), offsets.boxed()]);

    writer.start().unwrap();
    writer.write(&chunk, None).unwrap();
    writer.finish().unwrap();

    buf
}

// #[get("/path?<path_id>")]
// pub async fn path_id(
//     coord_sys_cache: &State<crate::CoordSysCache>,
//     //
// ) -> Vec<u8> {
//     todo!();
// }

pub async fn path_interval_to_global_blocks_impl(
    graph: &Arc<ArrowGFA>,
    path_cs: Arc<CoordSys>,
    path_id: u32,
    start_bp: u64,
    end_bp: u64,
) -> Vec<u8> {
    let step_range = path_cs.bp_to_step_range(start_bp, end_bp);

    let steps = graph.path_steps(path_id);
    let len = *step_range.end() - *step_range.start();
    let step_slice = steps.clone().sliced(*step_range.start(), len);

    let sorted = step_slice.values_iter().copied().collect::<Vec<_>>();

    let mut ranges = Vec::new();

    let mut range_start = sorted[0] >> 1;
    let mut prev_seg_ix = sorted[0] >> 1;

    for handle in sorted {
        let seg_ix = handle >> 1;

        if seg_ix.abs_diff(prev_seg_ix) > 2 {
            ranges.push([range_start, prev_seg_ix]);
            range_start = seg_ix;
        }

        prev_seg_ix = seg_ix;
    }

    if range_start != prev_seg_ix {
        ranges.push([range_start, prev_seg_ix]);
    }

    let mut out: Vec<u8> = Vec::with_capacity(ranges.len() * 8);

    for range in ranges {
        out.extend_from_slice(bytemuck::cast_slice(&range));
    }

    out
}

#[get("/path_interval_to_global_blocks?<path_id>&<start_bp>&<end_bp>")]
pub async fn path_interval_to_global_blocks(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<crate::CoordSysCache>,
    path_id: u32,
    start_bp: u64,
    end_bp: u64,
) -> Vec<u8> {
    let path_cs = coord_sys_cache
        .get_or_compute_for_path(graph, path_id)
        .await
        .unwrap();

    path_interval_to_global_blocks_impl(
        graph, path_cs, path_id, start_bp, end_bp,
    )
    .await
}

// TODO the return type of this and so many other endpoints
#[get("/path/segment_at_offset?<path_id>&<pos_bp>")]
pub async fn get_segment_at_path_position(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<crate::CoordSysCache>,
    path_id: u32,
    pos_bp: u64,
) -> Json<u32> {
    let path_cs = coord_sys_cache
        .get_or_compute_for_path(graph, path_id)
        .await
        .unwrap();
    let seg = path_cs.segment_at_pos(pos_bp);
    Json(seg)
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AnnotationRange {
    path_id: u32,

    start_bp: u64,
    end_bp: u64,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PreparedAnnotation {
    first_step: u32,
    last_step: u32,

    start_world_x: f32,
    start_world_y: f32,
    end_world_x: f32,
    end_world_y: f32,

    blocks_1d_bp: Vec<u64>,
}

// #[post("/prepare_annotation_record/<coord_sys>", data = "<record>")]
#[post("/prepare_annotation_record", data = "<records>")]
pub async fn prepare_annotation_record(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<crate::CoordSysCache>,
    graph_layout: &State<GraphLayout>,
    records: Json<Vec<AnnotationRange>>,
    // coord_sys: String,
) -> Option<Json<Vec<PreparedAnnotation>>> {
    let cs = {
        let cs_map = coord_sys_cache.map.read().await;
        cs_map.get("<global>")?.clone()
    };

    let mut result = Vec::new();

    for record in records.0 {
        let path_steps = graph.path_steps(record.path_id);

        // TODO should probably not fail the entire request here
        let step_range = cs.bp_to_step_range(record.start_bp, record.end_bp);
        let first_step = path_steps.get(*step_range.start())?;
        let last_step = path_steps.get(*step_range.end())?;

        let [x0, y0, ..] = graph_layout.segment_position(first_step)?;
        let [.., x1, y1] = graph_layout.segment_position(last_step)?;

        let path_cs = coord_sys_cache
            .get_or_compute_for_path(graph, record.path_id)
            .await?;

        let blocks_1d_bp = path_interval_to_global_blocks_impl(
            graph, path_cs, path_id, start_bp, end_bp,
        )
        .await;

        result.push(PreparedAnnotation {
            first_step,
            last_step,
            start_world_x: x0,
            start_world_y: y0,
            end_world_x: x1,
            end_world_y: y1,
            blocks_1d_bp,
        });
    }

    Some(Json(result))
}
