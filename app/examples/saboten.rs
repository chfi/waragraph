use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use waragraph::spoke::{HubId, SpokeGraph};
use waragraph_core::graph::{Node, PathIndex};

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let args = parse_args()?;

    let path_index = Arc::new(PathIndex::from_gfa(&args.gfa)?);

    let spoke_graph = SpokeGraph::new_from_graph(&path_index);

    let three_ecs = {
        //

        let seg_hubs = (0..path_index.node_count as u32).map(|i| {
            let node = Node::from(i);
            let left = spoke_graph.node_endpoint_hub(node.as_reverse());
            let right = spoke_graph.node_endpoint_hub(node.as_forward());
            (left, right)
        });

        let tec_graph = three_edge_connected::Graph::from_edges(
            seg_hubs.into_iter().map(|(l, r)| (l.ix(), r.ix())),
        );

        let mut components =
            three_edge_connected::find_components(&tec_graph.graph);

        components
    };

    println!("found {} 3EC components", three_ecs.len());

    // create HyperSpokeGraph from hub partitions

    //

    Ok(())
}

struct Args {
    gfa: PathBuf,
}

fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}
