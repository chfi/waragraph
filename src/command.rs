//! Command palette system & features

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{anyhow, bail, Result};

use euclid::Length;
use parking_lot::RwLock;
use raving::compositor::Compositor;
use rhai::plugin::*;

use crate::{
    geometry::{
        LayoutElement, ListLayout, ScreenPoint, ScreenRect, ScreenSideOffsets,
    },
    text::TextCache,
    viewer::gui::layer::rect_rgba,
};

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
        let mut commands: HashMap<rhai::ImmutableString, Arc<Command>> =
            HashMap::default();

        for (key, val) in self.commands {
            /*
            if let Some(map) = val.try_cast::<rhai::Map>() {
            }
            */

            if let Some(fn_ptr) = val.try_cast::<rhai::FnPtr>() {
                commands.insert(key.into(), Arc::new(Command::new(fn_ptr)));
            }
        }

        Ok(CommandModule {
            name: self.name,
            desc: self.desc,

            commands,

            fn_ptr_ast: ast,

            source_path,
            // list_view: ListView::new(None),
        })
    }
}

// pub enum ResultType {
//     Value { output: rhai::ImmutableString },
//     // Command { command: Arc<Command>, output: rhai::ImmutableString },
//     Command { output: rhai::Immutable
// }

#[derive(Debug, Clone)]
pub enum ResultProducer {
    Value(rhai::Dynamic),
    Command {
        module: rhai::ImmutableString,
        command: rhai::ImmutableString,
    },
    // Command(Arc<Command>),
}

// pub struct CommandOutput
#[derive(Debug, Clone)]
pub struct ResultItem {
    text: rhai::ImmutableString,

    ty: rhai::ImmutableString,

    item: ResultProducer,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub fn_ptr: rhai::FnPtr,
    pub inputs: HashMap<rhai::ImmutableString, rhai::ImmutableString>,
    pub output: Option<rhai::ImmutableString>,
}

impl Command {
    pub fn new(fn_ptr: rhai::FnPtr) -> Self {
        Self {
            fn_ptr,
            inputs: HashMap::default(),
            output: None,
        }
    }
}

pub struct CommandModule {
    pub name: rhai::ImmutableString,
    pub desc: rhai::ImmutableString,

    commands: HashMap<rhai::ImmutableString, Arc<Command>>,

    fn_ptr_ast: Arc<rhai::AST>,

    source_path: PathBuf,
    // source_filename: rhai::ImmutableString,
    // results: Vec<ResultProducer>,
    // list_view: ListView<ResultItem>,
}

pub enum CommandInput {
    MoveDown,
    MoveUp,
    Select,
    Clear,
}

pub struct CommandPalette {
    // input_history: Vec<String>,
    // output_history: Vec<rhai::Dynamic>,

    // stack: Vec<rhai::Dynamic>,
    pub input_buffer: String,

    modules: HashMap<rhai::ImmutableString, CommandModule>,

    offset: ScreenPoint,

    rect: ScreenRect,
    layout: ListLayout,
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

                        if name == "->" {
                            cmd.output = Some(ty.trim().into());
                        } else {
                            cmd.inputs.insert(name.into(), ty.trim().into());
                        }
                    } else {
                        desc.push_str(rest.trim());
                        desc.push_str("\n");
                    }
                }
            }

            let desc = rhai::ImmutableString::from(desc);

            log::warn!("INPUTS: {:?}", cmd.inputs);
            log::warn!("desc: {}", desc);

            log::error!("Command: {:#?}", cmd);

            module.commands.insert(f.name.into(), Arc::new(cmd));

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
        let bg_rect = euclid::rect(80.0, 80.0, 500.0, 500.0);

        let [top, bottom] = bg_rect.split_ver(bg_rect.height() * 0.15);

        let pad = ScreenSideOffsets::new(8.0, 8.0, 8.0, 8.0);

        let layout = ListLayout {
            origin: bottom.origin,
            size: bottom.size,
            side_offsets: Some(pad),
            slot_height: Length::new(60.0),
        };

        Self {
            input_buffer: String::new(),
            modules: HashMap::new(),

            offset: ScreenPoint::new(100.0, 100.0),

            layout,

            rect: bg_rect,
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

    pub fn window_rect(&self) -> ScreenRect {
        self.rect
    }

    // TODO
    pub fn list_rect(&self) -> ScreenRect {
        let rect = self.layout.inner_rect();
        // let bg_rect = euclid::rect(80.0, 80.0, 500.0, 500.0);
        rect
    }

    pub fn queue_glyphs(&self, text_cache: &mut TextCache) -> Result<()> {
        use glyph_brush::{Section, Text};

        // let input_rect = euclid::rect(self.offset.x

        let window = self.window_rect();

        let [top, bottom] = window.split_ver(window.height() * 0.15);

        let pad = ScreenSideOffsets::new(16.0, 8.0, 8.0, 8.0);

        let top = top.inner_rect(pad);
        let bottom = bottom.inner_rect(pad);

        let input_scale = 24.0;

        let input_text = Text::new(&self.input_buffer).with_scale(input_scale);

        let rect = top.inner_rect(pad);

        let pos = (rect.min_x(), rect.min_y());

        let section = Section::default()
            .with_screen_position(pos)
            .add_text(input_text);

        text_cache.queue(section);

        Ok(())
    }

    pub fn update_layer(
        &self,
        compositor: &mut Compositor,
        layer_name: &str,
        rect_sublayer: &str,
        line_sublayer: &str,
    ) -> Result<()> {
        compositor.with_layer(layer_name, |layer| {
            if let Some(sublayer_data) = layer
                .get_sublayer_mut(line_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                //
            }

            if let Some(sublayer_data) = layer
                .get_sublayer_mut(rect_sublayer)
                .and_then(|s| s.draw_data_mut().next())
            {
                let bg_rect = self.window_rect();

                let color_bg = rgb::RGBA::new(0.6, 0.6, 0.6, 1.0);
                let color_fg = rgb::RGBA::new(0.75, 0.75, 0.75, 1.0);

                let pad = ScreenSideOffsets::new(8.0, 8.0, 8.0, 8.0);

                let [top, bottom] = bg_rect.split_ver(bg_rect.height() * 0.15);

                let base = vec![
                    rect_rgba(bg_rect, color_bg),
                    rect_rgba(top.inner_rect(pad), color_fg),
                    rect_rgba(bottom.inner_rect(pad), color_fg),
                ];

                sublayer_data.update_vertices_array(base)?;
                // .update_vertices_array(Some(rect_rgba(bg_rect, color)))?;
            }

            Ok(())
        })?;

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
