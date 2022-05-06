use std::{collections::HashMap, num::NonZeroU32, path::PathBuf};

use ash::vk;
use bstr::ByteSlice;
use gfa::gfa::GFA;
use gpu_allocator::vulkan::Allocator;
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
    util::{BufFmt, BufId, BufMeta, BufferStorage, LabelStorage},
    viewer::ViewDiscrete1D,
};

use lazy_static::lazy_static;

pub mod data;
pub mod layout;
pub mod math;
pub mod view;

// use lazy_static so that modules only have to be loaded once
lazy_static! {
    static ref CONFIG_MODULE: Arc<rhai::Module> = {
        let module = rhai::exported_module!(crate::config::config);
        Arc::new(module)
    };
    static ref KEY_MODULE: Arc<rhai::Module> = {
        let module = crate::input::create_key_module();
        Arc::new(module)
    };
    static ref MOUSE_MODULE: Arc<rhai::Module> = {
        let module = crate::input::create_mouse_module();
        Arc::new(module)
    };
    static ref VIEW_MODULE: Arc<rhai::Module> = {
        let module = rhai::exported_module!(view::rhai_module);
        Arc::new(module)
    };
}

// pub mod ivec;
// pub mod labels;

pub type EvalResult<T> = Result<T, Box<rhai::EvalAltResult>>;

#[derive(Default, Clone)]
pub struct Console {
    pub input: String,
    focus: usize,

    pub scope: rhai::Scope<'static>,

    pub modules: HashMap<rhai::ImmutableString, Arc<rhai::Module>>,

    pub ast: Arc<rhai::AST>,
}

impl Console {
    /*
    pub fn handle_input(
        &mut self,
        input: &winit::event::KeyboardInput,
    ) -> Result<()> {
        // winit::event::ElementState::
        // input.state
        let pressed =
            matches!(input.state, winit::event::ElementState::Pressed);

        if let Some(vk) = input.virtual_keycode {
            match vk {}
        }
    }
    */

    pub fn eval(
        &mut self,
        db: &sled::Db,
        buffers: &BufferStorage,
        input: &str,
    ) -> Result<rhai::Dynamic> {
        let ast = Arc::make_mut(&mut self.ast);
        eval_scope_ast(ast, &mut self.scope, &self.modules, db, buffers, input)
    }

    pub fn create_engine(
        &self,
        db: &sled::Db,
        buffers: &BufferStorage,
    ) -> rhai::Engine {
        let mut engine = create_engine(db, buffers);

        for (name, module) in self.modules.iter() {
            engine.register_static_module(name, module.clone());
        }

        engine
    }

    pub fn create_engine_fn(
        &self,
        db: &sled::Db,
        buffers: &BufferStorage,
    ) -> Arc<dyn Fn() -> rhai::Engine + Send + Sync + 'static> {
        let db = db.clone();
        let buffers = buffers.clone();
        let modules = self.modules.clone();

        Arc::new(move || {
            let mut engine = create_engine(&db, &buffers);

            for (name, module) in modules.iter() {
                engine.register_static_module(name, module.clone());
            }

            engine
        })
    }

    pub fn handle_input(
        &mut self,
        db: &sled::Db,
        buffers: &BufferStorage,
        txt: &LabelStorage,
        input: ConsoleInput,
    ) -> Result<()> {
        match input {
            ConsoleInput::AppendChar(c) => {
                self.input.insert(self.focus, c);
                self.focus += 1;
            }
            ConsoleInput::Submit => {
                let ast = Arc::make_mut(&mut self.ast);

                match eval_scope_ast::<rhai::Dynamic>(
                    ast,
                    &mut self.scope,
                    &self.modules,
                    db,
                    buffers,
                    &self.input,
                ) {
                    Ok(r) => {
                        let body = match r.type_name() {
                            // "string" => {
                            //     format!("{}", r.cast::<rhai::ImmutableString>())
                            // }
                            "waragraph::graph::Node" => format!(
                                "Node: {}",
                                r.cast::<crate::graph::Node>()
                            ),
                            "waragraph::graph::Path" => format!(
                                "Path: {}",
                                r.cast::<crate::graph::Path>().ix()
                            ),
                            "()" => "()".to_string(),
                            _ => format!("{}: {}", r.type_name(), r),
                        };
                        log::warn!("Console result: {}", body);
                    }
                    Err(e) => {
                        log::error!("Console error: {:?}", e);
                    }
                }
                self.input.clear();
                self.focus = 0;
            }
            ConsoleInput::Backspace => {
                if self.focus >= 1 {
                    self.focus -= 1;
                    self.input.remove(self.focus);
                }
            }
            ConsoleInput::Delete => {
                if self.focus < self.input.len() {
                    self.input.remove(self.focus);
                }
            }
            ConsoleInput::Left => {
                if self.focus > 0 {
                    self.focus -= 1;
                }
            }
            ConsoleInput::Right => {
                if self.focus < self.input.len() {
                    self.focus += 1;
                }
            }
        }

        txt.set_text_for(b"console", &self.input)?;

        Ok(())
    }
}

