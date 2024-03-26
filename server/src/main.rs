use std::collections::HashMap;
use std::sync::Arc;

use rocket::route::{Handler, Outcome};
use rocket::tokio::sync::RwLock;
use rocket::{get, launch, post, routes, Responder, Route};
use rocket::{Data, Request, State};

use sprs::CsVec;
use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};
use waragraph_core::coordinate_system::CoordSys;
use waragraph_core::graph_layout::GraphLayout;
use waragraph_core::PathId;

use ultraviolet::Vec2;

use crate::datasets::DatasetsCache;

// use waragraph_server::paths;
pub mod coordinate_system;
pub mod datasets;
pub mod paths;

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

// NB just return the sequence byte array for now; arrow IPC later
#[get("/sequence_array")]
async fn get_sequence_array(graph: &State<Arc<ArrowGFA>>) -> Vec<u8> {
    let buf = graph.segment_sequences.values();
    let mut out = Vec::with_capacity(buf.len());
    out.extend_from_slice(buf);
    out
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

#[post("/graph_layout/sample_steps?<tolerance>", data = "<steps_bytes>")]
async fn post_sample_steps_world_space(
    // graph: &State<Arc<ArrowGFA>>,
    layout: &State<GraphLayout>,
    steps_bytes: Vec<u8>,
    tolerance: f32,
) -> Option<Vec<u8>> {
    let steps: &[u32] = bytemuck::try_cast_slice(&steps_bytes).ok()?;

    let mut out = Vec::with_capacity(steps.len() * 2);

    for step in steps {
        todo!();
    }

    Some(out)
}

#[get("/graph_layout/sample_path_id?<path_id>&<start_bp>&<end_bp>&<tolerance>")]
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

    let mut points_out: Vec<u8> = Vec::with_capacity(points.len() * 8);

    for p in points {
        points_out.extend_from_slice(bytemuck::cast_slice(&[p]));
    }

    Some(points_out)
}

#[get("/graph_layout/sample_path_name?<path_name>&<start_bp>&<end_bp>&<tolerance>")]
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

