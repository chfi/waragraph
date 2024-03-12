use std::{io::BufReader, path::PathBuf};

use waragraph_core::arrow_graph::parser::arrow_graph_from_gfa;
use waragraph_core::arrow_graph::ArrowGFA;
use waragraph_core::coordinate_system::CoordSys;

use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
struct ConvertCli {
    /// Input GFA path
    #[arg(long)]
    gfa: PathBuf,

    /// Output Arrow archive path
    #[arg(short, long)]
    out: PathBuf,
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct PosQueryCli {
    /// Input Arrow archive path
    #[arg(short, long)]
    archive: PathBuf,

    /// Path name to use as reference
    #[arg(short, long)]
    path_name: String,

    /// Path position (bp, 0-based)
    #[arg(short, long)]
    offset: u64,
}

#[derive(Subcommand, Debug)]
enum Command {
    Convert(ConvertCli),
    Position(PosQueryCli),
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

pub fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Convert(cli) => {
            let gfa_file = std::fs::File::open(cli.gfa)?;
            eprintln!("Parsing GFA");
            let arrow_gfa = arrow_graph_from_gfa(BufReader::new(gfa_file))?;
            eprintln!("Writing archive");
            arrow_gfa.write_archive_file(cli.out)?;
        }
        Command::Position(cli) => {
            let (arrow_gfa, _mmap) =
                unsafe { ArrowGFA::mmap_archive(&cli.archive)? };

            let Some(path_id) = arrow_gfa.path_name_id(&cli.path_name) else {
                eprintln!("Missing path: `{}`", cli.path_name);
                std::process::exit(1);
            };

            log::info!("Building coordinate system for path {}", cli.path_name);
            let coord_sys = CoordSys::path_from_arrow_gfa(&arrow_gfa, path_id);

            let seg = coord_sys.segment_at_pos(cli.offset);
            println!("{seg}");
        }
    }

    Ok(())
}
