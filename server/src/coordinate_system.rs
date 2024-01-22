use std::sync::Arc;

use arrow2::array::{
    BinaryArray, Float32Array, Int32Array, StructArray, UInt32Array, Utf8Array,
};
use arrow2::chunk::Chunk;
use arrow2::datatypes::DataType;
use arrow2::io::ipc::write::{FileWriter, WriteOptions};
use rocket::serde::{json::Json, Serialize};
use rocket::{get, post, State};
use waragraph_core::arrow_graph::ArrowGFA;
use waragraph_core::coordinate_system::CoordSys;

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

    println!("node_order: {}", cs.node_order.len());
    println!("offsets: {}", offsets.len());
    println!("offset 0: {:?}", offsets.get(0));

    let olen = offsets.len();
    let offsets = offsets.sliced(1, olen - 1);

    let chunk =
        Chunk::new(vec![cs.node_order.clone().boxed(), offsets.boxed()]);

    writer.start().unwrap();

    writer.write(&chunk, None).unwrap();

    writer.finish().unwrap();

    println!("buffer length: {}", buf.len());

    buf
}