// enum ConsoleInput<'a> {
//     AppendStr(&'a str),
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConsoleInput {
    AppendChar(char),
    Submit,
    Backspace,
    Left,
    Right,
    Delete,
    // InsertChar(char),
    // Home,
    // End,
    // Endline,
}

pub fn register_buffer_storage(
    db: &sled::Db,
    buffers: &BufferStorage,
    engine: &mut rhai::Engine,
) {
    let buffers_ = buffers.clone();

    engine.register_result_fn("get_buffer", move |name: &str| {
        // 1. get the ID from the name

        // all of this should be either methods, in a transaction with
        // proper error handling, or both
        let name_key = BufferStorage::name_key(name);
        let tree = &buffers_.tree;

        let id = tree
            .get(&name_key)
            .ok()
            .flatten()
            .and_then(|id| BufId::read_from(id.as_ref()))
            .ok_or(rhai::EvalAltResult::from(format!(
                "buffer `{}` not found",
                name_key.as_bstr()
            )))?;

        // 3. get the set from the Arc<Vec<->> via the index
        let set = buffers_.get_desc_set_ix(id).unwrap();

        // 4. get the length via the buffer data, i guess
        let meta = BufMeta::get_stored(&tree, id).unwrap();

        let k_data = id.as_data_key();
        let data = tree.get(&k_data).ok().flatten();
        let len = data.map(|d| d.len() / meta.fmt.size()).unwrap_or_default();

        // return object map with length and storage set, at least
        let mut map = rhai::Map::default();
        map.insert("set".into(), rhai::Dynamic::from(set));
        map.insert("len".into(), rhai::Dynamic::from(len as i64));
        map.insert("id".into(), rhai::Dynamic::from(id));

        Ok(map)
    });

    // let alloc_queue = buffers.alloc_queue.clone();
    let alloc_id = buffers.allocated_id.clone();

    engine.register_type_with_name::<BufId>("BufId");

    engine.register_fn("is_ready", move |id: BufId| {
        let alloc = alloc_id.load();
        let id = id.0;
        alloc >= id
    });

    /*
    let alloc_queue = buffers.alloc_queue.clone();
    let alloc_id = buffers.allocated_id.clone();

    engine.register_result_fn("get", move |id: BufId| {
        if alloc_id.load().0 >= id.0 {
            Ok
        } else {
            Err("buffer not ready".into())
        }
    });
    */

    let alloc_queue = buffers.alloc_queue.clone();

    let db_ = db.clone();
    let buffers_ = buffers.clone();
    // TODO check if the name already exists here
    engine.register_result_fn(
        "allocate_vec4_buffer",
        move |name: &str, capacity: i64| {
            let fmt = BufFmt::FVec4;

            let id = buffers_
                .queue_allocate_buffer(&db_, name, fmt, capacity as usize)
                .map_err(|e| rhai::EvalAltResult::from(e.to_string()))?;

            Ok(id)
        },
    );

    // let db_ = db.clone();
    let buffers_ = buffers.clone();

    // TODO check if the name already exists here
    engine.register_result_fn(
        "fill_vec4_buffer",
        move |id: BufId, vals: Vec<rhai::Dynamic>| {
            let vals = vals
                .into_iter()
                .filter_map(|v| v.try_cast::<[f32; 4]>())
                .collect::<Vec<_>>();

            buffers_.insert_data(id, &vals).unwrap();

            Ok(())
        },
    );
}

