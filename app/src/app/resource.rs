use std::{collections::BTreeMap, sync::Arc};

use egui::epaint::ahash::HashMap;
use tokio::{sync::RwLock, task::JoinHandle};
use waragraph_core::graph::{sampling::PathData, Node, PathId, PathIndex};

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

pub struct GraphData<T> {
    pub node_data: Vec<T>,
}

pub struct GraphPathData<T> {
    pub path_data: Vec<Vec<T>>,
}

impl<T> PathData<T> for GraphPathData<T> {
    fn get_path(&self, path_id: PathId) -> &[T] {
        &self.path_data[path_id.ix()]
    }
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
            let name = "node_id".to_string();
            let graph = graph.clone();
            let ctor =
                move || Ok((0..graph.node_count).map(|i| i as f32).collect());

            graph_f32.insert(name, Arc::new(ctor));
        }

        // path depth
        {
            let name = "depth".to_string();
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
            let name = "strand".to_string();
            let graph = graph.clone();
            let ctor = move |path: PathId| {
                let path_steps = &graph.path_steps[path.ix()];
                let node_set = &graph.path_node_sets[path.ix()];

                let mut path_data: BTreeMap<Node, (f32, f32)> =
                    BTreeMap::default();

                for step in path_steps {
                    let node = step.node();
                    let d = if step.is_reverse() { 1.0 } else { 0.0 };

                    let (v, n) = path_data.entry(node).or_insert((0.0, 0.0));

                    *v += d;
                    *n += 1.0;
                }

                let path_data = path_data
                    .into_iter()
                    .map(|(_node, (v, count))| v / count)
                    .collect::<Vec<_>>();

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
    graph: Arc<PathIndex>,
    graph_f32: RwLock<HashMap<String, Arc<GraphData<f32>>>>,
    path_f32: RwLock<HashMap<String, Arc<GraphPathData<f32>>>>,

    // fetching: HashMap<StoreIndex, JoinHandle<()>>,
    // ready: HashSet<StoreIndex>,
    sources: GraphDataSources,
}

impl GraphDataCache {
    pub fn init(graph: &Arc<PathIndex>) -> Self {
        let sources = GraphDataSources::tmp_init(graph);

        let graph_f32 = RwLock::new(HashMap::default());
        let path_f32 = RwLock::new(HashMap::default());

        let graph = graph.clone();

        Self {
            graph,
            graph_f32,
            path_f32,
            sources,
        }
    }

    pub fn fetch_graph_data_blocking(
        &self,
        key: &str,
    ) -> Option<Arc<GraphData<f32>>> {
        if let Some(data) = self.graph_f32.blocking_read().get(key) {
            return Some(data.clone());
        }

        let source = self.sources.graph_f32.get(key)?;

        let node_data = source().unwrap();
        let data = Arc::new(GraphData { node_data });

        self.graph_f32
            .blocking_write()
            .insert(key.to_string(), data.clone());

        Some(data)
    }

    pub fn fetch_path_data_blocking(
        &self,
        key: &str,
    ) -> Option<Arc<GraphPathData<f32>>> {
        if let Some(data) = self.path_f32.blocking_read().get(key) {
            return Some(data.clone());
        }

        let source = self.sources.path_f32.get(key)?;

        let path_ids = self.graph.path_names.left_values();
        let mut data = Vec::with_capacity(self.graph.path_names.len());

        for &path in path_ids {
            let path_data = source(path).unwrap();
            data.push(path_data);
        }

        let data = Arc::new(GraphPathData { path_data: data });

        self.path_f32
            .blocking_write()
            .insert(key.to_string(), data.clone());

        Some(data)
    }
}
