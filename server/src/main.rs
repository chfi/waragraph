use std::collections::HashMap;
use std::sync::Arc;

use rocket::tokio::sync::RwLock;
use rocket::State;
use rocket::{get, launch, routes};

use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};
use waragraph_core::coordinate_system::CoordSys;

#[get("/world")]
fn world() -> &'static str {
    "hello world"
}

// #[get("/args")]
// fn args_route(args_s: &State<ArgsVec>) -> String {
//     let args = args_s.0.join("\n");
//     args
//     // let args = args_vec.join("\n");
// }

// #[get("/coordinate_system/<path_name>")]
// fn coord_sys_path_name(path_name: String) {
// }

#[get("/sample_path_data/<path_name>/<dataset>/<bin_count>")]
async fn sample_path_data(
    graph: &State<Arc<ArrowGFA>>,
    cs_cache: &State<CoordSysCache>,
    path_name: &str,
    dataset: &str,
    bin_count: u32,
) -> Option<()> {
    // get/create coord sys... tokio spawn blocking?
    let cs = cs_cache
        .get_or_compute_for_path(graph.inner(), path_name)
        .await;

    // this assumes the target coord sys is the global graph one

    // "dataset" needs to provide both data buffer and color map

    Some(())
}

// #[derive(Debug, Clone)]
// struct ArgsVec(Vec<String>);

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

        let path_index = graph.path_name_index(path_name)?;

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

    rocket::build()
        .manage(Arc::new(agfa))
        .manage(CoordSysCache::default())
        .mount("/hello", routes![world])
    // .mount("/", routes![args_route])
}

// fn main() {
//     println!("Hello, world!");
// }
