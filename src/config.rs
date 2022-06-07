use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::RwLock;

use rhai::plugin::*;

use rhai::ImmutableString;

#[derive(Debug, Default, Clone)]
pub struct ConfigMap {
    pub map: Arc<RwLock<rhai::Map>>,

    pub schema: Option<Arc<BTreeMap<ImmutableString, std::any::TypeId>>>,
}

#[export_module]
pub mod config {

    use rhai::Dynamic as Dyn;

    use crate::console::EvalResult;

    use super::*;

    pub type Config = super::ConfigMap;

    pub fn new_config_map() -> ConfigMap {
        ConfigMap::default()
    }

    #[rhai_fn(global, return_raw)]
    pub fn update(
        ctx: NativeCallContext,
        cfg: &mut ConfigMap,
        key: &str,
        f: rhai::FnPtr,
    ) -> EvalResult<Dyn> {
        if let Some(val) = cfg.map.write().get_mut(key) {
            let result = f.call_raw(&ctx, Some(val), [])?;
            Ok(result)
        } else {
            Ok(Dyn::UNIT)
        }
    }

    #[rhai_fn(global, return_raw)]
    pub fn fetch(
        ctx: NativeCallContext,
        cfg: &mut ConfigMap,
        key: &str,
        f: rhai::FnPtr,
    ) -> EvalResult<Dyn> {
        if let Some(val) = cfg.map.read().get(key) {
            // doesn't bind `this` since `this` has to be mutable
            let result: Dyn = f.call_within_context(&ctx, (val.to_owned(),))?;
            Ok(result)
        } else {
            Ok(Dyn::UNIT)
        }
    }

    #[rhai_fn(name = "get", global, return_raw)]
    pub fn get_str_key(cfg: &mut ConfigMap, key: &str) -> EvalResult<Dyn> {
        if let Some(val) = cfg.map.read().get(key) {
            Ok(val.to_owned())
        } else {
            Ok(Dyn::UNIT)
        }
    }

    /// Returns `false` if the value doesn't match the schema (always
    /// `true` if there is no schema)
    #[rhai_fn(name = "set", global, return_raw)]
    pub fn set_str_key_dyn(
        cfg: &mut ConfigMap,
        key: &str,
        val: Dyn,
    ) -> EvalResult<Dyn> {
        let type_matches = cfg
            .schema
            .as_ref()
            .and_then(|schema| {
                let expected = schema.get(key)?;
                Some(expected == &val.type_id())
            })
            .unwrap_or(true);

        if type_matches {
            let old = cfg.map.write().insert(key.into(), val);
            Ok(old.unwrap_or(Dyn::UNIT))
        } else {
            Ok(Dyn::FALSE)
        }
    }
}