pub fn create_engine(db: &sled::Db, buffers: &BufferStorage) -> rhai::Engine {
    let mut engine = raving::script::console::create_batch_engine();

    register_buffer_storage(db, buffers, &mut engine);
    append_to_engine(db, &mut engine);

    engine
}

// pub fn create_engine(db: &sled::Db) -> rhai::Engine {
pub fn append_to_engine(db: &sled::Db, engine: &mut rhai::Engine) {
    engine.register_static_module("config", CONFIG_MODULE.clone());
    engine.register_static_module("key", KEY_MODULE.clone());
    engine.register_static_module("mouse", MOUSE_MODULE.clone());
    engine.register_static_module("view", VIEW_MODULE.clone());

    // example of loading a rhai script as a console module
    /*
    let ast = engine.compile_file("util.rhai".into()).unwrap();

    let module =
        rhai::Module::eval_ast_as_new(rhai::Scope::new(), &ast, &engine)
            .unwrap();

    let module = Arc::new(module);

    engine.register_global_module(module.clone());
    */

    engine.register_fn("rgba", |r: f32, g: f32, b: f32, a: f32| [r, g, b, a]);
    engine.register_fn("rgba", |r: i64, g: i64, b: i64, a: i64| {
        let f = |v: i64| -> f32 { ((v as u8) as f32) / 255.0 };
        [f(r), f(g), f(b), f(a)]
    });

    engine.register_fn("rgba", |r: f32, g: f32, b: f32| [r, g, b, 1.0]);
    engine.register_fn("rgba", |r: i64, g: i64, b: i64| {
        let f = |v: i64| -> f32 { ((v as u8) as f32) / 255.0 };
        [f(r), f(g), f(b), 1.0]
    });

    engine.register_type_with_name::<IVec>("IVec");

    engine.register_fn("ivec", |len: i64| {
        let len = len as usize;
        IVec::from(vec![0u8; len])
    });

    engine.register_fn("to_blob", |v: &mut IVec| v.to_vec());

    engine.register_fn("print_vec", |v: &mut IVec| {
        if let Ok(string) = v.to_str() {
            log::error!("print: {}", string);
        }
    });

    engine.register_fn("write_u64", |v: &mut IVec, offset: i64, val: i64| {
        let val = val as u64;
        let o = offset as usize;
        if o + 8 <= v.len() {
            v[o..o + 8].clone_from_slice(&val.to_le_bytes());
        }
    });

    engine.register_fn(
        "write_u64s",
        |v: &mut IVec, offset: i64, vs: rhai::Array| {
            let mut offset = offset as usize;
            for val in vs {
                if let Some(i) = val.try_cast::<i64>() {
                    let i = i as u64;
                    v[offset..offset + 8].clone_from_slice(&i.to_le_bytes());
                    offset += 8;
                }
            }
        },
    );

    engine.register_fn(
        "write_ascii",
        |v: &mut IVec, offset: i64, txt: &str| {
            let offset = offset as usize;
            let bytes = txt.as_bytes();
            v[offset..offset + bytes.len()].clone_from_slice(bytes);
        },
    );

    engine.register_fn("to_ivec", |s: &str| IVec::from(s.as_bytes()));

    engine.register_result_fn(
        "subslice",
        |v: &mut IVec, offset: i64, len: i64| {
            let o = offset as usize;
            let l = len as usize;

            if o >= v.len() || o + l > v.len() {
                return Err("offset out of bounds".into());
            }

            Ok(v.subslice(o, l))
        },
    );

    engine.register_result_fn("as_u64", |v: &mut IVec| {
        u64::read_from(v.as_ref()).ok_or("bytestring is not u64".into())
    });

    engine.register_result_fn("as_u32", |v: &mut IVec| {
        u32::read_from(v.as_ref()).ok_or("bytestring is not u32".into())
    });

    let db_ = db.clone();
    engine.register_fn("exists", move |k: &str| {
        let k = k.as_bytes();
        db_.get(k).ok().flatten().is_some()
    });

    let db_ = db.clone();
    engine.register_fn("exists", move |k: &mut IVec| {
        db_.get(k).ok().flatten().is_some()
    });

    let db_ = db.clone();
    engine.register_result_fn("get", move |k: &str| {
        let k = k.as_bytes();
        if let Some(v) = db_.get(k).ok().flatten() {
            Ok(v)
        } else {
            Err("key not found".into())
        }
    });

    let db_ = db.clone();
    engine.register_result_fn("get", move |k: &mut IVec| {
        if let Some(v) = db_.get(k).ok().flatten() {
            Ok(v)
        } else {
            Err("key not found".into())
        }
    });

    let db_ = db.clone();
    engine.register_fn("set", move |k: &str, v: IVec| {
        // let k = k.as_bytes();
        db_.insert(k, v).unwrap();
        // let v = db_.get(k).unwrap().unwrap();
        // v
    });

    // let db_ = db.clone();
    // engine.register_fn("set", move |

    let db_ = db.clone();
    engine.register_fn("view", move || {
        let raw = db_.get(b"view").unwrap().unwrap();
        ViewDiscrete1D::from_bytes(&raw)
    });

    let db_ = db.clone();
    engine.register_fn("set_view_offset", move |new: i64| {
        let offset = new.abs() as usize;
        let raw = db_.get(b"view").unwrap().unwrap();
        let mut view = ViewDiscrete1D::from_bytes(&raw).unwrap();
        view.offset = offset.clamp(0, view.max() - view.len());
        log::warn!("new view offset: {}", view.offset);
        let bytes = view.as_bytes();
        db_.update_and_fetch(b"view", |_| Some(&bytes)).unwrap();
    });

    let db_ = db.clone();
    engine.register_fn("view_offset", move || {
        let raw = db_.get(b"view").unwrap().unwrap();
        let view = ViewDiscrete1D::from_bytes(&raw).unwrap();
        view.offset() as i64
    });
    // let tree =
}

