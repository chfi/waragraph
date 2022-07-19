use std::collections::HashMap;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use std::sync::Arc;

use crossbeam::atomic::AtomicCell;

use super::state::*;

pub struct MachineKeymap {
    key_map: FxHashMap<(winit::event::KeyboardInput, bool), Input>,
}

impl MachineKeymap {
    pub fn from_machine(
        machine: &StateMachine,
        keys: FxHashMap<winit::event::KeyboardInput, InputId>,
    ) -> Self {
        let mut key_map = FxHashMap::default();
        for (key, input) in keys {
            match machine.inputs[input.0] {
                InputDef::Unit => {
                    key_map.insert((key, true), Input::Unit);
                }
                InputDef::Bool => {
                    for t in [true, false] {
                        key_map.insert((key, t), Input::Bool(t));
                    }
                }
            }
        }

        Self { key_map }
    }
}
