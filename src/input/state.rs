use std::collections::HashMap;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum InputDef {
    Unit,
    Bool,
    // Int,
    // Float,
    // Dyn,
}

#[derive(Debug, Clone)]
pub enum Input {
    Unit,
    Bool(bool),
    // Int(i64),
    // Float(f32),
    // Dyn(rhai::Dynamic),
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum OutputDef {
    Unit,
    Map,
}

#[derive(Debug, Clone)]
pub enum Output {
    Unit,
    Map(rhai::Map),
}

pub type InputHandlerFn = Arc<
    dyn Fn(&mut Option<rhai::Map>, Input) -> Option<StateId>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
pub enum InputHandler {
    RustFn(InputHandlerFn),
    RhaiFn(StateId, InputId, rhai::FnPtr), // RhaiFn {
                                           //     ast: Arc<rhai::AST>,
                                           //     fn_ptr: rhai::FnPtr,
                                           // },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InputId(pub usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct StateId(pub usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OutputId(pub usize);

#[derive(Default, Clone)]
pub struct StateMachineBuilder {
    //    state_map: HashMap<rhai::ImmutableString, StateId>,
    //    input_map: HashMap<rhai::ImmutableString, InputId>,
    //    output_map: HashMap<rhai::ImmutableString, OutputId>,
    state_vars: Vec<Option<rhai::Map>>,
    inputs: Vec<InputDef>,
    outputs: Vec<OutputDef>,

    // takes a state var (if applicable), and an input (which must match), and optionally returns a new state to switch to
    state_input_handlers: Vec<FxHashMap<InputId, InputHandler>>,
}

impl StateMachineBuilder {
    fn add_state_impl(&mut self, init_state: Option<rhai::Map>) -> StateId {
        let id = self.state_vars.len();
        self.state_vars.push(init_state);
        self.state_input_handlers.push(Default::default());
        StateId(id)
    }

    pub fn add_state(&mut self) -> StateId {
        self.add_state_impl(None)
    }

    pub fn add_state_with_var(&mut self, init_state: rhai::Map) -> StateId {
        self.add_state_impl(Some(init_state))
    }

    pub fn add_input(&mut self, def: InputDef) -> InputId {
        let id = self.inputs.len();
        self.inputs.push(def);
        InputId(id)
    }

    pub fn add_output(&mut self, def: OutputDef) -> OutputId {
        let id = self.outputs.len();
        self.outputs.push(def);
        OutputId(id)
    }

    pub fn add_input_handler<F>(
        &mut self,
        state: StateId,
        input: InputId,
        handler: F,
    ) where
        F: Fn(&mut Option<rhai::Map>, Input) -> Option<StateId>
            + Send
            + Sync
            + 'static,
    {
        let si = state.0;
        let map = &mut self.state_input_handlers[si];
        map.insert(input, InputHandler::RustFn(Arc::new(handler)));
    }

    pub fn add_input_handler_rhai(
        &mut self,
        state: StateId,
        input: InputId,
        handler: rhai::FnPtr,
    ) {
        let si = state.0;
        let map = &mut self.state_input_handlers[si];
        map.insert(input, InputHandler::RhaiFn(state, input, handler));
    }

    /*
    pub fn build(
        self,
        ast: Arc<rhai::AST>,
        init: Option<StateId>,
    ) -> StateMachine {
        let current_state = init.unwrap_or(StateId(0));

        StateMachine {
            builder: self,
            current_state,
        }
    }
    */
}

#[derive(Clone)]
pub struct StateMachine {
    pub(super) current_state: StateId,

    pub(super) state_vars: Vec<Option<rhai::Map>>,
    pub(super) inputs: Vec<InputDef>,
    pub(super) outputs: Vec<OutputDef>,

    // takes a state var (if applicable), and an input (which must match), and optionally returns a new state to switch to
    pub(super) state_input_handlers: Vec<FxHashMap<InputId, InputHandlerFn>>,
}

impl StateMachine {
    pub fn build_no_rhai(
        builder: StateMachineBuilder,
        init_state: Option<StateId>,
    ) -> Self {
        #[cfg(debug_assertions)]
        for map in &builder.state_input_handlers {
            for (_input, handler) in map {
                if matches!(handler, InputHandler::RhaiFn(_, _, _)) {
                    panic!("Expected no Rhai handlers in this builder!");
                }
            }
        }

        //

        let mut state_input_handlers =
            builder.state_input_handlers.into_iter().map(|map| {
                map.into_iter().filter_map(|(k, v)| {
                    if let InputHandler::RustFn(f) = v {
                        Some((k, f))
                    } else {
                        None
                    }
                }).collect()
            }).collect();

        let current_state = init_state.unwrap_or(StateId(0));

        Self {
            current_state,
            state_vars: builder.state_vars,
            inputs: builder.inputs,
            outputs: builder.outputs,
            state_input_handlers,
        }
    }
}

fn extend_engine(
    engine: &mut rhai::Engine,
    builder: &Arc<RwLock<StateMachineBuilder>>,
) {
    let build = builder.clone();
    engine.register_fn("state", move || {
        let mut b = build.write();
        let state = b.add_state();
        state
    });

    let build = builder.clone();
    engine.register_fn("state", move |init: rhai::Map| {
        let mut b = build.write();
        let state = b.add_state_with_var(init);
        state
    });

    let build = builder.clone();
    engine.register_fn("input", move || {
        let mut b = build.write();
        let input = b.add_input(InputDef::Unit);
        input
    });

    let build = builder.clone();
    engine.register_fn("output", move || {
        let mut b = build.write();
        let output = b.add_output(OutputDef::Unit);
        output
    });

    let build = builder.clone();
    engine.register_fn(
        "on",
        move |state: &mut StateId, input: InputId, handler: rhai::FnPtr| {
            let mut b = build.write();
            b.add_input_handler_rhai(*state, input, handler);
        },
    );
}
