//! Command palette system & features

use std::{
    cell::RefMut,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{anyhow, bail, Result};

use crossbeam::atomic::AtomicCell;
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

#[derive(Default)]
struct FilePathPromptConfig {
    predicate: Option<Box<dyn Fn(&std::path::Path) -> bool>>,
    current_dir: PathBuf,
}

impl FilePathPromptConfig {
    pub fn new(current_dir: Option<PathBuf>) -> Result<Self> {
        let current_dir = if let Some(dir) = current_dir {
            dir
        } else {
            std::env::current_dir()?
        };

        Ok(Self {
            predicate: None,
            current_dir,
        })
    }

    pub fn from_ext_whitelist<'a>(
        current_dir: Option<PathBuf>,
        ext_whitelist: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self> {
        use std::ffi::OsString;

        let current_dir = if let Some(dir) = current_dir {
            dir
        } else {
            std::env::current_dir()?
        };

        let whitelist = {
            let set = ext_whitelist
                .into_iter()
                .map(OsString::from)
                .collect::<HashSet<_>>();

            (!set.is_empty()).then(|| set)
        };

        if let Some(whitelist) = whitelist {
            let predicate = Box::new(move |path: &std::path::Path| {
                if path.is_dir() {
                    return true;
                }

                if let Some(ext) = path.extension() {
                    whitelist.contains(ext)
                } else {
                    true
                }
            });

            Ok(Self {
                predicate: Some(predicate),
                current_dir,
            })
        } else {
            Ok(Self {
                predicate: None,
                current_dir,
            })
        }
    }
}

type FilePathPrompt<I> = PromptObjectRaw<PathBuf, I, PathBuf, PathBuf>;

// pub struct FilePathPrompt {
// }

struct PromptObjectRaw<V, I, P, T>
where
    V: Clone + 'static,
    P: Clone,
    T: Clone,
    I: Iterator<Item = V>,
{
    prompt_input: P,
    select: Box<dyn Fn(&V) -> PromptAction<P, T> + 'static>,
    display: Box<dyn Fn(&V, ScreenRect) -> glyph_brush::OwnedSection + 'static>,
    update_choices: Box<dyn FnMut(P) -> I + 'static>,
    // done: impl Fn(T) + 'static,
}

impl<V, I, P, T> PromptObjectRaw<V, I, P, T>
where
    V: Clone + 'static,
    P: Clone,
    T: Clone,
    I: Iterator<Item = V>,
{
    //
}

struct PromptObject {
    act: Box<dyn FnMut(usize) -> std::ops::ControlFlow<(), ()>>,
    show: Box<dyn FnMut(usize, ScreenRect) -> glyph_brush::OwnedSection>,

    result_len: std::rc::Rc<AtomicCell<usize>>,
}

impl PromptObject {
    fn new<V, I, P, T>(
        init: P,
        act: impl Fn(&V) -> PromptAction<P, T> + 'static,
        show: impl Fn(&V, ScreenRect) -> glyph_brush::OwnedSection + 'static,
        mut update_choices: impl FnMut(P) -> I + 'static,
        done: impl Fn(T) + 'static,
    ) -> PromptObject
    where
        V: Clone + 'static,
        P: Clone,
        T: Clone,
        I: Iterator<Item = V>,
    {
        use std::cell::RefCell;
        use std::rc::Rc;

        let results = update_choices(init).collect::<Vec<_>>();

        let result_len = Rc::new(AtomicCell::new(results.len()));
        let results = Rc::new(RefCell::new(results));

        let res = results.clone();
        // todo add cache here
        let show = Box::new(move |ix: usize, rect: ScreenRect| {
            //
            let res = res.borrow();
            let section = show(&res[ix], rect);
            section
        });

        let res = results.clone();
        let len = result_len.clone();
        let act = Box::new(move |ix: usize| {
            let results = res.borrow();

            match act(&results[ix]) {
                PromptAction::PromptFor(prompt) => {
                    std::mem::drop(results);

                    let mut results = res.borrow_mut();
                    results.clear();
                    results.extend(update_choices(prompt));
                    len.store(results.len());

                    std::ops::ControlFlow::Continue(())
                }
                PromptAction::Return(value) => {
                    std::mem::drop(results);

                    done(value);

                    std::ops::ControlFlow::Break(())
                }
            }
        });

        Self {
            act,
            show,
            result_len,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PromptAction<P, T> {
    PromptFor(P),
    Return(T),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CmdArg<T> {
    name: rhai::ImmutableString,

    ty_name: rhai::ImmutableString,

    data: T,
}

struct PromptCommandState {
    result_ty: Option<rhai::ImmutableString>,
}

struct PromptArgumentState {
    // (module name, command)
    module: rhai::ImmutableString,
    command: Arc<Command>,

    remaining_args: Vec<CmdArg<()>>,
    applied_args: Vec<CmdArg<rhai::Dynamic>>,
}

enum PromptState {
    Command(PromptCommandState),
    Argument(PromptArgumentState),
}

// struct FilePickerResults {
//     working_dir: PathBuf,
//     included_exts: Option<HashSet<String>>,
// }

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
                let cmd = Arc::new(Command::new(key.as_str(), fn_ptr));
                commands.insert(key.into(), cmd);
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
    // Value(rhai::Dynamic),
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
    pub name: rhai::ImmutableString,

    // pub desc: rhai::ImmutableString,
    pub fn_ptr: rhai::FnPtr,
    pub inputs: HashMap<rhai::ImmutableString, rhai::ImmutableString>,
    pub output: Option<rhai::ImmutableString>,
}

impl Command {
    pub fn new(name: &str, fn_ptr: rhai::FnPtr) -> Self {
        Self {
            name: name.into(),
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

struct CurriedCommand {
    module: rhai::ImmutableString,
    command: Arc<Command>,

    remaining_args: Vec<(rhai::ImmutableString, std::any::TypeId)>,

    args: HashMap<rhai::ImmutableString, rhai::Dynamic>,
}

#[derive(Clone)]
pub enum PromptFor {
    Command,
    Argument { ty: rhai::ImmutableString },
}

pub struct CommandPalette {
    // input_history: Vec<String>,
    // output_history: Vec<rhai::Dynamic>,
    // prompt_state: Option<PromptState>,

    // stack: Vec<rhai::Dynamic>,
    selection_focus: Option<usize>,

    pub input_buffer: String,

    // results: Vec<(rhai::ImmutableString, Arc<Command>)>,
    results: Vec<ResultItem>,
    // result_texts: Vec<rhai::ImmutableString>,
    modules: HashMap<rhai::ImmutableString, CommandModule>,

    offset: ScreenPoint,

    rect: ScreenRect,
    layout: ListLayout,
}

impl CommandPalette {
    pub fn handle_input(
        &mut self,
        mut engine: rhai::Engine,
        event: &winit::event::WindowEvent,
    ) -> Result<()> {
        use winit::event::{KeyboardInput, VirtualKeyCode as VK, WindowEvent};

        match event {
            WindowEvent::ReceivedCharacter(c) => {
                if !c.is_ascii_control() && c.is_ascii() {
                    self.input_buffer.push(*c);
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                //
            }
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(key) = input.virtual_keycode {
                    let pressed =
                        input.state == winit::event::ElementState::Pressed;

                    match key {
                        VK::Up => {
                            // Move result selection up
                            if pressed {
                                if let Some(focus) =
                                    self.selection_focus.as_mut()
                                {
                                    if *focus > 0 {
                                        *focus -= 1;
                                    }
                                } else {
                                    self.selection_focus =
                                        Some(self.results.len() - 1);
                                }
                            }
                        }
                        VK::Down => {
                            // Move result selection down
                            if pressed {
                                if let Some(focus) =
                                    self.selection_focus.as_mut()
                                {
                                    if *focus < self.results.len() - 1 {
                                        *focus += 1;
                                    }
                                } else {
                                    self.selection_focus = Some(0);
                                }
                            }
                        }
                        VK::Left => {
                            // Move input focus left
                            // TODO
                        }
                        VK::Right => {
                            // Move input focus right
                            // TODO
                        }
                        VK::Return => {
                            // Confirm selection
                            // TODO
                            // if let Some(ix) = self.selection_focus.take()
                            if let Some(item) = self
                                .selection_focus
                                .take()
                                .and_then(|ix| self.results.get(ix))
                            {
                                let ResultProducer::Command { module, command } =
                                    &item.item;
                                self.run_command(&engine, &module, &command)?;
                            }
                        }
                        VK::Tab => {
                            // Autocomplete
                            // TODO
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }

        Ok(())
        //
    }

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

            let mut cmd = Command::new(f.name, fn_ptr);

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

    pub fn build_results(&mut self) {
        // self.result_texts.clear();

        let mut results = Vec::new();

        for (mod_name, module) in self.modules.iter() {
            for (cmd_name, cmd) in &module.commands {
                let result = ResultItem {
                    text: cmd_name.clone(),
                    ty: cmd.output.clone().unwrap_or_default(),
                    item: ResultProducer::Command {
                        module: mod_name.clone(),
                        command: cmd_name.clone(),
                    },
                };

                // results.push((&module.name, cmd));
                results.push(result);
            }
        }

        results.sort_by(|c0, c1| c0.text.cmp(&c1.text));

        self.results.clear();
        self.results.extend(results.into_iter());
    }

    pub fn new() -> Self {
        let bg_rect = euclid::rect(80.0, 80.0, 500.0, 500.0);

        let [top, bottom] = bg_rect.split_ver(bg_rect.height() * 0.15);

        let pad = ScreenSideOffsets::new(8.0, 8.0, 8.0, 8.0);

        let layout = ListLayout {
            origin: bottom.origin,
            size: bottom.size,
            side_offsets: Some(pad),
            slot_height: Length::new(30.0),
        };

        Self {
            input_buffer: String::new(),
            modules: HashMap::new(),

            selection_focus: None,

            offset: ScreenPoint::new(100.0, 100.0),

            results: Vec::new(),

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

        let window = self.window_rect();

        let [top, bottom] = window.split_ver(window.height() * 0.15);

        let top_pad = ScreenSideOffsets::new(16.0, 8.0, 8.0, 8.0);

        let top = top.inner_rect(top_pad);

        let input_scale = 24.0;

        let input_text = Text::new(&self.input_buffer).with_scale(input_scale);

        let input_rect = top.inner_rect(top_pad);

        let pos = (input_rect.min_x(), input_rect.min_y());

        let section = Section::default()
            .with_screen_position(pos)
            .add_text(input_text);

        text_cache.queue(section);

        let result_scale = 20.0;

        let pad = ScreenSideOffsets::new(8.0, 8.0, 8.0, 8.0);

        for (ix, rect, entry) in self.layout.apply_to_rows(self.results.iter())
        {
            let text = Text::new(entry.text.as_str()).with_scale(result_scale);

            let r = rect.inner_rect(pad);

            let pos = (r.min_x(), r.min_y());
            let bounds = (r.max_x(), r.max_y());

            let section = Section::default()
                .with_screen_position(pos)
                // .with_layout(layout)
                .with_bounds(bounds)
                .add_text(text);

            text_cache.queue(section);
        }

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

                let color_focus = rgb::RGBA::new(0.85, 0.85, 0.85, 1.0);

                let pad = ScreenSideOffsets::new(8.0, 8.0, 8.0, 8.0);

                let [top, bottom] = bg_rect.split_ver(bg_rect.height() * 0.15);

                let base = vec![
                    rect_rgba(bg_rect, color_bg),
                    rect_rgba(top.inner_rect(pad), color_fg),
                    rect_rgba(bottom.inner_rect(pad), color_fg),
                ];

                let selection = self.selection_focus.and_then(|ix| {
                    let (_, rect, _) = self
                        .layout
                        .apply_to_rows(self.results.iter())
                        .nth(ix)?;

                    // let sel_vx = rect_rgba(rect.inner_rect(pad), color_focus);
                    let sel_vx = rect_rgba(rect, color_focus);
                    Some(sel_vx)
                });

                sublayer_data
                    .update_vertices_array(base.into_iter().chain(selection))?;
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
