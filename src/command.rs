//! Command palette system & features

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{anyhow, bail, Result};

use rhai::plugin::*;

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
        // let filename = source_path.fi

        let x = source_path;

        todo!();
    }
}

pub struct CommandModule {
    pub name: rhai::ImmutableString,
    pub desc: rhai::ImmutableString,

    commands: HashMap<rhai::ImmutableString, rhai::FnPtr>,
    fn_ptr_ast: Arc<rhai::AST>,

    source_path: PathBuf,
    source_filename: rhai::ImmutableString,
}

pub struct CommandManager {
    modules: HashMap<rhai::ImmutableString, CommandModule>,
}

pub struct CommandPalette {
    input_history: Vec<String>,
    output_history: Vec<rhai::Dynamic>,

    stack: Vec<rhai::Dynamic>,

    input_buffer: String,
}

impl CommandPalette {
    pub fn new() -> Self {
        //

        todo!();
    }

    pub fn run_command(&self, input: &str) -> Result<()> {
        //
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
    static ref RHAI_MODULE: Arc<rhai::Module> {
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

    // pub fn command_module(name: &str, desc: &str) -> CommandModuleBuilder {
    //     CommandModuleBuilder {
    //         name: name.into(),
    //         desc: desc.into(),

    //         commands: rhai::Map::default(),
    //     }
    // }

    pub fn add_command(
        builder: &mut CommandModuleBuilder,
        cmd_name: &str,
        cmd_desc: &str,
        cmd_fn: rhai::FnPtr,
    ) {
        let mut obj = rhai::Map::default();

        obj.insert("name".into(), rhai::Dynamic::from(cmd_name));
        obj.insert("desc".into(), rhai::Dynamic::from(cmd_desc));
        obj.insert("fn".into(), rhai::Dynamic::from(cmd_fn));

        builder.commands.insert(cmd_name.into(), obj.into());
    }
}