pub fn eval<T: Clone + Send + Sync + 'static>(
    db: &sled::Db,
    buffers: &BufferStorage,
    script: &str,
) -> Result<T> {
    let engine = create_engine(db, buffers);
    match engine.eval(script) {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow!("eval err: {:?}", err)),
    }
}

pub fn eval_scope_ast<T: Clone + Send + Sync + 'static>(
    old_ast: &mut rhai::AST,
    scope: &mut rhai::Scope,
    modules: &HashMap<rhai::ImmutableString, Arc<rhai::Module>>,
    db: &sled::Db,
    buffers: &BufferStorage,
    script: &str,
) -> Result<T> {
    let mut engine = create_engine(db, buffers);

    for (name, module) in modules.iter() {
        engine.register_static_module(name, module.clone());
    }

    let new_ast = engine.compile_with_scope(scope, script)?;
    *old_ast = old_ast.merge(&new_ast);

    let result = engine.eval_ast_with_scope(scope, old_ast);

    old_ast.clear_statements();

    match result {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow!("eval err: {:?}", err)),
    }
}

pub fn eval_scope<T: Clone + Send + Sync + 'static>(
    scope: &mut rhai::Scope,
    modules: &HashMap<rhai::ImmutableString, Arc<rhai::Module>>,
    db: &sled::Db,
    buffers: &BufferStorage,
    script: &str,
) -> Result<T> {
    let mut engine = create_engine(db, buffers);

    for (name, module) in modules.iter() {
        engine.register_static_module(name, module.clone());
    }

    match engine.eval_with_scope(scope, script) {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow!("eval err: {:?}", err)),
    }
}

pub type RhaiBatchFn1<A> = Box<
    dyn Fn(A) -> Result<BatchBuilder, Box<rhai::EvalAltResult>> + Send + Sync,
>;
pub type RhaiBatchFn2<A, B> = Box<
    dyn Fn(A, B) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn3<A, B, C> = Box<
    dyn Fn(A, B, C) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn4<A, B, C, D> = Box<
    dyn Fn(A, B, C, D) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
pub type RhaiBatchFn5<A, B, C, D, E> = Box<
    dyn Fn(A, B, C, D, E) -> Result<BatchBuilder, Box<rhai::EvalAltResult>>
        + Send
        + Sync,
>;
