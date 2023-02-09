use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use waragraph::spoke::SpokeGraph;
use waragraph_core::graph::PathIndex;

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let args = parse_args()?;

    let path_index = Arc::new(PathIndex::from_gfa(&args.gfa)?);

    let spoke_graph = SpokeGraph::new(path_index.clone());

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
