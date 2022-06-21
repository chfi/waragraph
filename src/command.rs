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
    list::ListView,
    text::TextCache,
    viewer::gui::layer::rect_rgba,
};

mod file_prompt;

use file_prompt::*;

pub struct PromptContext {
    universe: Arc<RwLock<PromptUniverse>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArgType {
    // id: u64,
    name: rhai::ImmutableString,
    id: std::any::TypeId,
}

impl ArgType {
    fn new<T: std::any::Any>(name: &str) -> Self {
        Self {
            name: name.into(),
            id: std::any::TypeId::of::<T>(),
        }
    }
}

#[derive(Clone)]
enum ArgPrompt {
    Const {
        mk_prompt: Arc<
            dyn Fn(Option<rhai::ImmutableString>) -> PromptObject + 'static,
        >,
    },
    WithArgInput {
        input_type: ArgType,
        mk_prompt: Arc<
            dyn Fn(
                    Option<rhai::ImmutableString>,
                    &rhai::Dynamic,
                ) -> PromptObject
                + 'static,
        >,
    }, /*
       WithArgInput {
           input_type: ArgType,
           mk_prompt:
               Arc<dyn Fn(&rhai::Dynamic, &rhai::Map) -> PromptObject + 'static>,
       },
       */
}

struct PromptUniverse {
    arg_prompts: FxHashMap<ArgType, ArgPrompt>,

    types_by_name: HashMap<rhai::ImmutableString, ArgType>,
    // type_ids: HashMap<rhai::ImmutableString, std::any::TypeId>,
}

impl PromptUniverse {
    // pub fn get_prompt(&self, arg_ty: std::any::TypeId) -> Option<ArgPrompt> {
    //     self.arg_prompts.get(&arg_ty).cloned()
    // }

