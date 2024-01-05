use std::collections::HashMap;
use std::sync::Arc;

use rocket::tokio::sync::RwLock;
use rocket::State;
use rocket::{get, launch, routes};

use sprs::CsVec;
use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};
use waragraph_core::coordinate_system::CoordSys;
use waragraph_core::PathId;

// #[get("/args")]
// fn args_route(args_s: &State<ArgsVec>) -> String {
//     let args = args_s.0.join("\n");
//     args
//     // let args = args_vec.join("\n");
// }

// #[get("/coordinate_system/<path_name>")]
// fn coord_sys_path_name(path_name: String) {
// }

#[get("/path_data/<path_name>/<dataset>/<left>/<right>/<bin_count>")]
async fn sample_path_data(
    graph: &State<Arc<ArrowGFA>>,
    cs_cache: &State<CoordSysCache>,
    dataset_cache: &State<DatasetsCache>,
    path_name: &str,
    dataset: &str,
    left: u64,
    right: u64,
    bin_count: u32,
) -> Option<Vec<u8>> {
    let cs = cs_cache
        .get_or_compute_for_path(graph.inner(), path_name)
        .await?;

    // this assumes the target coord sys is the global graph one

    // "dataset" needs to match in size with coord sys
    // no color is applied by the server -- the sampled data is returned
    let path_id = graph.path_name_id(path_name)?;

    let datasets = dataset_cache.map.read().await;
    let path_data = datasets.get(dataset)?.get(&PathId(path_id))?;

    let indices = path_data.indices();
    let data = path_data.data();

    let mut output = vec![0u8; bin_count as usize * 4];

    cs.sample_impl(
        left..=right,
        indices,
        data,
        bytemuck::cast_slice_mut(&mut output),
    );

    Some(output)
}

// NB: this is a placeholder; the path map level is missing
#[derive(Debug, Default)]
struct DatasetsCache {
    // map: RwLock<HashMap<String, Arc<Vec<f32>>>>,
    map: RwLock<HashMap<String, HashMap<PathId, Arc<sprs::CsVecI<f32, u32>>>>>,
}

impl DatasetsCache {
    async fn get_dataset(
        &self,
        data_key: &str,
        path_id: PathId,
    ) -> Option<Arc<sprs::CsVecI<f32, u32>>> {
        self.map.read().await.get(data_key)?.get(&path_id).cloned()
    }

    async fn set_dataset(
        &self,
        data_key: &str,
        path_id: PathId,
        data: sprs::CsVecI<f32, u32>,
    ) {
        self.map
            .write()
            .await
            .entry(data_key.to_string())
            .or_default()
            .insert(path_id, Arc::new(data));
    }
}

#[derive(Debug, Default)]
struct CoordSysCache {
    map: RwLock<HashMap<String, Arc<CoordSys>>>,
}

impl CoordSysCache {
    // async fn get_for_path(
    //     )

    async fn get_or_compute_for_path(
        &self,
        graph: &Arc<ArrowGFA>,
        path_name: &str,
    ) -> Option<Arc<CoordSys>> {
        use rocket::tokio::task::spawn_blocking;

        let path_index = graph.path_name_id(path_name)?;

        {
            let map = self.map.read().await;

            // TODO: should be Global | Path (name)
            if map.contains_key(path_name) {
                return map.get(path_name).cloned();
            }
        }

        let graph = graph.clone();
        let cs = spawn_blocking(move || {
            CoordSys::path_from_arrow_gfa(&graph, path_index)
        })
        .await
        .ok()?;

        let cs = Arc::new(cs);

        self.map
            .write()
            .await
            .insert(path_name.to_string(), cs.clone());

        // {
        // let mut map = self.map.write().await;
        // map.insert(path_name, cs.clone());
        // }

        Some(cs)
    }
}

#[launch]
fn rocket() -> _ {
    let args = std::env::args().collect::<Vec<_>>();

    let gfa = &args[1];
    // let tsv = args[2];

    let gfa = std::fs::File::open(gfa)
        .map(std::io::BufReader::new)
        .unwrap();
    // let tsv = std::fs::File::open(tsv).unwrap();

    let agfa =
        waragraph_core::arrow_graph::parser::arrow_graph_from_gfa(gfa).unwrap();

    // TODO these should be computed on demand
    let datasets = DatasetsCache::default();
    {
        let mut depth_data = HashMap::default();

        for (path_id, _name) in agfa.path_names.values_iter().enumerate() {
            let depth = agfa.path_vector_sparse_u32(path_id as u32);

            let n = depth.dim();
            let (indices, data) = depth.into_raw_storage();
            let f_data = data.into_iter().map(|v| v as f32).collect::<Vec<_>>();
            let depth = sprs::CsVecI::new(n, indices, f_data);

            depth_data.insert(PathId(path_id as u32), Arc::new(depth));
        }

        datasets
            .map
            .blocking_write()
            .insert("depth".to_string(), depth_data);
    }

    rocket::build()
        .manage(Arc::new(agfa))
        .manage(CoordSysCache::default())
        .manage(datasets)
        .mount("/sample", routes![sample_path_data])
}

// fn main() {
//     println!("Hello, world!");
// }
