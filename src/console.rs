use std::collections::HashMap;

use bstr::ByteSlice;
use raving::{
    compositor::{label_space::LabelSpace, Compositor, SublayerAllocMsg},
    script::console::BatchBuilder,
    vk::{GpuResources, VkEngine},
};

use sled::IVec;
use smartstring::SmartString;

use zerocopy::{AsBytes, FromBytes};

use std::sync::Arc;

use ndarray::prelude::*;

use anyhow::{anyhow, bail, Result};

use bstr::ByteSlice as BstrByteSlice;

use crate::util::{BufFmt, BufId, BufMeta, BufferStorage};

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

pub type EvalResult<T> = Result<T, Box<rhai::EvalAltResult>>;

#[derive(Clone)]
pub struct Console {
    pub scope: rhai::Scope<'static>,

    pub modules: HashMap<rhai::ImmutableString, Arc<rhai::Module>>,

    pub ast: Arc<rhai::AST>,

    focus: usize,
    pub input: String,
    input_history: Vec<rhai::ImmutableString>,
    output: Vec<SmartString<smartstring::Compact>>,

    label_space: LabelSpace,
}

impl Console {
    pub const LAYER_NAME: &'static str = "console";
    pub const RECT_SUBLAYER_NAME: &'static str = "sublayer-rect";
    pub const TEXT_SUBLAYER_NAME: &'static str = "sublayer-text";

    pub fn init(
        engine: &mut VkEngine,
        compositor: &mut Compositor,
    ) -> Result<Self> {
        let label_space = LabelSpace::new(engine, "console", 1024 * 1024)?;

        compositor.new_layer(Self::LAYER_NAME, 20, true);

        compositor.sublayer_alloc_tx.send(SublayerAllocMsg::new(
            Self::LAYER_NAME,
            Self::RECT_SUBLAYER_NAME,
            "rect-rgb",
            &[],
        ))?;

        compositor.sublayer_alloc_tx.send(SublayerAllocMsg::new(
            Self::LAYER_NAME,
            Self::TEXT_SUBLAYER_NAME,
            "text",
            &[label_space.text_set],
        ))?;

        Ok(Self {
            scope: rhai::Scope::default(),
            modules: HashMap::default(),
            ast: Arc::new(rhai::AST::default()),

            focus: 0,
            input: String::new(),
            input_history: Vec::new(),
            output: Vec::new(),

            label_space,
        })
    }

    pub fn update_layer(
        &mut self,
        res: &mut GpuResources,
        compositor: &mut Compositor,
        pos: [f32; 2],
    ) -> Result<()> {
        if self.label_space.used_bytes() > self.label_space.capacity() / 2 {
            self.label_space.clear();
        }

        let (text_s, text_l) =
            self.label_space.bounds_for_insert(&self.input)?;

        let _ = self.label_space.write_buffer(res);

        compositor.with_layer(Self::LAYER_NAME, |layer| {
            let [x, y] = pos;
            let [width, _] = compositor.window_dims();

            if let Some(sublayer) =
                layer.get_sublayer_mut(Self::TEXT_SUBLAYER_NAME)
            {
                let color = [0.0f32, 0.0, 0.0, 1.0];

                let mut out = [0u8; 8 + 8 + 16];
                out[0..8].clone_from_slice([x + 1.0, y + 1.0].as_bytes());
                out[8..16].clone_from_slice(
                    [text_s as u32, text_l as u32].as_bytes(),
                );
                out[16..32].clone_from_slice(color.as_bytes());

                sublayer.draw_data_mut().try_for_each(|data| {
                    data.update_vertices_array(Some(out))
                })?;
            }

            if let Some(sublayer) =
                layer.get_sublayer_mut(Self::RECT_SUBLAYER_NAME)
            {
                let mut bg = [0u8; 8 + 8 + 16];
                bg[0..8].clone_from_slice([x, y].as_bytes());
                bg[8..16].clone_from_slice([width as f32, 10.0].as_bytes());
                bg[16..32]
                    .clone_from_slice([0.85f32, 0.85, 0.85, 1.0].as_bytes());

                sublayer.draw_data_mut().try_for_each(|data| {
                    data.update_vertices_array_range(0..1, [bg])
                })?;
            }

            Ok(())
        })?;

        Ok(())
    }

    pub fn call_fn(
        &mut self,
        db: &sled::Db,
        buffers: &BufferStorage,
        // ast: &rhai::AST,
        fn_name: &str,
        this_ptr: Option<&mut rhai::Dynamic>,
        args: impl AsMut<[rhai::Dynamic]>,
    ) -> Result<rhai::Dynamic> {
        let mut engine = create_engine(db, buffers);

        for (name, module) in self.modules.iter() {
            engine.register_static_module(name, module.clone());
        }

        let result = engine.call_fn_raw(
            &mut self.scope,
            &self.ast,
            true,
            false,
            fn_name,
            this_ptr,
            args,
        )?;

        Ok(result)
    }

    pub fn eval(
        &mut self,
        db: &sled::Db,
        buffers: &BufferStorage,
        input: &str,
    ) -> Result<rhai::Dynamic> {
        let ast = Arc::make_mut(&mut self.ast);
        eval_scope_ast(ast, &mut self.scope, &self.modules, db, buffers, input)
    }

    pub fn eval_file<P: AsRef<std::path::Path>>(
        &mut self,
        db: &sled::Db,
        buffers: &BufferStorage,
        path: P,
    ) -> Result<rhai::Dynamic> {
        let ast = Arc::make_mut(&mut self.ast);
        eval_scope_ast_file(
            ast,
            &mut self.scope,
            &self.modules,
            db,
            buffers,
            path.as_ref(),
        )
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

        // txt.set_text_for(b"console", &self.input)?;

        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConsoleInput {
    AppendChar(char),
    Submit,
    Backspace,
    Left,
    Right,
    Delete,
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

    engine.register_global_module(
        crate::viewer::app::PATH_UI_STATE_MODULE.clone(),
    );

    // utility functions used in color.rhai
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

pub fn eval_scope_ast_file<T: Clone + Send + Sync + 'static>(
    old_ast: &mut rhai::AST,
    scope: &mut rhai::Scope,
    modules: &HashMap<rhai::ImmutableString, Arc<rhai::Module>>,
    db: &sled::Db,
    buffers: &BufferStorage,
    script_path: &std::path::Path,
) -> Result<T> {
    let mut engine = create_engine(db, buffers);

    for (name, module) in modules.iter() {
        engine.register_static_module(name, module.clone());
    }

    let new_ast = engine.compile_file_with_scope(scope, script_path.into())?;
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
