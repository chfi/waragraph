use std::sync::Arc;

use rocket::serde::{json::Json, Serialize};
use rocket::{get, post, State};
use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PathMetadata {
    name: String,
    id: u32,

    // TODO get the length in Bp as well
    step_count: u64,
}

#[get("/path_metadata")]
pub async fn path_metadata(
    graph: &State<Arc<ArrowGFA>>,
) -> Json<Vec<PathMetadata>> {
    let mut out = Vec::with_capacity(graph.path_names.len());

    for (path_id, path_name) in graph.path_names.values_iter().enumerate() {
        let step_count = graph.path_steps(path_id as u32).len() as u64;

        out.push(PathMetadata {
            name: path_name.to_string(),
            id: path_id as u32,

            step_count,
        });
    }

    Json(out)
}

#[get("/path_steps/<path_id>")]
pub async fn path_steps(graph: &State<Arc<ArrowGFA>>, path_id: u32) -> Vec<u8> {
    let buf = graph.path_steps(path_id);
    let mut out: Vec<u32> = Vec::with_capacity(buf.len());
    out.extend_from_slice(buf.values());
    bytemuck::cast_vec(out)
}

#[get("/paths_on_segment/<segment>")]
pub async fn paths_on_segment(
    path_index: &State<Arc<PathIndex>>,
    segment: u32,
) -> Option<Vec<u8>> {
    let vec = path_index.segment_path_matrix.paths_on_segment(segment)?;

    todo!();
}
