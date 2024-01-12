use std::collections::HashMap;
use std::sync::Arc;

use rocket::tokio::sync::RwLock;
use rocket::State;
use rocket::{get, launch, routes};

use sprs::CsVec;
use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};
use waragraph_core::coordinate_system::CoordSys;
use waragraph_core::graph_layout::GraphLayout;
use waragraph_core::PathId;

use ultraviolet::Vec2;

// #[get("/args")]
// fn args_route(args_s: &State<ArgsVec>) -> String {
//     let args = args_s.0.join("\n");
//     args
//     // let args = args_vec.join("\n");
// }

// #[get("/coordinate_system/<path_name>")]
// fn coord_sys_path_name(path_name: String) {
// }

#[get("/graph_layout")]
async fn get_graph_layout(layout: &State<GraphLayout>) -> Vec<u8> {
    use arrow2::io::ipc::write::FileWriter;
    use arrow2::io::ipc::write::WriteOptions;

    use arrow2::chunk::Chunk;

    let mut buf: Vec<u8> = vec![];

    let schema = layout.arrow_schema();

    // Cursor::n`
    let mut writer = FileWriter::new(
        std::io::Cursor::new(&mut buf),
        schema,
        None,
        WriteOptions { compression: None },
    );

    let chunk =
        Chunk::new(vec![layout.xs.clone().boxed(), layout.ys.clone().boxed()]);

    writer.start().unwrap();

    writer.write(&chunk, None).unwrap();

    writer.finish().unwrap();

    //
    buf
}

#[get("/graph_layout/segment_position/<segment>")]
async fn get_segment_position(
    layout: &State<GraphLayout>,
    segment: u32,
) -> Option<Vec<u8>> {
    // lol should just use serde & probably return json
    let positions = layout.segment_position(segment)?;
    let mut out = vec![0u8; 16];
    out[0..16].clone_from_slice(bytemuck::cast_slice(&positions));
    Some(out)
}

#[get("/graph_layout/sample_path?<path_id>&<start_bp>&<end_bp>&<tolerance>")]
async fn get_sample_path_id_world_space(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<CoordSysCache>,
    layout: &State<GraphLayout>,
    path_id: u32,
    start_bp: u64,
    end_bp: u64,
    tolerance: f32,
) -> Option<Vec<u8>> {
    // get/build coord sys for path
    let cs = coord_sys_cache
        .get_or_compute_for_path(graph.inner(), path_id)
        .await?;

    let step_range = cs.bp_to_step_range(start_bp, end_bp);
    let path_steps = graph.path_steps(path_id);
    let path_slice = {
        let start = *step_range.start();
        let end = *step_range.end();
        path_steps.clone().sliced(start, end - start - 1)
    };
    // let path_slice = path_steps.slice(step_range.start(), step_range.end() - step_range.start - 1)

    if path_slice.len() == 0 {
        return Some(vec![]);
    }

    let tol_sq = tolerance * tolerance;

    let path_vertices = path_slice.values_iter().flat_map(|&step_handle| {
        let seg = step_handle >> 1;
        let i = (seg * 2) as usize;

        let p0 =
            Vec2::new(layout.xs.get(i).unwrap(), layout.ys.get(i).unwrap());
        let p1 = Vec2::new(
            layout.xs.get(i + 1).unwrap(),
            layout.ys.get(i + 1).unwrap(),
        );

        [p0, p1]
    });

    let mut last_vertex = None;

    let mut points: Vec<Vec2> = Vec::new();

    for p in path_vertices {
        last_vertex = Some(p);

        if let Some(last_p) = points.last().copied() {
            let delta = p - last_p;
            let _dist_sq = delta.mag_sq();

            if delta.mag_sq() >= tol_sq {
                points.push(p);
            }
        } else {
            points.push(p);
        }
    }

    if points.len() == 1 {
        if let Some(p) = last_vertex {
            if p != points[0] {
                points.push(p);
            }
        }
    }

    Some(bytemuck::cast_vec(points))
}

#[get("/graph_layout/sample_path?<path_name>&<start_bp>&<end_bp>&<tolerance>")]
async fn get_sample_path_name_world_space(
    graph: &State<Arc<ArrowGFA>>,
    coord_sys_cache: &State<CoordSysCache>,
    layout: &State<GraphLayout>,
    path_name: &str,
    start_bp: u64,
    end_bp: u64,
    tolerance: f32,
) -> Option<Vec<u8>> {
    let path_id = graph.path_name_id(path_name)?;

    get_sample_path_id_world_space(
        graph,
        coord_sys_cache,
        layout,
        path_id,
        start_bp,
        end_bp,
        tolerance,
    )
    .await
}

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
    let path_id = graph.path_name_id(path_name)?;

    let cs = cs_cache
        .get_or_compute_for_path(graph.inner(), path_id)
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
        path_id: u32,
    ) -> Option<Arc<CoordSys>> {
        use rocket::tokio::task::spawn_blocking;

        let path_name = graph.path_name(path_id)?;
        // let path_iid = graph.path_name_id(path_name)?;

        {
            let map = self.map.read().await;

            // TODO: should be Global | Path (name)
            if map.contains_key(path_name) {
                return map.get(path_name).cloned();
            }
        }

        let graph = graph.clone();
        let cs = spawn_blocking(move || {
            CoordSys::path_from_arrow_gfa(&graph, path_id)
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

    let gfa = std::fs::File::open(gfa)
        .map(std::io::BufReader::new)
        .unwrap();

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

    use waragraph_core::graph_layout::GraphLayout;

    let tsv = &args[2];

    let graph_layout = match GraphLayout::from_tsv(tsv) {
        Ok(g) => g,
        Err(e) => panic!("${:#?}", e),
    };

    rocket::build()
        .manage(Arc::new(agfa))
        .manage(CoordSysCache::default())
        .manage(datasets)
        .manage(graph_layout)
        // TODO: should be configurable, & only do this in debug mode
        .mount("/", rocket::fs::FileServer::from("../web/dist"))
        .mount(
            "/",
            routes![
                get_graph_layout,
                get_segment_position,
                get_sample_path_name_world_space,
                get_sample_path_id_world_space
            ],
        )
        .mount("/sample", routes![sample_path_data])
}