#[post(
    "/path_data/<dataset_key>/<left>/<right>/<bin_count>",
    data = "<path_names>"
)]
async fn batch_sample_path_data(
    graph: &State<Arc<ArrowGFA>>,
    cs_cache: &State<CoordSysCache>,
    dataset_cache: &State<DatasetsCache>,
    path_names: rocket::serde::json::Json<Vec<String>>,
    dataset_key: &str,
    left: u64,
    right: u64,
    bin_count: u32,
) -> Option<Vec<u8>> {
    use rocket::tokio::task::spawn_blocking;

    let cs = {
        let cs_map = cs_cache.map.read().await;
        cs_map.get("<global>").unwrap().clone()
    };

    let datasets = dataset_cache.path_map.read().await;
    let dataset = datasets.get(dataset_key)?.clone();

    let path_datasets = path_names
        .iter()
        .map(|name| -> Option<_> {
            let path_id = graph.path_name_id(name)?;
            let path_data = dataset.get(&PathId(path_id))?;
            Some(path_data.clone())
        })
        .collect::<Vec<_>>();

    let output = spawn_blocking(move || {
        let mut output =
            Vec::<u8>::with_capacity(bin_count as usize * 4 * path_names.len());

        for (i, path) in path_datasets.into_iter().enumerate() {
            let start = i * bin_count as usize;
            let end = start + bin_count as usize;
            let slice = &mut output[start..end];
            if let Some(path_data) = path {
                cs.sample_impl(
                    left..=right,
                    path_data.indices(),
                    path_data.data(),
                    bytemuck::cast_slice_mut(&mut output),
                );
            } else {
                slice.fill(0);
            }
        }

        Some(output)
    })
    .await
    .ok()?;

    output
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
    // TODO: this only samples against the global coord sys for now
    let cs = {
        let cs_map = cs_cache.map.read().await;

        // NB this is generated before the server launches, for now
        cs_map.get("<global>").unwrap().clone()
    };

    // this assumes the target coord sys is the global graph one

    // "dataset" needs to match in size with coord sys
    // no color is applied by the server -- the sampled data is returned
    let path_id = graph.path_name_id(path_name)?;

    let datasets = dataset_cache.path_map.read().await;
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

#[derive(Debug, Default)]
pub struct CoordSysCache {
    pub(crate) map: RwLock<HashMap<String, Arc<CoordSys>>>,
}

impl CoordSysCache {
    // async fn get_for_path(
    //     )

    pub(crate) async fn get_or_compute_for_path(
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

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = std::env::args().collect::<Vec<_>>();

    let graph_path = &args[1];

    let agfa = if graph_path.to_ascii_lowercase().ends_with(".gfa") {
        log::info!("Parsing GFA {graph_path}");

        let gfa =
            std::fs::File::open(graph_path).map(std::io::BufReader::new)?;

        waragraph_core::arrow_graph::parser::arrow_graph_from_gfa(gfa)?
    } else {
        log::info!("Deserializing archived graph {graph_path}");
        // parse as archived arrow GFA

        let t = std::time::Instant::now();
        let (agfa, mmap) = unsafe { ArrowGFA::mmap_archive(graph_path)? };
        let elapsed = t.elapsed().as_millis();
        log::info!("Read graph in {elapsed} ms");
        // if the memory map itself goes out of memory, the ArrowGFA is invalidated;
        // since it's going to live for the entire application, just leak it here
        std::mem::forget(mmap);
        agfa
    };

    use waragraph_core::graph_layout::GraphLayout;

    let tsv = &args[2];

    log::info!("Parsing layout TSV {tsv}");
    let graph_layout = GraphLayout::from_tsv(tsv)?;

    log::info!("Precomputing data");
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
            .path_map
            .write()
            .await
            .insert("depth".to_string(), depth_data);

        let graph_depth = agfa
            .graph_depth_vector()
            .into_iter()
            .map(|v| v as f32)
            .collect::<Vec<_>>();

        datasets
            .graph_map
            .write()
            .await
            .insert("depth".to_string(), Arc::new(graph_depth));
    }

    let cs_cache = CoordSysCache::default();

    {
        let mut cs_map = cs_cache.map.write().await;

        let cs = CoordSys::global_from_arrow_gfa(&agfa);
        cs_map.insert("<global>".to_string(), Arc::new(cs));
    }

    let path_index = PathIndex::from_arrow_gfa(&agfa);

    log::info!("Launching server");
    let _rocket = rocket::build()
        .manage(Arc::new(agfa))
        .manage(Arc::new(path_index))
        .manage(cs_cache)
        .manage(datasets)
        .manage(graph_layout)
        .manage(paths::PathOffsetCache::default())
        // TODO: should be configurable, & only do this in debug mode
        .mount(
            "/",
            FileServerWithHeaders(rocket::fs::FileServer::from("../web/dist")),
        )
        .mount(
            "/",
            routes![
                get_sequence_array,
                get_graph_layout,
                get_segment_position,
                get_sample_path_name_world_space,
                get_sample_path_id_world_space
            ],
        )
        .mount("/", routes![datasets::get_graph_dataset])
        .mount(
            "/",
            routes![
                paths::path_metadata,
                paths::path_steps,
                paths::paths_on_segment
            ],
        )
        .mount(
            "/coordinate_system",
            routes![
                coordinate_system::global,
                coordinate_system::global_segment_range,
                coordinate_system::global_segment_at_offset,
                coordinate_system::path_interval_to_global_blocks,
                coordinate_system::get_segment_at_path_position,
                coordinate_system::prepare_annotation_records,
            ],
        )
        .mount("/sample", routes![sample_path_data, batch_sample_path_data])
        .launch()
        .await?;

    Ok(())
}

#[derive(Clone)]
struct FileServerWithHeaders(rocket::fs::FileServer);

impl From<FileServerWithHeaders> for Vec<Route> {
    fn from(server: FileServerWithHeaders) -> Self {
        let mut routes: Vec<Route> = server.0.clone().into();
        for route in &mut routes {
            route.handler = Box::new(server.clone());
        }
        routes
    }
}

#[rocket::async_trait]
impl Handler for FileServerWithHeaders {
    async fn handle<'r>(
        &self,
        req: &'r Request<'_>,
        data: Data<'r>,
    ) -> Outcome<'r> {
        let mut outcome = self.0.handle(req, data).await;

        if let Outcome::Success(ref mut resp) = &mut outcome {
            resp.set_raw_header("Cross-Origin-Opener-Policy", "same-origin");
            resp.set_raw_header("Cross-Origin-Embedder-Policy", "require-corp");
        }

        outcome
    }
}
