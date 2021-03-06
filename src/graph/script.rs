use std::sync::Arc;

use bstr::ByteSlice;

use rhai::plugin::*;

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
    module.set_native_fn("get_graph", move || Ok(graph.clone()));

    let node_count = waragraph.node_count();
    module.set_native_fn("node_count", move || Ok(node_count as i64));

    let path_count = waragraph.path_count();
    module.set_native_fn("path_count", move || Ok(path_count as i64));

    let graph = waragraph.to_owned();
    module.set_native_fn("node_at_pos", move |pos: i64| {
        let total = graph.total_len();
        let pos = if pos.is_positive() {
            pos.min(total as i64) as usize
        } else {
            let v = pos + total as i64;
            v as usize
        };

        let ix = graph
            .node_sum_lens
            .binary_search(&pos)
            .map_or_else(|x| x, |x| x);

        let node = Node::from(ix);
        Ok(node)
    });

    let graph = waragraph.to_owned();
    module.set_native_fn("node_at_pos", move |pos: i64| {
        let pos = pos as usize;
        if let Some(node) = graph.node_at_pos(pos as usize) {
            Ok(Dyn::from(node))
        } else {
            Ok(Dyn::FALSE)
        }
    });

    let graph = waragraph.to_owned();
    module.set_native_fn("pos_at_node", move |path: Path, node: Node| {
        if let Some(path_sum) = graph.path_sum_lens.get(path.ix()) {
            if let Ok(ix) = path_sum.binary_search_by_key(&node, |(n, _)| *n) {
                if let Some((_, offset)) = path_sum.get(ix) {
                    return Ok(Dyn::from_int(*offset as i64));
                }
            }
        }

        Ok(Dyn::FALSE)
    });

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
            index_map.insert(name.into(), Dyn::from(path));
            names.push(Dyn::from(name.clone()));
        }

        let path_names = Dyn::from(names);
        let path_indices = Dyn::from(index_map);

        (path_names.into_shared(), path_indices.into_shared())
    };

    module.set_var("path_names", path_names);
    module.set_var("path_indices", path_indices);

    let graph = waragraph.to_owned();
    module.set_native_fn("get_path", move |name: &str| {
        some_dyn_or_other!(graph.path_index(name), Dyn::FALSE)
    });

    let graph = waragraph.to_owned();
    module.set_raw_fn(
        "name",
        rhai::FnNamespace::Global,
        rhai::FnAccess::Public,
        [std::any::TypeId::of::<Path>()],
        move |_ctx, args| {
            graph
                .path_name(args[0].clone_cast())
                .map_or(Ok(Dyn::FALSE), |s| Ok(Dyn::from(s.clone())))
        },
    );

    let graph = waragraph.to_owned();
    module.set_native_fn("path_offset", move |path: Path| {
        Ok(graph.path_offset(path) as i64)
    });

    module
}

#[export_module]
pub mod rhai_module {

    use super::super::{
        Node as NodeIx, Path as PathIx, Waragraph as Waragraph_,
    };

    pub type Node = NodeIx;
    pub type Path = PathIx;
    pub type Waragraph = Arc<Waragraph_>;

    pub fn path(p: i64) -> Path {
        PathIx(p as usize)
    }

    pub fn node(i: i64) -> Node {
        NodeIx(i as u32)
    }

    #[rhai_fn(global, name = "to_int")]
    pub fn node_to_int(node: Node) -> i64 {
        node.0 as i64
    }

    #[rhai_fn(global, pure)]
    pub fn to_string(node: &mut Node) -> String {
        node.to_string()
    }
}