    pub fn new(
        annotations: &Arc<
            RwLock<BTreeMap<rhai::ImmutableString, Arc<AnnotationSet>>>,
        >,
    ) -> Self {
        let mut arg_prompts = FxHashMap::default();
        let mut types_by_name = HashMap::default();

        let path_type = ArgType::new::<std::path::PathBuf>("PathBuf");
        let bed_type = ArgType::new::<Arc<AnnotationSet>>("BED");
        let bed_col_ix_type = ArgType::new::<i64>("BEDColumn");
        // let bed_col_name_type = ArgType::new::<rhai::ImmutableString>("BEDColumn");

        let bool_type = ArgType::new::<bool>("bool");

        let mut add_type =
            |ty: &ArgType| types_by_name.insert(ty.name.clone(), ty.clone());

        add_type(&path_type);
        add_type(&bed_type);
        add_type(&bool_type);
        add_type(&bed_col_ix_type);

        let bool_prompt = ArgPrompt::Const {
            mk_prompt: Arc::new(|_opt| {
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

        arg_prompts.insert(bool_type, bool_prompt);

        let path_prompt = ArgPrompt::Const {
            mk_prompt: Arc::new(|opt| {
                let builder = if let Some(opts) = opt {
                    let exts = opts.split(",").map(|s| s.trim());
                    FilePathPromptConfig::from_ext_whitelist(None, exts)
                        .unwrap()
                } else {
                    FilePathPromptConfig::new(None).unwrap()
                };

                let prompt = builder.into_prompt();
                prompt.build().unwrap()
            }),
        };

        arg_prompts.insert(path_type, path_prompt);

        let annots = annotations.clone();

        let bed_prompt = ArgPrompt::Const {
            mk_prompt: Arc::new(move |_opt| {
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

        arg_prompts.insert(bed_type.clone(), bed_prompt);

        // let annots = annotations.clone();

        let col_ix_ty = bed_col_ix_type.clone();

        let bed_col_ix_prompt = ArgPrompt::WithArgInput {
            input_type: bed_type.clone(),
            mk_prompt: Arc::new(move |_opt, bed| {
                let bed = bed.clone_cast::<Arc<AnnotationSet>>();

                let builder = DynPromptConfig {
                    result_type: col_ix_ty.clone(),
                    results_producer: Arc::new(move || {
                        let mut results: rhai::Array = Vec::new();

                        for ix in 0..bed.columns.len() {
                            results
                                .push(rhai::Dynamic::from_int(3 + ix as i64));
                        }

                        results
                    }),
                    show: Arc::new(move |ix| {
                        let ix = ix.clone_cast::<i64>();
                        ix.to_string().into()
                    }),
                };

                let prompt = builder.into_prompt().unwrap();

                prompt.build().unwrap()
            }),
        };

        arg_prompts.insert(bed_col_ix_type, bed_col_ix_prompt);

        Self {
            arg_prompts,
            types_by_name,
        }
    }
}

type CommandPrompt<I, P> =
    PromptObjectRaw<CommandEntry, I, P, CommandArgumentState>;

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
                    format!(" -> {}", out.name)
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
        dyn FnMut(usize) -> Result<std::ops::ControlFlow<rhai::Dynamic, usize>>,
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

                    Ok(std::ops::ControlFlow::Continue(results.len()))
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

    ty: ArgType,

    opts: Option<rhai::ImmutableString>,

    data: T,
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
            let opts = command.arg_opts.get(&ty.name).cloned();

            // let ty = ArgType {
            //     name: ty.name.clone(),
            //     ty: ty.id,
            // };

            result.remaining_args.push_back(CmdArg {
                name: arg_name.clone(),
                ty: ty.clone(),
                opts,
                data: (),
            });
        }

        result
    }

    fn apply_argument(&mut self, arg: &rhai::Dynamic) -> Result<()> {
        if let Some(first_arg) = self.remaining_args.pop_front() {
            if arg.type_id() == first_arg.ty.id {
                let arg = CmdArg {
                    name: first_arg.name.clone(),
                    ty: first_arg.ty.clone(),
                    opts: first_arg.opts.clone(),
                    data: arg.clone(),
                };

                self.applied_args.push(arg);
            } else {
                let ty_name = first_arg.ty.name.clone();
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
    pub output: Option<ArgType>,

    pub arg_opts: HashMap<rhai::ImmutableString, rhai::ImmutableString>,
}

impl Command {
    pub fn new(name: &str, fn_ptr: rhai::FnPtr) -> Self {
        Self {
            name: name.into(),
            fn_ptr,
            inputs: Vec::new(),
            output: None,
            arg_opts: HashMap::default(),
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

#[derive(Clone)]
pub enum PromptFor {
    Command,
    Argument { ty: rhai::ImmutableString },
}

struct PromptState {
    cmd_arg_state: Option<CommandArgumentState>,
    prompt_state: Box<PromptObject>,
    list_view: ListView<usize>,
}

pub struct CommandPalette {
    // cmd_arg_state: Option<CommandArgumentState>,
    // prompt_state: Option<Box<PromptObject>>,
    // list_view: Option<ListView<()>>,
    prompt_state: Option<PromptState>,

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

        let prompt_state = prompt.build()?;

        let result_len = prompt_state.result_len.load();
        let mut list_view = ListView::new(0..result_len);
        list_view.resize(self.layout.slot_count().0);

        let state = PromptState {
            cmd_arg_state: None,
            prompt_state: Box::new(prompt_state),
            list_view,
        };

        self.prompt_state = Some(state);

        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.prompt_state.is_some()
    }

    pub fn close_prompt(&mut self) {
        self.prompt_state = None;
    }

    fn set_focus(&mut self, new: usize) {
        if self.prompt_state.is_none() {
            self.selection_focus = None;
            return;
        }

        if let Some(state) = self.prompt_state.as_mut() {
            let view = &mut state.list_view;
            view.scroll_to_ix(new);
            self.selection_focus = Some(new.min(view.max() - 1));
        }
    }

    // Returns Ok(true) if the argument state is saturated,
    // Err(prompt) if a prompt was created for one of the command
    // arguments, and Ok(false) if an argument is needed but cannot be
    // produced
    fn handle_cmd_arg_state(
        universe: &PromptUniverse,
        arg_state: &mut CommandArgumentState,
    ) -> std::result::Result<bool, PromptObject> {
        if arg_state.is_saturated() {
            return Ok(true);
        }
        let next_arg = arg_state.remaining_args.front().unwrap();

        if let Some(prompt) = universe.arg_prompts.get(&next_arg.ty) {
            match prompt {
                ArgPrompt::Const { mk_prompt } => {
                    let prompt = mk_prompt(next_arg.opts.clone());

                    if prompt.result_len.load() == 0 {
                        Ok(false)
                    } else {
                        Err(prompt)
                    }
                }
                ArgPrompt::WithArgInput {
                    input_type,
                    mk_prompt,
                } => {
                    let input_arg =
                        arg_state.applied_args.iter().find_map(|arg| {
                            (&arg.ty == input_type).then(|| arg.data.clone())
                        });

                    if let Some(input) = input_arg {
                        let prompt = mk_prompt(next_arg.opts.clone(), &input);
                        Err(prompt)
                    } else {
                        Ok(false)
                    }
                }
            }
        } else {
            Ok(false)
        }
    }

    // returns Ok(true) if the selection can be executed
    fn act_on_selection_impl(
        universe: &PromptUniverse,
        state: &mut PromptState,
        focus_ix: usize,
        list_len: usize,
    ) -> Result<bool> {
        let mut execute = false;

        match (state.prompt_state.act)(focus_ix)? {
            std::ops::ControlFlow::Continue(new_len) => {
                let mut list_view = ListView::new(0..new_len);
                list_view.resize(list_len);
                state.list_view = list_view;
                //
                log::error!("continue prompt");
            }
            std::ops::ControlFlow::Break(value) => {
                if value.type_id()
                    == std::any::TypeId::of::<CommandArgumentState>()
                {
                    let mut arg_state = value.cast::<CommandArgumentState>();

                    match Self::handle_cmd_arg_state(universe, &mut arg_state) {
                        Ok(true) => {
                            state.cmd_arg_state = Some(arg_state);
                            // saturated
                            execute = true;
                        }
                        Ok(false) => {
                            //
                            log::error!("cannot saturate command");
                        }

                        Err(prompt) => {
                            let result_len = prompt.result_len.load();
                            let mut list_view = ListView::new(0..result_len);
                            list_view.resize(list_len);

                            state.list_view = list_view;
                            state.prompt_state = Box::new(prompt);
                            state.cmd_arg_state = Some(arg_state);
                        }
                    }
                } else if let Some(arg_state) = state.cmd_arg_state.as_mut() {
                    arg_state.apply_argument(&value)?;

                    match Self::handle_cmd_arg_state(universe, arg_state) {
                        Ok(true) => {
                            // saturated
                            execute = true;
                        }
                        Ok(false) => {
                            //
                            log::error!("cannot saturate command");
                        }

                        Err(prompt) => {
                            log::error!("updating prompt");
                            let result_len = prompt.result_len.load();
                            let mut list_view = ListView::new(0..result_len);
                            list_view.resize(list_len);

                            state.list_view = list_view;
                            state.prompt_state = Box::new(prompt);
                        }
                    }
                }
            }
        }

        Ok(execute)
    }

    fn act_on_selection(
        &mut self,
        mk_engine: impl FnOnce() -> rhai::Engine,
        list_len: usize,
    ) -> Result<()> {
        if self.prompt_state.is_none() || self.selection_focus.is_none() {
            return Ok(());
        }

        let universe = &self.context.universe.read();

        let mut clear_state = false;

        if let Some(state) = self.prompt_state.as_mut() {
            let focus = self.selection_focus.unwrap();

            match Self::act_on_selection_impl(universe, state, focus, list_len)
            {
                Ok(true) => {
                    if let Some(arg_state) = state.cmd_arg_state.as_mut() {
                        let engine = mk_engine();
                        let result =
                            arg_state.execute(&engine, &self.modules)?;

                        clear_state = true;
                    }
                }
                Ok(false) => {
                    //
                }
                Err(e) => {
                    //
                    log::error!("Command palette error: {:?}", e);
                    clear_state = true;
                }
            }
        }

        if clear_state {
            self.prompt_state = None;
            self.selection_focus = None;
        }

        Ok(())
    }

    pub fn handle_input(
        &mut self,
        mk_engine: impl FnOnce() -> rhai::Engine,
        event: &winit::event::WindowEvent,
    ) -> Result<()> {
        use winit::event::{VirtualKeyCode as VK, WindowEvent};

        if self.prompt_state.is_none() {
            self.input_buffer.clear();
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

                    let result_len = self
                        .prompt_state
                        .as_ref()
                        .unwrap()
                        .prompt_state
                        .result_len
                        .load();

                    match key {
                        VK::Up => {
                            // Move result selection up
                            if pressed {
                                let ix = self
                                    .selection_focus
                                    .map(|ix| {
                                        ix.checked_sub(1).unwrap_or_default()
                                    })
                                    .unwrap_or(result_len - 1);

                                self.set_focus(ix);
                            }
                        }
                        VK::Down => {
                            // Move result selection down
                            if pressed {
                                let ix = self
                                    .selection_focus
                                    .map(|ix| ix + 1)
                                    .unwrap_or(0);
                                self.set_focus(ix);
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
                            if !pressed {
                                return Ok(());
                            }

                            let list_len = self.layout.slot_count().0;
                            self.act_on_selection(mk_engine, list_len)?;
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
                        let mut ty_name = fields.next().unwrap().trim();

                        let ty_name = {
                            let mut opts = None;

                            if let Some(opt_start) = ty_name.find('(') {
                                if let Some(end) = ty_name.find(')') {
                                    opts = Some(rhai::ImmutableString::from(
                                        &ty_name[(opt_start + 1)..end],
                                    ));
                                }

                                ty_name = &ty_name[..opt_start];
                            }

                            let ty_name = rhai::ImmutableString::from(ty_name);

                            if let Some(opts) = opts {
                                log::warn!("inserting arg opt for command {}: {} -> {:?}", cmd.name, ty_name, opts);
                                cmd.arg_opts.insert(ty_name.clone(), opts);
                            }

                            ty_name
                        };

                        if let Some(arg_ty) = ctx.types_by_name.get(&ty_name) {
                            if name == "->" {
                                cmd.output = Some(arg_ty.clone());
                            } else {
                                cmd.inputs.push((name.into(), arg_ty.clone()));
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

        let context = PromptContext {
            universe: Arc::new(RwLock::new(PromptUniverse::new(annotations))),
        };

        Ok(Self {
            // prompt_state_: Arc::new(AtomicCell::new(None)),
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

    pub fn header_rect(&self) -> ScreenRect {
        let mut rect = self.layout.rect();
        rect.origin.y -= 80.0;
        rect.size.height = 80.0;
        rect
    }

    pub fn list_rect(&self) -> ScreenRect {
        self.layout.rect()
    }

    pub fn list_inner_rect(&self) -> ScreenRect {
        self.layout.inner_rect()
    }

    pub fn window_rect(&self) -> ScreenRect {
        let header = self.header_rect();
        let list = self.layout.rect();
        header.union(&list)
    }

    pub fn queue_glyphs(&mut self, text_cache: &mut TextCache) -> Result<()> {
        if self.prompt_state.is_none() {
            return Ok(());
        }

        use glyph_brush::{Section, Text};

        let window = self.window_rect();

        let top = self.header_rect();
        // let [top, bottom] = window.split_ver(window.height() * 0.15);

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

        let indices = state.list_view.row_indices();

        // let indices = 0..(state.prompt_state.result_len.load());

        let mut max_text_x = self.layout.origin.x;

        // let mut max_rect_x = self.layout.origin.x;

        for (ix, rect, res_ix) in self.layout.apply_to_rows(indices) {
            if res_ix >= state.prompt_state.result_len.load() {
                continue;
            }

            let text = (state.prompt_state.show)(res_ix, rect);

            let r = rect.inner_rect(pad);

            let pos = (r.min_x(), r.min_y());
            let bounds = (r.max_x(), r.max_y());

            let section = Section::default()
                .with_screen_position(pos)
                // .with_layout(layout)
                .with_bounds(bounds)
                .add_text(&text);

            text_cache.queue(&section);

            if let Some(text_rect) = text_cache.brush.glyph_bounds(&section) {
                if max_text_x < text_rect.max.x {
                    max_text_x = text_rect.max.x;
                }
            }
        }

        let list_inner = self.list_inner_rect();

        let offsets = self.layout.offsets();

        let width = (max_text_x - list_inner.origin.x).max(500.0);
        self.layout.size.width = width + offsets.left + 2.0 * offsets.right;

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

                let top = self.header_rect();
                let bottom = self.list_rect();
                // let [top, bottom] = bg_rect.split_ver(bg_rect.height() * 0.15);

                let base = vec![
                    rect_rgba(bg_rect, color_bg),
                    rect_rgba(top.inner_rect(pad), color_fg),
                    rect_rgba(bottom.inner_rect(pad), color_fg),
                ];

                let result_len = self
                    .prompt_state
                    .as_ref()
                    .unwrap()
                    .prompt_state
                    .result_len
                    .load();

                let state = self.prompt_state.as_ref().unwrap();
                let indices = state.list_view.row_indices();

                let selection = self.selection_focus.and_then(|ix| {
                    indices.contains(&ix).then(|| {
                        let offset = ix - indices.start;
                        let (_, rect, res_ix) =
                            self.layout.apply_to_rows(indices).nth(offset)?;

                        let sel_vx = rect_rgba(rect, color_focus);
                        Some(sel_vx)
                    })?
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
