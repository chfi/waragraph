//! Command palette system & features

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{anyhow, bail, Result};

use parking_lot::RwLock;
use raving::compositor::Compositor;
use rhai::plugin::*;

use crate::text::TextCache;

#[derive(Default, Clone)]
pub struct CommandModuleBuilder {
    pub name: rhai::ImmutableString,
    pub desc: rhai::ImmutableString,

    commands: rhai::Map,
    // commands: HashMap<rhai::ImmutableString,
}

impl CommandModuleBuilder {
    pub fn build(
        self,
        ast: Arc<rhai::AST>,
        source_path: PathBuf,
    ) -> Result<CommandModule> {
        let mut commands: HashMap<rhai::ImmutableString, Command> =
            HashMap::default();

        for (key, val) in self.commands {
            /*
            if let Some(map) = val.try_cast::<rhai::Map>() {
            }
            */

            if let Some(fn_ptr) = val.try_cast::<rhai::FnPtr>() {
                commands.insert(key.into(), Command::new(fn_ptr));
            }
        }

        Ok(CommandModule {
            name: self.name,
            desc: self.desc,

            commands,

            fn_ptr_ast: ast,

            source_path,
        })
    }
}

#[derive(Clone)]
pub struct Command {
    pub fn_ptr: rhai::FnPtr,
    pub inputs: HashMap<rhai::ImmutableString, rhai::ImmutableString>,
}

impl Command {
    pub fn new(fn_ptr: rhai::FnPtr) -> Self {
        Self {
            fn_ptr,
            inputs: HashMap::default(),
        }
    }
}

pub struct CommandModule {
    pub name: rhai::ImmutableString,
    pub desc: rhai::ImmutableString,

    commands: HashMap<rhai::ImmutableString, Command>,

    fn_ptr_ast: Arc<rhai::AST>,

    source_path: PathBuf,
    // source_filename: rhai::ImmutableString,
}

pub struct CommandPalette {
    // input_history: Vec<String>,
    // output_history: Vec<rhai::Dynamic>,

    // stack: Vec<rhai::Dynamic>,
    input_buffer: String,

    modules: HashMap<rhai::ImmutableString, CommandModule>,
}

impl CommandPalette {
    pub fn load_rhai_module(
        &mut self,
        mut engine: rhai::Engine,
        path: &str,
    ) -> Result<()> {
        // engine.register_global_module(RHAI_MODULE.clone());

        let source_path: PathBuf = path.into();

        let builder = Arc::new(RwLock::new(CommandModuleBuilder::default()));

        let ast = {
            let mut engine = engine;

            let b = builder.clone();
            engine.register_fn("set_name", move |name: &str| {
                let mut b = b.write();
                b.name = name.into();
            });

            let b = builder.clone();
            engine.register_fn("set_desc", move |desc: &str| {
                let mut b = b.write();
                b.desc = desc.into();
            });
            let b = builder.clone();
            engine.register_fn(
                "add_command",
                move |name: &str, desc: &str, fn_ptr: rhai::FnPtr| {
                    let mut b = b.write();
                    b.commands.insert(name.into(), fn_ptr.into());
                },
            );

            let ast = engine.compile_file(source_path.clone())?;

            let _: () = engine.eval_ast(&ast)?;

            ast
        };

        let builder = Arc::try_unwrap(builder)
            .map_err(|_| anyhow!("Builder still shared!"))?
            .into_inner();

        let ast = ast.clone_functions_only_filtered(
            |ns, acc, global, name, arity| {
                log::error!(
                    "{:?}\t{:?}\t{:?}\t{:?}\t{:?}",
                    ns,
                    acc,
                    global,
                    name,
                    arity
                );
                true
            },
        );
        let ast = Arc::new(ast);

        let mut module = builder.build(ast.clone(), source_path)?;

        for f in ast.iter_functions() {
            if f.name.starts_with("anon$") {
                // log::warn!("skipping anon");
                continue;
            }

            let mut desc = String::new();

            log::warn!("f.name: {}", f.name);
            let fn_ptr = rhai::FnPtr::new(f.name.clone())?;

            let mut cmd = Command::new(fn_ptr);

            for line in f.comments.iter() {
                if let Some(rest) = line.strip_prefix("///") {
                    log::warn!("COMMENT: {}", rest);
                    let rest = rest.trim();

                    if let Some(rest) = rest.strip_prefix("@") {
                        log::warn!("STRIP: {}", rest);
                        let mut fields = rest.split(":");

                        let name = fields.next().unwrap();
                        let ty = fields.next().unwrap();

                        cmd.inputs.insert(name.into(), ty.trim().into());
                    } else {
                        desc.push_str(rest.trim());
                        desc.push_str("\n");
                    }
                }
            }

            let desc = rhai::ImmutableString::from(desc);

            log::warn!("INPUTS: {:?}", cmd.inputs);
            log::warn!("desc: {}", desc);

            module.commands.insert(f.name.into(), cmd);

            log::warn!(" >>>> inserted {}", f.name);
        }

        log::warn!(
            "loaded module `{}` with {} commands",
            module.name,
            module.commands.len()
        );

        self.modules.insert(module.name.clone(), module);

        Ok(())
    }

    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            modules: HashMap::new(),
        }
    }

    pub fn run_command(
        &self,
        engine: &rhai::Engine,
        module: &str,
        cmd: &str,
    ) -> Result<()> {
        if let Some(module) = self.modules.get(module) {
            if let Some(cmd) = module.commands.get(cmd) {
                let res: rhai::Dynamic =
                    cmd.fn_ptr.call(engine, &module.fn_ptr_ast, ())?;
                log::error!("command result: {:?}", res);
            } else {
                bail!("Unknown command `{}:{}`", module.name, cmd);
            }
        } else {
            bail!("Unknown module `{}`", module);
        }

        Ok(())
    }

    pub fn queue_glyphs(&self, text_cache: &mut TextCache) -> Result<()> {
        todo!();
    }

    pub fn update_layer(
        &self,
        compositor: &mut Compositor,
        layer_name: &str,
        rect_sublayer: &str,
        line_sublayer: &str,
    ) -> Result<()> {
        compositor.with_layer(layer_name, |layer| {
            // if let Some(sublayer_data) = layer

            Ok(())
        });

        Ok(())
    }
}

/*
lazy_static::lazy_static! {
    static ref RHAI_MODULE: Arc<rhai::Module> = {
        let mut module = rhai::exported_module!(rhai_module);
        Arc::new(module)
    };
}

#[export_module]
pub mod rhai_module {
}
*/
