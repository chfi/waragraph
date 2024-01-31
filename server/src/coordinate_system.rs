use std::sync::Arc;

use arrow2::array::{
    Array, BinaryArray, Float32Array, Int32Array, StructArray, UInt32Array,
    Utf8Array,
};
use arrow2::chunk::Chunk;
use arrow2::datatypes::DataType;
use arrow2::io::ipc::write::{FileWriter, WriteOptions};
use rocket::serde::{json::Json, Serialize};
use rocket::{get, post, State};
use waragraph_core::arrow_graph::ArrowGFA;
use waragraph_core::coordinate_system::CoordSys;

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
) -> Option<Json<[u64; 2]>> {
    let cs_map = coord_sys_cache.map.read().await;
    let global_cs = cs_map.get("<global>")?;
    let range = global_cs.segment_range(segment)?;

    Some(Json([range.start, range.end]))
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

#[get("/path_interval_to_global_blocks?<path_id>&<start_bp>&<end_bp>")]
pub async fn path_interval_to_global_blocks(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<crate::CoordSysCache>,
    path_id: u32,
    start_bp: u64,
    end_bp: u64,
) -> Vec<u8> {
    // let global_cs = coord_sys_cache
    //     .map
    //     .read()
    //     .await
    //     .get("global")
    //     .unwrap()
    //     .clone();

    let path_cs = coord_sys_cache
        .get_or_compute_for_path(graph, path_id)
        .await
        .unwrap();

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
    // bytemuck::cast_vec(ranges)
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
