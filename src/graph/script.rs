use std::collections::BTreeMap;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use bstr::ByteSlice;
use parking_lot::RwLock;

use rhai::plugin::*;

use rhai::ImmutableString;

use rhai::Dynamic as Dyn;
// use rhai::Dynamic::{FALSE, TRUE, UNIT};

use super::Node;
use super::Path;
use super::Waragraph;

macro_rules! some_dyn_or_other {
    ($x:expr, $y:expr) => {
        if let Some(val) = $x {
            Ok(rhai::Dynamic::from(val))
        } else {
            Ok($y)
        }
    };
}

pub fn create_graph_module(waragraph: &Arc<Waragraph>) -> rhai::Module {
    let mut module: rhai::Module = rhai::exported_module!(rhai_module);

    let graph = waragraph.to_owned();
    module.set_native_fn("sequence_str", move |node: Node| {
        graph
            .sequences
            .get(usize::from(node))
            .and_then(|s| s.to_str().ok())
            .map_or(Ok(Dyn::FALSE), |s| {
                let is = rhai::ImmutableString::from(s);
                Ok(Dyn::from(is))
            })
    });

    let (path_names, path_indices) = {
        let mut index_map = rhai::Map::default();
        let mut names = Vec::new();
        for (&path, name) in waragraph.path_names.iter() {
            let name = name.to_str().unwrap();
            index_map.insert(name.into(), Dyn::from(path));
            names.push(Dyn::from(rhai::ImmutableString::from(name)));
        }

        let path_names = Dyn::from(names);
        let path_indices = Dyn::from(index_map);

        (path_names.into_shared(), path_indices.into_shared())
    };

    module.set_var("path_names", path_names);
    module.set_var("path_indices", path_indices);

    let graph = waragraph.to_owned();
    module.set_native_fn("get_path", move |name: &str| {
        some_dyn_or_other!(graph.path_index(name.as_bytes()), Dyn::FALSE)
    });

    let graph = waragraph.to_owned();
    module.set_native_fn("name", move |path: Path| {
        graph.path_name(path).and_then(|s| s.to_str().ok()).map_or(
            Ok(Dyn::FALSE),
            |s| {
                let is = rhai::ImmutableString::from(s);
                Ok(Dyn::from(is))
            },
        )

        // some_dyn_or_other!(graph.path_name(path), Dyn::FALSE)
    });

    let graph = waragraph.to_owned();
    module.set_native_fn("path_offset", move |path: Path| {
        Ok(graph.path_offset(path) as i64)
    });

    module
}

#[export_module]
pub mod rhai_module {

    use super::super::{Node as NodeIx, Path as PathIx};

    pub type Node = NodeIx;
    pub type Path = PathIx;

    pub fn node(i: i64) -> Node {
        NodeIx(i as u32)
    }

    #[rhai_fn(name = "to_int")]
    pub fn node_to_int(node: Node) -> i64 {
        node.0 as i64
    }
}
