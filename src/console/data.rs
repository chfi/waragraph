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

#[derive(Clone)]
pub enum BedColumn {
    Int(Vec<i64>),
    Float(Vec<f32>),
    String(Vec<rhai::ImmutableString>),
    // Dyn(Vec<rhai::Dynamic>),
}

impl BedColumn {
    pub fn parse_push(&mut self, field: &str) -> Result<()> {
        match self {
            BedColumn::Int(vs) => {
                let val = field.parse::<i64>()?;
                vs.push(val);
            }
            BedColumn::Float(vs) => {
                let val = field.parse::<f32>()?;
                vs.push(val);
            }
            BedColumn::String(vs) => {
                vs.push(rhai::ImmutableString::from(field));
            }
        }
        Ok(())
    }
}

pub struct AnnotationSet {
    // e.g. BED file path
    source: rhai::ImmutableString,

    // records: Vec<BEDRecord>,

    // map from path to a map from nodes to bitmaps representing the
    // record indices at each node
    path_record_indices:
        FxHashMap<Path, BTreeMap<Node, roaring::RoaringBitmap>>,

    record_nodes: BTreeMap<usize, roaring::RoaringBitmap>,
    // path_record_nodes: FxHashMap<Path, BTreeMap<usize, roaring::RoaringBitmap>>,
    // columns: Vec<Vec<
    column_headers: FxHashMap<rhai::ImmutableString, usize>,
    pub columns: Vec<BedColumn>,
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

        // let mut path_record_nodes: FxHashMap<
        //     Path,
        //     BTreeMap<usize, roaring::RoaringBitmap>,
        // > = FxHashMap::default();

        let mut record_nodes: BTreeMap<usize, roaring::RoaringBitmap> =
            BTreeMap::default();

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

                    // prepare the columns
                    let mut fields = line.split("\t");

