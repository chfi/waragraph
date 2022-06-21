//! Command palette system & features

use std::{
    cell::RefMut,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{anyhow, bail, Result};

use crossbeam::atomic::AtomicCell;
use euclid::Length;
use glyph_brush::{GlyphCruncher, OwnedText};
use parking_lot::RwLock;
use raving::compositor::Compositor;
use rhai::plugin::*;
use rustc_hash::FxHashMap;

use crate::{
    console::data::AnnotationSet,
    geometry::{
        LayoutElement, ListLayout, ScreenPoint, ScreenRect, ScreenSideOffsets,
    },
    text::TextCache,
    viewer::gui::layer::rect_rgba,
};

pub struct PromptContext {
    universe: Arc<RwLock<PromptUniverse>>,
}

#[derive(Clone)]
enum ArgPrompt {
    Const {
        mk_prompt: Arc<dyn Fn() -> PromptObject + 'static>,
    },
    /*
    WithArgInput {
        input_type: ArgType,
        mk_prompt:
            Arc<dyn Fn(&rhai::Dynamic, &rhai::Map) -> PromptObject + 'static>,
    },
    */
}

struct PromptUniverse {
    arg_prompts: FxHashMap<std::any::TypeId, ArgPrompt>,

    type_ids: HashMap<rhai::ImmutableString, std::any::TypeId>,
}

impl PromptUniverse {
    pub fn get_prompt(&self, arg_ty: std::any::TypeId) -> Option<ArgPrompt> {
        self.arg_prompts.get(&arg_ty).cloned()
    }

    pub fn new(
        annotations: &Arc<
            RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>,
        >,
    ) -> Self {
        let mut arg_prompts = FxHashMap::default();
        let mut type_ids = HashMap::default();

        let path_type_id = std::any::TypeId::of::<std::path::PathBuf>();
        let bed_type_id = std::any::TypeId::of::<Arc<AnnotationSet>>();
        let bool_type_id = std::any::TypeId::of::<bool>();

        type_ids.insert("PathBuf".into(), path_type_id);
        type_ids.insert("BED".into(), bed_type_id);
        type_ids.insert("bool".into(), bool_type_id);

        let bool_prompt = ArgPrompt::Const {
            mk_prompt: Arc::new(|| {
                let arg_ty = ArgType {
                    name: "bool".into(),
                    id: std::any::TypeId::of::<bool>(),
                };

                let builder = DynPromptConfig {
                    result_type: arg_ty,
                    results_producer: Arc::new(move || {
                        vec![rhai::Dynamic::TRUE, rhai::Dynamic::FALSE]
                    }),
                    show: Arc::new(move |value| {
                        value.clone_cast::<bool>().to_string().into()
                    }),
                };
                let prompt = builder.into_prompt().unwrap();

                prompt.build().unwrap()
            }),
        };

        arg_prompts.insert(bool_type_id, bool_prompt);

        let path_prompt = ArgPrompt::Const {
            // mk_prompt: Box::new(|config| {
            mk_prompt: Arc::new(|| {
                let builder = FilePathPromptConfig::new(None).unwrap();

                let prompt = builder.into_prompt();

                prompt.build().unwrap()
            }),
        };

        arg_prompts.insert(path_type_id, path_prompt);

        let annots = annotations.clone();

        let bed_prompt = ArgPrompt::Const {
            mk_prompt: Arc::new(move || {
                let arg_ty = ArgType {
                    name: "BED".into(),
                    id: std::any::TypeId::of::<Arc<AnnotationSet>>(),
                };

                let annots = annots.clone();
                let builder = DynPromptConfig {
                    result_type: arg_ty,
                    results_producer: Arc::new(move || {
                        //
                        let annots = annots.read();

                        let mut results: rhai::Array = Vec::new();

                        for (name, bed) in annots.iter() {
                            results.push(rhai::Dynamic::from(bed.clone()));
                        }

                        results
                    }),
                    show: Arc::new(move |bed| {
                        let bed = bed.clone_cast::<Arc<AnnotationSet>>();
                        bed.source.clone()
                    }),
                };
                let prompt = builder.into_prompt().unwrap();

                prompt.build().unwrap()
            }),
        };

        arg_prompts.insert(bed_type_id, bed_prompt);

        Self {
            arg_prompts,
            type_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArgType {
    // id: u64,
    name: rhai::ImmutableString,
    id: std::any::TypeId,
}

type CommandPrompt<I, P> =
    PromptObjectRaw<CommandEntry, I, P, CommandArgumentState>;

type FilePathPrompt<I> = PromptObjectRaw<PathBuf, I, PathBuf, PathBuf>;

type DynPrompt<I, P> = PromptObjectRaw<rhai::Dynamic, I, P, rhai::Dynamic>;

#[derive(Clone)]
struct DynPromptConfig {
    result_type: ArgType,

    results_producer: Arc<dyn Fn() -> rhai::Array + Send + Sync>,

    show: Arc<dyn Fn(&rhai::Dynamic) -> rhai::ImmutableString + Send + Sync>,
}

impl DynPromptConfig {
    pub fn into_prompt(
        self,
    ) -> Result<DynPrompt<impl Iterator<Item = rhai::Dynamic>, ()>> {
        let select = Box::new(
            |val: &rhai::Dynamic| -> PromptAction<(), rhai::Dynamic> {
                //
                PromptAction::Return(val.clone())
            },
        );

        let display = Box::new(
            move |entry: &rhai::Dynamic,
                  _rect: ScreenRect|
                  -> glyph_brush::OwnedText {
                let result_scale = 20.0;

                OwnedText::new(&format!(">{}", (self.show)(entry)))
            },
        );

        let producer = self.results_producer;

        let update_choices = Box::new(move |_: ()| -> Result<_> {
            let vals = producer();
            Ok(vals.into_iter())
        });

        let prompt_object = PromptObjectRaw {
            prompt_input: (),
            select,
            display,
            update_choices,
        };

        Ok(prompt_object)
    }
}

#[derive(Default)]
struct CommandPromptConfig {
    // output_ty_filter: Option<Arc<dyn Fn(Option<&ArgType>) -> bool>>,
}

#[derive(Clone)]
struct CommandEntry {
    module: rhai::ImmutableString,
    command: Arc<Command>,
}

impl CommandPromptConfig {
    pub fn into_prompt(
        self,
        modules: &HashMap<rhai::ImmutableString, CommandModule>,
    ) -> Result<CommandPrompt<impl Iterator<Item = CommandEntry>, ()>> {
        let mut results = Vec::new();

        for (mod_name, module) in modules.iter() {
            for (cmd_name, cmd) in &module.commands {
                results.push(CommandEntry {
                    module: mod_name.clone(),
                    command: cmd.clone(),
                });
            }
        }

        results.sort_by(|c0, c1| {
            (&c0.module, &c0.command.name).cmp(&(&c1.module, &c1.command.name))
        });

        // let results = Arc::new(results);

        let select = Box::new(
            |entry: &CommandEntry| -> PromptAction<(), CommandArgumentState> {
                let state = CommandArgumentState::from_command(
                    &entry.module,
                    &entry.command,
                );

                PromptAction::Return(state)
            },
        );

        let display = Box::new(
            |entry: &CommandEntry,
             _rect: ScreenRect|
             -> glyph_brush::OwnedText {
                let result_scale = 20.0;

                let mut input_str = String::new();

                let inputs =
                    entry.command.inputs.iter().for_each(|(arg_name, ty)| {
                        if !input_str.is_empty() {
                            input_str.push_str(", ");
                        }

                        input_str.push_str(arg_name.as_str());
                        input_str.push_str(" : ");
                        input_str.push_str(ty.name.as_str());
                    });

                let input = if input_str.is_empty() {
                    input_str
                } else {
                    format!("({})", input_str)
                };

                let output = if let Some(out) = &entry.command.output {
                    format!("-> {}", out.name)
                } else {
                    String::new()
                };

                OwnedText::new(&format!(
                    ">{}{}{}",
                    entry.command.name, input, output
                ))
            },
        );

        let update_choices = Box::new(move |_: ()| -> Result<_> {
            let vals = results.clone();
            Ok(vals.into_iter())
        });

        let prompt_object = PromptObjectRaw {
            prompt_input: (),
            select,
            display,
            update_choices,
        };

        Ok(prompt_object)
    }
}

#[derive(Default)]
struct FilePathPromptConfig {
    predicate: Option<Arc<dyn Fn(&std::path::Path) -> bool>>,
    current_dir: PathBuf,
}

impl FilePathPromptConfig {
    pub fn into_prompt(self) -> FilePathPrompt<impl Iterator<Item = PathBuf>> {
        let prompt_input = self.current_dir.clone();

        let mut config = self;

        let select =
            Box::new(|path: &PathBuf| -> PromptAction<PathBuf, PathBuf> {
                if path.is_dir() {
                    PromptAction::PromptFor(path.to_owned())
                } else {
                    PromptAction::Return(path.to_owned())
                }
            });

        let display = Box::new(
            |path: &PathBuf, _rect: ScreenRect| -> glyph_brush::OwnedText {
                let result_scale = 20.0;

                if path.is_relative() {
                    log::error!("{:?} is relative", path.file_name());
                }

                if let Some(file_name) =
                    path.file_name().and_then(|name| name.to_str())
                {
                    OwnedText::new(file_name).with_scale(result_scale)
                } else {
                    OwnedText::new(&format!("{:?}", path.file_name()))
                }
            },
        );

        let update_choices = Box::new(move |path: PathBuf| {
            if path.is_dir() {
                let path = path.canonicalize()?;
                config.current_dir = path;
            }

            let results = config.current_results()?;

            let predicate = config.predicate.clone();

            // let go_up: PathBuf = "..".into();
            let go_up = path.parent().map(|_| "/..".into());

            let contents = results.filter_map(move |entry| {
                let path = entry.ok()?.path();

                if let Some(predicate) = &predicate {
                    predicate(&path).then(|| path)
                } else {
                    Some(path)
                }
            });

            let iter = go_up.into_iter().chain(contents);

            Ok(iter)
        });

        let prompt_object = PromptObjectRaw {
            prompt_input,
            select,
            display,
            update_choices,
        };

        prompt_object
    }

    pub fn current_results(&self) -> Result<std::fs::ReadDir> {
        let dir = std::fs::read_dir(&self.current_dir)?;
        Ok(dir)
    }

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
            let predicate = Arc::new(move |path: &std::path::Path| {
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

struct PromptObjectRaw<V, I, P, T>
where
    V: Clone + 'static,
    P: Clone,
    T: Clone,
    I: Iterator<Item = V>,
{
    prompt_input: P,
    select: Box<dyn Fn(&V) -> PromptAction<P, T> + 'static>,
    display: Box<dyn Fn(&V, ScreenRect) -> glyph_brush::OwnedText + 'static>,
    update_choices: Box<dyn FnMut(P) -> Result<I> + 'static>,
    // done: impl Fn(T) + 'static,
}

impl<V, I, P, T> PromptObjectRaw<V, I, P, T>
where
    V: Clone + 'static,
    P: Clone + 'static,
    T: Clone + Send + Sync + 'static,
    I: Iterator<Item = V> + 'static,
{
    fn build(self) -> Result<PromptObject> {
        PromptObject::new(
            self.prompt_input,
            self.select,
            self.display,
            self.update_choices,
            // done,
        )
    }
}

struct PromptObject {
    act: Box<
        dyn FnMut(usize) -> Result<std::ops::ControlFlow<rhai::Dynamic, ()>>,
    >,
    show: Box<dyn FnMut(usize, ScreenRect) -> glyph_brush::OwnedText>,

    result_len: std::rc::Rc<AtomicCell<usize>>,
}

impl PromptObject {
    fn new<V, I, P, T>(
        init: P,
        act: impl Fn(&V) -> PromptAction<P, T> + 'static,
        show: impl Fn(&V, ScreenRect) -> glyph_brush::OwnedText + 'static,
        mut update_choices: impl FnMut(P) -> Result<I> + 'static,
        // done: impl Fn(T) -> Option<PromptObject> + 'static,
    ) -> Result<PromptObject>
    where
        V: Clone + 'static,
        P: Clone,
        T: Clone + Send + Sync + 'static,
        I: Iterator<Item = V>,
    {
        use std::cell::RefCell;
        use std::rc::Rc;

        let results = update_choices(init)?.collect::<Vec<_>>();

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
                    results.extend(update_choices(prompt)?);
                    len.store(results.len());

                    Ok(std::ops::ControlFlow::Continue(()))
                }
                PromptAction::Return(value) => {
                    Ok(std::ops::ControlFlow::Break(rhai::Dynamic::from(value)))
                }
            }
        });

        Ok(Self {
            act,
            show,
            result_len,
        })
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
    ty: std::any::TypeId,

    data: T,
}

struct PromptCommandState {
    result_ty: Option<rhai::ImmutableString>,
}

#[derive(Debug, Clone)]
struct CommandArgumentState {
    // (module name, command)
    module: rhai::ImmutableString,
    command: Arc<Command>,

    remaining_args: VecDeque<CmdArg<()>>,
    applied_args: Vec<CmdArg<rhai::Dynamic>>,
}

impl CommandArgumentState {
    fn from_command(
        module: &rhai::ImmutableString,
        command: &Arc<Command>,
    ) -> Self {
        let mut result = Self {
            module: module.clone(),
            command: command.clone(),

            remaining_args: VecDeque::new(),
            applied_args: Vec::new(),
        };

        for (arg_name, ty) in command.inputs.iter() {
            result.remaining_args.push_back(CmdArg {
                name: arg_name.clone(),
                ty_name: ty.name.clone(),
                ty: ty.id,
                data: (),
            });
        }

        result
    }

    fn apply_argument(&mut self, arg: &rhai::Dynamic) -> Result<()> {
        if let Some(first_arg) = self.remaining_args.pop_front() {
            if arg.type_id() == first_arg.ty {
                let arg = CmdArg {
                    name: first_arg.name.clone(),
                    ty_name: first_arg.ty_name.clone(),
                    ty: first_arg.ty,

                    data: arg.clone(),
                };

                self.applied_args.push(arg);
            } else {
                let ty_name = first_arg.ty_name.clone();
                self.remaining_args.push_front(first_arg);
                bail!("Argument mismatch: {} != {}", arg.type_name(), ty_name);
            }
        }

        Ok(())
    }

    fn is_saturated(&self) -> bool {
        self.remaining_args.is_empty()
    }

    fn execute(
        &self,
        engine: &rhai::Engine,
        modules: &HashMap<rhai::ImmutableString, CommandModule>,
    ) -> Result<rhai::Dynamic> {
        if !self.is_saturated() {
            log::error!("Command being executed without being saturated");
        }

        let mut args = Vec::new();

        let module = modules.get(&self.module).unwrap();

        for arg in self.applied_args.iter() {
            args.push(arg.data.clone());
        }

        let res: rhai::Dynamic =
            self.command.fn_ptr.call(engine, &module.fn_ptr_ast, args)?;
        //

        Ok(res)
    }
}

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

#[derive(Debug, Clone)]
pub struct Command {
    pub name: rhai::ImmutableString,

    // pub desc: rhai::ImmutableString,
    pub fn_ptr: rhai::FnPtr,
    pub inputs: Vec<(rhai::ImmutableString, ArgType)>,
    // pub inputs: HashMap<rhai::ImmutableString, ArgType>,
    pub output: Option<ArgType>,
}

impl Command {
    pub fn new(name: &str, fn_ptr: rhai::FnPtr) -> Self {
        Self {
            name: name.into(),
            fn_ptr,
            inputs: Vec::new(),
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
    cmd_arg_state: Option<CommandArgumentState>,

    prompt_state: Option<Box<PromptObject>>,

    // stack: Vec<rhai::Dynamic>,
    selection_focus: Option<usize>,

    pub input_buffer: String,

    // results: Vec<(rhai::ImmutableString, Arc<Command>)>,
    // result_texts: Vec<rhai::ImmutableString>,
    modules: HashMap<rhai::ImmutableString, CommandModule>,

    offset: ScreenPoint,

    rect: ScreenRect,
    layout: ListLayout,

    context: PromptContext,
}

impl CommandPalette {
    pub fn open_command_prompt(&mut self) -> Result<()> {
        let config = CommandPromptConfig::default();
        let prompt = config.into_prompt(&self.modules)?;

        let prompt_cell: Arc<AtomicCell<Option<PromptObject>>> =
            Arc::new(AtomicCell::new(None));

        let prompt_state = prompt.build()?;
        self.prompt_state = Some(Box::new(prompt_state));

        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.prompt_state.is_some()
    }

    pub fn handle_input(
        &mut self,
        mut engine: rhai::Engine,
        event: &winit::event::WindowEvent,
    ) -> Result<()> {
        use winit::event::{KeyboardInput, VirtualKeyCode as VK, WindowEvent};

        if self.prompt_state.is_none() {
            return Ok(());
        }

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

                    let result_len =
                        self.prompt_state.as_ref().unwrap().result_len.load();

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
                                    self.selection_focus = Some(result_len - 1);
                                }
                            }
                        }
                        VK::Down => {
                            // Move result selection down
                            if pressed {
                                if let Some(focus) =
                                    self.selection_focus.as_mut()
                                {
                                    if *focus < result_len - 1 {
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

                            if !pressed {
                                return Ok(());
                            }

                            let mut new_state = None;

                            if let Some(mut prompt_state) =
                                self.prompt_state.take()
                            {
                                if let Some(ix) = self.selection_focus.take() {
                                    match (prompt_state.act)(ix).unwrap() {
                                        std::ops::ControlFlow::Continue(_) => {
                                            //
                                            log::error!("continue prompt");
                                        }
                                        std::ops::ControlFlow::Break(value) => {
                                            if value.type_id()
                                                == std::any::TypeId::of::<
                                                    CommandArgumentState,
                                                >(
                                                )
                                            {
                                                let state = value
                                                .cast::<CommandArgumentState>(
                                            );

                                                if state.is_saturated() {
                                                    let result = state
                                                        .execute(
                                                            &engine,
                                                            &self.modules,
                                                        )?;
                                                    log::error!("executed command, result: {:?}", result);
                                                } else {
                                                    log::error!("got a command argument state: {:?}", state);

                                                    let next_arg = state
                                                        .remaining_args
                                                        .front()
                                                        .unwrap();

                                                    log::error!(
                                                        "next argument: {:?}",
                                                        next_arg
                                                    );

                                                    let ctx = self
                                                        .context
                                                        .universe
                                                        .read();
                                                    if let Some(
                                                        ArgPrompt::Const {
                                                            mk_prompt,
                                                        },
                                                    ) = ctx
                                                        .arg_prompts
                                                        .get(&next_arg.ty)
                                                    {
                                                        let prompt =
                                                            mk_prompt();
                                                        new_state = Some(
                                                            Box::new(prompt),
                                                        );
                                                        dbg!();
                                                    }

                                                    self.cmd_arg_state =
                                                        Some(state);
                                                }
                                            } else {
                                                log::error!(
                                                    "returned value: {:?}",
                                                    value
                                                );

                                                if let Some(mut state) =
                                                    self.cmd_arg_state.take()
                                                {
                                                    state.apply_argument(
                                                        &value,
                                                    )?;

                                                    if state.is_saturated() {
                                                        //
                                                        let result = state
                                                            .execute(
                                                                &engine,
                                                                &self.modules,
                                                            )?;
                                                    } else {
                                                        let next_arg = state
                                                            .remaining_args
                                                            .front()
                                                            .unwrap();

                                                        log::error!(
                                                        "next argument: {:?}",
                                                        next_arg
                                                    );

                                                        let ctx = self
                                                            .context
                                                            .universe
                                                            .read();
                                                        if let Some(
                                                            ArgPrompt::Const {
                                                                mk_prompt,
                                                            },
                                                        ) = ctx
                                                            .arg_prompts
                                                            .get(&next_arg.ty)
                                                        {
                                                            let prompt =
                                                                mk_prompt();
                                                            new_state =
                                                                Some(Box::new(
                                                                    prompt,
                                                                ));
                                                            dbg!();
                                                        }

                                                        self.cmd_arg_state =
                                                            Some(state);
                                                    }
                                                }

                                                //
                                            }
                                        }
                                    }
                                }
                            }
                            if new_state.is_some() {
                                log::warn!("updating state");
                            } else if new_state.is_none() {
                                log::warn!("empty state!!");
                            }
                            self.prompt_state = new_state;
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

        let ctx = self.context.universe.read();

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
                        let ty_name = fields.next().unwrap().trim();

                        if let Some(ty) = ctx.type_ids.get(ty_name) {
                            let arg_ty = ArgType {
                                name: ty_name.into(),
                                id: *ty,
                            };

                            if name == "->" {
                                cmd.output = Some(arg_ty);
                            } else {
                                cmd.inputs.push((name.into(), arg_ty));
                            }
                        } else {
                            log::warn!(
                                "unknown command argument type: {}",
                                ty_name
                            );
                            continue;
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

    pub fn new(
        annotations: &Arc<
            RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>,
        >,
    ) -> Result<Self> {
        let bg_rect = euclid::rect(80.0, 80.0, 500.0, 500.0);

        let [top, bottom] = bg_rect.split_ver(bg_rect.height() * 0.15);

        let pad = ScreenSideOffsets::new(8.0, 8.0, 8.0, 8.0);

        let layout = ListLayout {
            origin: bottom.origin,
            size: bottom.size,
            side_offsets: Some(pad),
            slot_height: Length::new(30.0),
        };

        let config = FilePathPromptConfig::new(None)?;
        let prompt = config.into_prompt();

        let prompt_state = prompt.build()?;
        // let prompt_state = prompt.build(|path| {
        //     log::error!("selected path: {:?}", path);
        //     None
        // })?;

        let context = PromptContext {
            universe: Arc::new(RwLock::new(PromptUniverse::new(annotations))),
        };

        Ok(Self {
            // prompt_state_: Arc::new(AtomicCell::new(None)),
            cmd_arg_state: None,
            prompt_state: None,

            input_buffer: String::new(),
            modules: HashMap::new(),

            selection_focus: None,

            offset: ScreenPoint::new(100.0, 100.0),

            layout,

            rect: bg_rect,

            context,
        })
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

    pub fn queue_glyphs(&mut self, text_cache: &mut TextCache) -> Result<()> {
        if self.prompt_state.is_none() {
            return Ok(());
        }

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

        let state = self.prompt_state.as_mut().unwrap();

        let indices = 0..state.result_len.load();

        for (ix, rect, _ix) in self.layout.apply_to_rows(indices) {
            let text = (state.show)(ix, rect);

            let r = rect.inner_rect(pad);

            let pos = (r.min_x(), r.min_y());
            let bounds = (r.max_x(), r.max_y());

            let width = r.width();

            let section = Section::default()
                .with_screen_position(pos)
                // .with_layout(layout)
                .with_bounds(bounds)
                .add_text(&text);

            // log::warn!("{}\t{:#?}", r.width(), section);

            text_cache.queue(&section);

            // if let Some(rect) = text_cache.brush.glyph_bounds(&section) {
            //     log::warn!("bounding box: {:?}", rect);
            // }
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
        if self.prompt_state.is_none() {
            compositor.toggle_layer(layer_name, false);
            return Ok(());
        } else {
            compositor.toggle_layer(layer_name, true);
        }

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

                let result_len =
                    self.prompt_state.as_ref().unwrap().result_len.load();

                let selection = self.selection_focus.and_then(|ix| {
                    let (_, rect, _) =
                        self.layout.apply_to_rows(0..result_len).nth(ix)?;

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
