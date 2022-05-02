use std::{
    collections::{BTreeMap, HashMap},
    io::BufReader,
    num::NonZeroU32,
};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
use parking_lot::RwLock;
use raving::{
    script::console::BatchBuilder,
    vk::{context::VkContext, BufferIx, GpuResources, VkEngine},
};
use rustc_hash::FxHashMap;

use sled::IVec;
use thunderdome::{Arena, Index};

use sprs::{CsMatI, CsVecI, TriMatI};
use zerocopy::{AsBytes, FromBytes};

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use bstr::ByteSlice as BstrByteSlice;

use crate::{
    graph::{Node, Path, Waragraph},
    util::{BufFmt, BufId, BufMeta, BufferStorage, LabelStorage},
    viewer::{DataSource, SlotFnCache, ViewDiscrete1D},
};

use rhai::plugin::*;

use lazy_static::lazy_static;

use self::rhai_module::DataSourceF32;

#[derive(Clone)]
pub enum BedColumn {
    Int(Vec<i64>),
    Float(Vec<f32>),
    String(Vec<rhai::ImmutableString>),
    Dyn(Vec<rhai::Dynamic>),
}

pub struct AnnotationSet {
    // e.g. BED file path
    source: rhai::ImmutableString,

    // records: Vec<BEDRecord>,

    // map from path to a map from nodes to bitmaps representing the
    // record indices at each node
    path_record_indices:
        FxHashMap<Path, BTreeMap<Node, roaring::RoaringBitmap>>,

    path_record_nodes: FxHashMap<Path, BTreeMap<usize, roaring::RoaringBitmap>>,
    // columns: Vec<Vec<
    column_headers: FxHashMap<rhai::ImmutableString, usize>,
    columns: Vec<BedColumn>,
}

impl AnnotationSet {
    pub fn load_bed<P: AsRef<std::path::Path>>(
        graph: &Arc<Waragraph>,
        path: P,
    ) -> Result<AnnotationSet> {
        use std::fs::File;
        use std::io::prelude::*;

        let path = path.as_ref();

        let source = path
            .to_str()
            .map(rhai::ImmutableString::from)
            .unwrap_or("UNKNOWN".into());

        let mut path_record_indices: FxHashMap<
            Path,
            BTreeMap<Node, roaring::RoaringBitmap>,
        > = FxHashMap::default();

        let mut path_record_nodes: FxHashMap<
            Path,
            BTreeMap<usize, roaring::RoaringBitmap>,
        > = FxHashMap::default();

        let mut column_headers = FxHashMap::default();

        let mut columns = Vec::new();

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut header_done = false;
        let mut ix = 0;

        for line in reader.lines() {
            let line = line?;

            if !header_done {
                if line.starts_with("#") {
                    //
                } else if line.starts_with("browser") {
                    //
                } else if line.starts_with("track") {
                    //
                } else {
                    header_done = true;
                }
            }

            if header_done {
                let mut fields = line.split("\t");

                let path_name = fields.next().unwrap();
                let start = fields.next().unwrap().parse::<usize>()?;
                let end = fields.next().unwrap().parse::<usize>()?;

                if let Some(path) = graph.path_index(path_name.as_bytes()) {
                    let offset = graph.path_offset(path);
                    let start = start - offset;
                    let end = end - offset;

                    let indices = path_record_indices.entry(path).or_default();
                    let nodes = path_record_nodes.entry(path).or_default();

                    for &(node, _) in
                        graph.nodes_in_path_range(path, start..end)
                    {
                        indices.entry(node).or_default().insert(ix as u32);
                        nodes.entry(ix).or_default().insert(node.into());
                    }
                }

                ix += 1;
            }
        }

        Ok(AnnotationSet {
            source,
            path_record_indices,
            path_record_nodes,
            column_headers,
            columns,
        })
    }

    pub fn path_records(
        &self,
        path: Path,
    ) -> Option<&BTreeMap<Node, roaring::RoaringBitmap>> {
        self.path_record_indices.get(&path)
    }