                    for (ix, field) in fields.skip(3).enumerate() {
                        let col = ix + 3;

                        if let Ok(_int) = field.parse::<i64>() {
                            columns.push(BedColumn::Int(Vec::new()));
                        } else if let Ok(_float) = field.parse::<f32>() {
                            columns.push(BedColumn::Float(Vec::new()));
                        } else {
                            columns.push(BedColumn::String(Vec::new()));
                        }
                    }
                }
            }

            if header_done {
                let mut fields = line.split("\t");

                let path_name = fields.next().unwrap();
                let start = fields.next().unwrap().parse::<usize>()?;
                let end = fields.next().unwrap().parse::<usize>()?;

                for (col_ix, field) in fields.enumerate() {
                    let col = col_ix + 3;
                    if let Err(e) = columns[col_ix].parse_push(field) {
                        log::error!(
                            "error parsing row {}, column {}: {:?}",
                            ix,
                            col,
                            e
                        );
                    }
                }

                if let Some(path) = graph.path_index(path_name.as_bytes()) {
                    let offset = graph.path_offset(path);
                    let start = start - offset;
                    let end = end - offset;

                    let indices = path_record_indices.entry(path).or_default();
                    let nodes = record_nodes.entry(ix).or_default();

                    for &(node, _) in
                        graph.nodes_in_path_range(path, start..end)
                    {
                        indices.entry(node).or_default().insert(ix as u32);
                        nodes.insert(node.into());
                    }
                }

                ix += 1;
            }
        }

        Ok(AnnotationSet {
            source,
            path_record_indices,
            record_nodes,
            // path_record_nodes,
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

    pub fn nodes_on_record(
        &self,
        record_ix: usize,
    ) -> Option<&roaring::RoaringBitmap> {
        self.record_nodes.get(&record_ix)
    }

    pub fn records_on_path_node(
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

pub fn add_module_fns(
    module: &mut rhai::Module,
    slot_fns: &Arc<RwLock<SlotFnCache>>,
    annotations: &Arc<
        RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>,
    >,
) {
    let annots = annotations.clone();
    module.set_native_fn(
        "get_annotation_set",
        move |source: rhai::ImmutableString| {
            let annots = annots.read();
            if let Some(set) = annots.get(&source) {
                Ok(set.to_owned())
            } else {
                Err(format!("annotation set `{}` not found", source).into())
            }
        },
    );

    let annots = annotations.clone();
    module.set_native_fn(
        "load_bed_file",
        move |graph: &mut Arc<Waragraph>, path: rhai::ImmutableString| {
            let file_path = std::path::Path::new(path.as_str());

            match AnnotationSet::load_bed(graph, file_path) {
                Ok(set) => {
                    let source = path;
                    let set = Arc::new(set);
                    annots.write().insert(source.clone(), set.clone());
                    Ok(set)
                }
                Err(err) => {
                    Err(format!("Error parsing BED file: {:?}", err).into())
                }
            }
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "create_data_source",
        move |set: &mut Arc<AnnotationSet>| {
            let mut cache = cache.write();
            let source_str = set.source.as_str();
            let source = set.source.clone();

            if cache.get_data_source_u32(source_str).is_some() {
                return Ok(source.clone());
            }

            let set = set.clone();
            cache.register_data_source_u32(source_str, move |path, node| {
                let indices = set.records_on_path_node(path, node)?;
                indices.select(0)
            });

            return Ok(source);
        },
    );

    let cache = slot_fns.clone();
    module.set_native_fn(
        "new_slot_fn_from_data_source",
        move |data_source_name: rhai::ImmutableString,
              slot_fn_name: rhai::ImmutableString| {
            let mut cache = cache.write();
            dbg!();
            if let Some(slot_fn) =
                cache.slot_fn_mid_u32(&data_source_name, |v| v)
            {
                dbg!();
                cache.slot_fn_u32.insert(slot_fn_name, slot_fn);
                dbg!();
                Ok(rhai::Dynamic::TRUE)
            } else {
                Ok(rhai::Dynamic::FALSE)
            }
        },
    );

    /*
    module.set_raw_fn(
        "insert_slot_fn",
        rhai::FnNamespace::Global,
        rhai::FnAccess::Public,
        [std::any::TypeId::of::<Arc<AnnotationSet>>()],
        move |ctx, args| {
            let set = std::mem::take(args[0]);
            let set = set.cast::<Arc<AnnotationSet>>();

            ctx.call_fn("create_data_source", args)
            // ctx.call_fn_raw(fn_name, is_ref_mut, is_method_call, args)
        },
    );
    */

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

    #[rhai_fn(return_raw)]
    pub fn queue_new_slot_fn(
        slot_fns: &mut SlotFnCache,
        name: &str,
        data_sources: rhai::Array,
        fn_ptr: rhai::FnPtr,
    ) -> EvalResult<rhai::Dynamic> {
        let mut sources = Vec::new();

        for desc in data_sources.into_iter() {
            let (ty, name) = desc.try_cast::<rhai::Map>()
                .and_then(|map| {
                    let ty = map.get("type")?;
                    let name = map.get("name")?;

                    if ty.type_name() == "string" && name.type_name() == "string" {
                        Some((ty.clone_cast(), name.clone_cast()))
                    } else {
                        None
                    }
                }).ok_or(
                "Element must be a map with `type` and `name` string fields",
            )?;

            let ty: rhai::ImmutableString = ty;
            let name: rhai::ImmutableString = name;

            match ty.as_str() {
                "u32" => {
                    sources.push((std::any::TypeId::of::<u32>(), name));
                }
                /*
                "f32" => {
                    //
                }
                "i64" => {
                    //
                }
                "dyn" => {
                    //
                }
                */
                _ => {
                    return Err(
                        format!("unknown data source type: {}", ty).into()
                    )
                }
            }
        }

        slot_fns.write().queue_slot_fn(name, sources, fn_ptr);

        Ok(rhai::Dynamic::UNIT)
    }

    #[rhai_fn(global, name = "at")]
    pub fn get_data_source_dyn(
        d: &mut DataSource<rhai::Dynamic>,
        path: Path,
        node: Node,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, node) {
            v
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "at")]
    pub fn get_data_source_f32(
        d: &mut DataSource<f32>,
        path: Path,
        node: Node,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, node) {
            rhai::Dynamic::from_float(v)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "at")]
    pub fn get_data_source_u32(
        d: &mut DataSourceU32,
        path: Path,
        node: Node,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, node) {
            rhai::Dynamic::from_int(v as i64)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    #[rhai_fn(global, name = "at")]
    pub fn get_data_source_i64(
        d: &mut DataSource<i64>,
        path: Path,
        node: Node,
    ) -> rhai::Dynamic {
        if let Some(v) = d(path, node) {
            rhai::Dynamic::from_int(v)
        } else {
            rhai::Dynamic::UNIT
        }
    }

    pub type AnnotationSet = Arc<super::AnnotationSet>;

    #[rhai_fn(return_raw)]
    pub fn nodes_in_record(
        set: &mut AnnotationSet,
        record_ix: i64,
    ) -> EvalResult<rhai::Array> {
        if let Some(nodes) = set.nodes_on_record(record_ix as usize) {
            let nodes = nodes
                .iter()
                .map(|i| rhai::Dynamic::from(Node::from(i)))
                .collect::<Vec<_>>();

            Ok(nodes)
        } else {
            Err("record out of bounds".into())
        }
    }

    #[rhai_fn(return_raw)]
    pub fn records_on_node(
        set: &mut AnnotationSet,
        path: Path,
        node: Node,
    ) -> EvalResult<rhai::Array> {
        if let Some(records) = set.records_on_path_node(path, node) {
            let records = records
                .iter()
                .map(|i| rhai::Dynamic::from(i as i64))
                .collect::<Vec<_>>();

            Ok(records)
        } else {
            Err("node not found in path records".into())
        }
    }

    #[rhai_fn(global, return_raw)]
    pub fn get_record_field(
        set: &mut AnnotationSet,
        record_ix: i64,
        column: i64,
    ) -> EvalResult<rhai::Dynamic> {
        if let Some(column) = set.columns.get(column as usize) {
            match column {
                super::BedColumn::Int(vs) => {
                    if let Some(val) = vs.get(record_ix as usize) {
                        return Ok(rhai::Dynamic::from_int(*val));
                    }
                }
                super::BedColumn::Float(vs) => {
                    if let Some(val) = vs.get(record_ix as usize) {
                        return Ok(rhai::Dynamic::from_float(*val));
                    }
                }
                super::BedColumn::String(vs) => {
                    if let Some(val) = vs.get(record_ix as usize) {
                        return Ok(rhai::Dynamic::from(val.to_owned()));
                    }
                }
            }
        }

        Err("Error getting record field".into())
    }
}
