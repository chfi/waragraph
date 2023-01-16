use std::{collections::BTreeMap, sync::Arc};

use egui::epaint::ahash::HashMap;
use waragraph_core::graph::{Node, PathId, PathIndex};

pub struct ResourceStore {
    //
}

pub struct GraphData<T> {
    per_node: Vec<T>,
}

// pub struct IndivPathData<T> {
//     per_node_in_step_order: Vec<T>,
// }

pub struct GraphPathData<T> {
    path_range: std::ops::RangeInclusive<PathId>,
    paths: Vec<Vec<T>>,
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

pub struct GraphDataStore {
    name_index_map: HashMap<String, StoreIndex>,

    graph_f32: Vec<GraphData<f32>>,
    path_f32: Vec<GraphPathData<f32>>,
    // graph_u32: Vec<GraphData<u32>>,
    // graph_i32: Vec<GraphData<i32>>,
    // path_u32: Vec<GraphPathData<u32>>,
    // path_i32: Vec<GraphPathData<i32>>,
}

// impl GraphIndexDataStore {
// }