    pub fn path_node_records(
        &self,
        path: Path,
        node: Node,
    ) -> Option<&roaring::RoaringBitmap> {
        let path = self.path_record_indices.get(&path)?;
        path.get(&node)
    }
}

pub fn create_rhai_module() -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(rhai_module);

    module
}

pub fn add_cache_fns(
    module: &mut rhai::Module,
    slot_fns: &Arc<RwLock<SlotFnCache>>,
) {
    let cache = slot_fns.clone();
    module.set_native_fn(
        "set_slot_color_scheme",
        move |slot_fn: rhai::ImmutableString,
              color_buffer: rhai::ImmutableString| {
            cache.write().slot_color.insert(slot_fn, color_buffer);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn("get_slot_color_scheme", move |slot_fn: &str| {
        if let Some(color) = cache.read().slot_color.get(slot_fn) {
            Ok(rhai::Dynamic::from(color.to_owned()))
        } else {
            Ok(rhai::Dynamic::FALSE)
        }
    });

    let cache = slot_fns.clone();
    module.set_native_fn(
        "register_data_source",
        move |name: rhai::ImmutableString, f: DataSource<u32>| {
            cache.write().data_sources_u32.insert(name, f);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "register_data_source",
        move |name: rhai::ImmutableString, f: DataSource<f32>| {
            cache.write().data_sources_f32.insert(name, f);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "register_data_source",
        move |name: rhai::ImmutableString, f: DataSource<rhai::Dynamic>| {
            cache.write().data_sources_dyn.insert(name, f);
            Ok(true)
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "get_data_source",
        move |name: rhai::ImmutableString| {
            let cache = cache.read();
            if let Some(data) = cache.data_sources_u32.get(&name) {
                Ok(rhai::Dynamic::from(data.clone()))
            } else if let Some(data) = cache.data_sources_f32.get(&name) {
                Ok(rhai::Dynamic::from(data.clone()))
            } else if let Some(data) = cache.data_sources_dyn.get(&name) {
                Ok(rhai::Dynamic::from(data.clone()))
            } else {
                Ok(rhai::Dynamic::FALSE)
            }
        },
    );
    //
}

// pub fn add_channel_fns(module: &mut rhai::Module,

// pub fn add_channel_fns_engine(engine: &mut rhai::Engine,

#[export_module]
pub mod rhai_module {
    use rhai::plugin::RhaiResult;

    use std::sync::Arc;

    use crate::{
        console::EvalResult,
        graph::{Node, Path, Waragraph},
        viewer::{DataSource, SlotUpdateFn},
    };

    pub type SlotFnCache = Arc<RwLock<crate::viewer::SlotFnCache>>;

    pub type ArcBytestring = Arc<Vec<u8>>;

    pub type DataSourceDyn = DataSource<rhai::Dynamic>;

    pub type DataSourceF32 = DataSource<f32>;
    pub type DataSourceU32 = DataSource<u32>;
    pub type DataSourceI32 = DataSource<i32>;
    pub type DataSourceI64 = DataSource<i64>;

    pub type SlotUpdateFnU32 = SlotUpdateFn<u32>;
    pub type SlotUpdateFnF32 = SlotUpdateFn<f32>;

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_dyn(
        d: &mut DataSource<rhai::Dynamic>,
        path: Path,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, Node::from(node as u32)) {
            v
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_f32(
        d: &mut DataSource<f32>,
        path: Path,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, Node::from(node as u32)) {
            rhai::Dynamic::from_float(v)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_u32(
        d: &mut DataSource<u32>,
        path: Path,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, Node::from(node as u32)) {
            rhai::Dynamic::from_int(v as i64)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "call")]
    pub fn call_data_source_i64(
        d: &mut DataSource<i64>,
        path: Path,
        node: i64,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, Node::from(node as u32)) {
            rhai::Dynamic::from_int(v)
        } else {
            rhai::Dynamic::UNIT
        }
    }
}
