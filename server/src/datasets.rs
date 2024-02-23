use std::collections::HashMap;
use std::sync::Arc;

use rocket::tokio::sync::RwLock;
use rocket::State;
use rocket::{get, launch, post, routes};

use sprs::CsVec;
use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};
use waragraph_core::coordinate_system::CoordSys;
use waragraph_core::graph_layout::GraphLayout;
use waragraph_core::PathId;

use ultraviolet::Vec2;

#[derive(Debug, Default)]
pub struct DatasetsCache {
    pub path_map:
        RwLock<HashMap<String, HashMap<PathId, Arc<sprs::CsVecI<f32, u32>>>>>,
    pub graph_map: RwLock<HashMap<String, Arc<Vec<f32>>>>,
}

// #[get("/graph_dataset/<data_key>")]
// pub async fn get_graph_dataset(
//     data_key: &str,
//     datasets: &State<DatasetsCache>,
// ) -> Option<Arc<[u8]>> {
//     let graph_map = datasets.graph_map.read().await;
//     let data = graph_map.get(data_key)?;
//     Some(bytemuck::cast_slice(data))
// }

#[get("/graph_dataset/<data_key>")]
pub async fn get_graph_dataset(
    data_key: &str,
    datasets: &State<DatasetsCache>,
) -> Option<Vec<u8>> {
    let graph_map = datasets.graph_map.read().await;
    let data = graph_map.get(data_key)?;
    // rocket has Responder impls for Arc<[u8]>, Vec<u8>, &'r [u8], but not Arc<Vec<u8>>;
    // should solve this with a wrapper & new impl later to avoid needless copying
    let mut inner: Vec<u8> = Vec::with_capacity(data.len() * 4);
    inner.extend_from_slice(bytemuck::cast_slice(data));
    Some(inner)
}
