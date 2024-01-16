use std::sync::Arc;

use rocket::serde::{json::Json, Serialize};
use rocket::{get, post, State};
use waragraph_core::arrow_graph::ArrowGFA;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PathMetadata {
    name: String,
    id: u64,

    step_count: u64,
}

#[get("/path_metadata")]
pub fn path_metadata(graph: &State<Arc<ArrowGFA>>) -> Json<Vec<PathMetadata>> {
    let mut out = Vec::with_capacity(graph.path_names.len());

    for (path_id, path_name) in graph.path_names.values_iter().enumerate() {
        let step_count = graph.path_steps(path_id as u32).len() as u64;

        out.push(PathMetadata {
            name: path_name.to_string(),
            id: path_id as u64,

            step_count,
        });
    }

    Json(out)
}
