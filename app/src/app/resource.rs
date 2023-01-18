use std::{collections::BTreeMap, sync::Arc};

use egui::epaint::ahash::HashMap;
use waragraph_core::graph::{Node, PathId, PathIndex};

#[derive(Default)]
pub struct AnyArcMap {
    values: HashMap<(std::any::TypeId, u64), Box<dyn std::any::Any>>,
}

impl AnyArcMap {
    fn key<T: std::any::Any>(key: &str) -> (std::any::TypeId, u64) {
        use std::hash::{Hash, Hasher};
        let id = std::any::TypeId::of::<T>();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (id, hasher.finish())
    }

    pub fn insert_shared<T>(&mut self, key: &str, value: Arc<T>)
    where
        T: std::any::Any,
    {
        let key = Self::key::<T>(key);
        self.values.insert(key, Box::new(value));
    }

    pub fn insert<T>(&mut self, key: &str, value: T)
    where
        T: std::any::Any,
    {
        self.insert_shared(key, Arc::new(value));
    }

    pub fn get<'a, T>(&'a self, key: &str) -> Option<&'a Arc<T>>
    where
        T: std::any::Any,
    {
        let key = Self::key::<T>(key);

        let val = self.values.get(&key)?;
        let id = std::any::TypeId::of::<Arc<T>>();
        let val = val.downcast_ref::<Arc<T>>()?;

        Some(val)
    }
}

pub struct ResourceCtx {
    //
}

pub struct GraphData<T> {
    per_node: Vec<T>,
}

// pub struct IndivPathData<T> {
//     per_node_in_step_order: Vec<T>,
// }

pub struct GraphPathData<T> {
    data: Vec<Vec<T>>,
}

pub(crate) enum StoreIndex {
    GraphFloat,
    PathFloat,
    // GraphUnsigned,
    // GraphSigned,
    // PathUnsigned,
    // PathSigned,
}

// pub type DataSourceFn<A, T> = Arc<dyn Fn(A) -> anyhow::Result<Vec<T>>>;

pub type GraphDataSourceFn<T> = Arc<dyn Fn() -> anyhow::Result<Vec<T>>>;
pub type PathDataSourceFn<T> = Arc<dyn Fn(PathId) -> anyhow::Result<Vec<T>>>;

pub struct GraphDataSources {
    graph_f32: HashMap<String, Arc<dyn Fn() -> anyhow::Result<Vec<f32>>>>,
    path_f32: HashMap<String, Arc<dyn Fn(PathId) -> anyhow::Result<Vec<f32>>>>,
}

impl GraphDataSources {
    pub fn tmp_init(graph: &Arc<PathIndex>) -> Self {
        let mut graph_f32: HashMap<String, GraphDataSourceFn<f32>> =
            HashMap::default();
        let mut path_f32: HashMap<String, PathDataSourceFn<f32>> =
            HashMap::default();

        // graph node ids
        {
            let name = "graph_node_ids".to_string();
            let graph = graph.clone();
            let ctor =
                move || Ok((0..graph.node_count).map(|i| i as f32).collect());

            graph_f32.insert(name, Arc::new(ctor));
        }

        // path depth
        {
            let name = "path_depth".to_string();
            let graph = graph.clone();
            let ctor = move |path: PathId| {
                let mut path_data: BTreeMap<Node, f32> = BTreeMap::default();
                for step in graph.path_steps[path.ix()].iter() {
                    *path_data.entry(step.node()).or_default() += 1.0;
                }
                let path_data =
                    path_data.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
                Ok(path_data)
            };

            path_f32.insert(name, Arc::new(ctor));
        }

        // path strand
        {
            let name = "path_strand".to_string();
            let graph = graph.clone();
            let ctor = move |path: PathId| {
                let path_steps = &graph.path_steps[path.ix()];
                let node_set = &graph.path_node_sets[path.ix()];
                let mut step_count = vec![0.0; node_set.len() as usize];
                let mut path_data = vec![0.0; node_set.len() as usize];

                for step in path_steps {
                    let node = step.node();
                    let d = if step.is_reverse() { 1.0 } else { 0.0 };

                    path_data[node.ix()] += d;
                    step_count[node.ix()] += 1.0;
                }

                for (val, count) in path_data.iter_mut().zip(step_count) {
                    *val = *val / count;
                }

                Ok(path_data)
            };

            path_f32.insert(name, Arc::new(ctor));
        }

        Self {
            graph_f32,
            path_f32,
        }
    }
}

pub struct GraphDataCache {
    name_index_map: HashMap<String, StoreIndex>,

    graph_f32: Vec<Arc<GraphData<f32>>>,
    path_f32: Vec<Arc<GraphPathData<f32>>>,
    // gpu_graph_buffers: HashMap<String, Arc<BufferDesc>>,

    // worker: JoinHandle<()>,

    // send_rq: crossbeam::channel::Sender
    // recv_rq: crossbeam::channel::Receiver
}

impl GraphDataCache {
    // pub fn
}

pub struct ResourceManager {
    // requests:
}
