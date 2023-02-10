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

    let spoke_graph = SpokeGraph::new(path_index.clone());

    // run 3EC algorithm on spoke_graph to get partition of *node endpoints*
    let three_ecs = {
        let segments = (0..path_index.node_count as u32).map(|i| {
            let node = Node::from(i);
            [node.as_reverse(), node.as_forward()]
        });
        let mut edges: Vec<(HubId, HubId)> = segments
            .filter_map(|[l, r]| {
                // project endpoints to HubIds
                let l_hub = spoke_graph.node_endpoint_hub(l)?;
                let r_hub = spoke_graph.node_endpoint_hub(r)?;

                // if proj(l) = proj(r), filter out
                (l_hub != r_hub).then_some((l_hub, r_hub))
            })
            .collect::<Vec<_>>();

        // doing this shouldn't make a difference to the output
        // edges.sort();
        // edges.dedup();

        let tec_graph = three_edge_connected::Graph::from_edges(
            edges.into_iter().map(|(l, r)| (l.ix(), r.ix())),
        );

        let mut components =
            three_edge_connected::find_components(&tec_graph.graph);

        // let components: Vec<_> =
        //     components.into_iter().filter(|c| c.len() > 1).collect();

        components.sort_by_cached_key(|c| c.len());

        components
    };

    // transform node endpoint partitions into hub partitions

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
