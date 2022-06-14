//! Command palette system & features

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{anyhow, bail, Result};

use raving::compositor::Compositor;
use rhai::plugin::*;

use crate::text::TextCache;

#[derive(Clone)]
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
        let mut commands: HashMap<rhai::ImmutableString, rhai::FnPtr> =
            HashMap::default();

        for (key, val) in self.commands {
            /*
            if let Some(map) = val.try_cast::<rhai::Map>() {
            }
            */

            if let Some(fn_ptr) = val.try_cast::<rhai::FnPtr>() {
                commands.insert(key.into(), fn_ptr);
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

pub struct CommandModule {
    pub name: rhai::ImmutableString,
    pub desc: rhai::ImmutableString,

    commands: HashMap<rhai::ImmutableString, rhai::FnPtr>,
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
        engine.register_global_module(RHAI_MODULE.clone());

        /*
        engine.register_fn("command_module", |name: &str, desc: &str| {
            CommandModuleBuilder {
                name: name.into(),
                desc: desc.into(),

                commands: rhai::Map::default(),
            }
        });

        engine.register_fn(
            "add_command",
            |builder: &mut CommandModuleBuilder,
             cmd_name: &str,
             cmd_desc: &str,
             cmd_fn: rhai::FnPtr| {
                builder.commands.insert(cmd_name.into(), cmd_fn.into());
            },
        );
        */

        let source_path: PathBuf = path.into();
        let ast = engine.compile_file(source_path.clone())?;

        // let builder = engine.eval_ast_with_scope(ast)
        let builder: CommandModuleBuilder = engine.eval_ast(&ast)?;

        let ast = Arc::new(ast);

        let module = builder.build(ast, source_path)?;

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
            if let Some(fn_ptr) = module.commands.get(cmd) {
                let res: rhai::Dynamic =
                    fn_ptr.call(engine, &module.fn_ptr_ast, ())?;
                log::error!("command result: {:?}", res);
            }
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

lazy_static::lazy_static! {
    static ref RHAI_MODULE: Arc<rhai::Module> = {
        Arc::new(create_rhai_module())
    };
}

pub fn create_rhai_module() -> rhai::Module {
    let mut module = rhai::exported_module!(rhai_module);

    //

    module
}

#[export_module]
pub mod rhai_module {

    pub type CommandModuleBuilder = super::CommandModuleBuilder;

    #[rhai_fn(global)]
    pub fn command_module(name: &str, desc: &str) -> CommandModuleBuilder {
        CommandModuleBuilder {
            name: name.into(),
            desc: desc.into(),

            commands: rhai::Map::default(),
        }
    }

    #[rhai_fn(global)]
    pub fn add_command(
        builder: &mut CommandModuleBuilder,
        cmd_name: &str,
        cmd_desc: &str,
        cmd_fn: rhai::FnPtr,
    ) {
        /*
        let mut obj = rhai::Map::default();

        obj.insert("name".into(), rhai::Dynamic::from(cmd_name));
        obj.insert("desc".into(), rhai::Dynamic::from(cmd_desc));
        obj.insert("fn".into(), rhai::Dynamic::from(cmd_fn));

        builder.commands.insert(cmd_name.into(), obj.into());
        */

        builder.commands.insert(cmd_name.into(), cmd_fn.into());
    }
}
