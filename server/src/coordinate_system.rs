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
    global_cs: Arc<CoordSys>,
    path_cs: Arc<CoordSys>,
    path_id: u32,
    start_bp: u64,
    end_bp: u64,
) -> Vec<[u64; 2]> {
    let step_range = path_cs.bp_to_step_range(start_bp, end_bp);

    let steps = graph.path_steps(path_id);
    let len = *step_range.end() - *step_range.start();
    let step_slice = steps.clone().sliced(*step_range.start(), len);

    let sorted = step_slice.values_iter().copied().collect::<Vec<_>>();

    let mut segment_ranges = Vec::new();

    let mut range_start = sorted[0] >> 1;
    let mut prev_seg_ix = sorted[0] >> 1;

    for handle in sorted {
        let seg_ix = handle >> 1;

        if seg_ix.abs_diff(prev_seg_ix) > 2 {
            segment_ranges.push([range_start, prev_seg_ix]);
            range_start = seg_ix;
        }

        prev_seg_ix = seg_ix;
    }

    if range_start != prev_seg_ix {
        segment_ranges.push([range_start, prev_seg_ix]);
    }

    let ranges = segment_ranges
        .into_iter()
        .filter_map(|[start, end]| {
            let start_bp = global_cs.segment_range(start)?.start;
            let end_bp = global_cs.segment_range(end)?.end;
            Some([start_bp, end_bp])
        })
        .collect::<Vec<_>>();

    ranges
}

#[get("/path_interval_to_global_blocks?<path_id>&<start_bp>&<end_bp>")]
pub async fn path_interval_to_global_blocks(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<crate::CoordSysCache>,
    path_id: u32,
    start_bp: u64,
    end_bp: u64,
) -> Vec<u8> {
    let global_cs = {
        let cs_map = coord_sys_cache.map.read().await;
        cs_map.get("<global>").unwrap().clone()
    };

    let path_cs = coord_sys_cache
        .get_or_compute_for_path(graph, path_id)
        .await
        .unwrap();

    let ranges = path_interval_to_global_blocks_impl(
        graph, global_cs, path_cs, path_id, start_bp, end_bp,
    )
    .await;

    let mut out: Vec<u8> = Vec::with_capacity(ranges.len() * 8);

    for range in ranges {
        out.extend_from_slice(bytemuck::cast_slice(&range));
    }

    out
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

    start_bp: u64,
    end_bp: u64,

    start_world_x: f32,
    start_world_y: f32,
    end_world_x: f32,
    end_world_y: f32,

    path_steps: Vec<u32>,
    blocks_1d_bp: Vec<[u64; 2]>,
}

// #[post("/prepare_annotation_record/<coord_sys>", data = "<record>")]
#[post("/prepare_annotation_records", data = "<records>")]
pub async fn prepare_annotation_records(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<crate::CoordSysCache>,
    graph_layout: &State<GraphLayout>,
    records: Json<Vec<AnnotationRange>>,
    // coord_sys: String,
) -> Option<Json<Vec<PreparedAnnotation>>> {
    let global_cs = {
        let cs_map = coord_sys_cache.map.read().await;
        cs_map.get("<global>")?.clone()
    };

    let mut result = Vec::new();

    for record in records.0 {
        let path_steps = graph.path_steps(record.path_id);

        let path_cs = coord_sys_cache
            .get_or_compute_for_path(graph, record.path_id)
            .await?;

        // TODO should probably not fail the entire request here
        let step_range =
            path_cs.bp_to_step_range(record.start_bp, record.end_bp);
        let first_step = path_steps.get(*step_range.start())?;
        let last_step = path_steps.get(*step_range.end())?;

        let path_steps = path_steps.clone().sliced(
            *step_range.start(),
            *step_range.end() - *step_range.start() + 1,
        );

        let path_steps = path_steps.values().to_vec();

        // TODO take orientation into account
        let [x0, y0, ..] = graph_layout.segment_position(first_step >> 1)?;
        let [.., x1, y1] = graph_layout.segment_position(last_step >> 1)?;

        let blocks_1d_bp = path_interval_to_global_blocks_impl(
            graph,
            global_cs.clone(),
            path_cs,
            record.path_id,
            record.start_bp,
            record.end_bp,
        )
        .await;

        // TODO orientation here too
        let first_range = global_cs.segment_range(first_step >> 1)?;
        let last_range = global_cs.segment_range(last_step >> 1)?;

        let start_bp = first_range.start;
        let end_bp = last_range.end;

        result.push(PreparedAnnotation {
            first_step,
            last_step,

            start_bp,
            end_bp,

            start_world_x: x0,
            start_world_y: y0,
            end_world_x: x1,
            end_world_y: y1,

            path_steps,
            blocks_1d_bp,
        });
    }

    Some(Json(result))
}