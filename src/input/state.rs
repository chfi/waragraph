use std::collections::HashMap;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;



#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum InputDef {
    Unit,
    Bool,
    Int,
    Float,
    Dyn,
}

#[derive(Debug, Clone)]
enum Input {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f32),
    Dyn(rhai::Dynamic)
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum OutputDef {
    Unit,
    Map,
}

#[derive(Debug, Clone)]
enum Output {
    Unit,
    Map(rhai::Map),
}


enum InputHandler {
    RustFn(Arc<dyn Fn(&mut Option<rhai::Map>, Input) -> Option<StateId>>),
    RhaiFn { ast: Arc<rhai::AST>, fn_ptr: rhai::FnPtr}
}

type InputId = usize;
type StateId = usize;
type OutputId = usize;

#[derive(Default, Clone)]
pub struct StateMachineBuilder {
    state_map: HashMap<rhai::ImmutableString, StateId>,
    input_map: HashMap<rhai::ImmutableString, InputId>,
    output_map: HashMap<rhai::ImmutableString, OutputId>,

    state_vars: Vec<Option<rhai::Map>>,
    inputs: Vec<InputDef>,
    outputs: Vec<OutputDef>,

    // takes a state var (if applicable), and an input (which must match), and optionally returns a new state to switch to
    state_input_handlers: Vec<FxHashMap<usize, Arc<dyn Fn(&mut Option<rhai::Map>, Input) -> Option<StateId>>>>,
}

impl StateMachineBuilder {
    fn add_state_impl(&mut self, name: &str, init_state: Option<rhai::Map>) -> StateId {
        if let Some(id) = self.state_map.get(name).copied() {
            id
        } else {
            let id = self.state_map.len();
            self.state_map.insert(name.into(), id);
            self.state_vars.push(init_state);
            self.state_input_handlers.push(Default::default());
            id
        }
    }

    pub fn add_state(&mut self, name: &str) -> StateId {
        self.add_state_impl(name, None)
    }
    
    pub fn add_state_with_var(&mut self, name: &str, init_state: rhai::Map) -> StateId {
        self.add_state_impl(name, Some(init_state))
    }
    
    pub fn add_input(&mut self, name: &str, def: InputDef) -> InputId {
        if let Some(id) = self.input_map.get(name).copied() {
            id
        } else {
            let id = self.input_map.len();
            self.inputs.push(def);
            id
        }
    }
    
    pub fn add_output(&mut self, name: &str, def: OutputDef) -> InputId {
        if let Some(id) = self.output_map.get(name).copied() {
            id
        } else {
            let id = self.output_map.len();
            self.outputs.push(def);
            id
        }
    }


}

#[derive(Clone)]
pub struct StateMachine {
    builder: StateMachineBuilder,

    current_state: usize,
}