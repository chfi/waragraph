use std::{io::BufReader, path::PathBuf};

use waragraph_core::arrow_graph::parser::arrow_graph_from_gfa;
use waragraph_core::arrow_graph::ArrowGFA;

use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Input GFA path
    #[arg(long)]
    gfa: PathBuf,

    /// Output Arrow archive path
    #[arg(short, long)]
    out: PathBuf,
}

pub fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    env_logger::init();

    let gfa_file = std::fs::File::open(cli.gfa)?;
    eprintln!("Parsing GFA");
    let arrow_gfa = arrow_graph_from_gfa(BufReader::new(gfa_file))?;
    eprintln!("Writing archive");
    arrow_gfa.write_archive_file(cli.out)?;

    Ok(())
}
