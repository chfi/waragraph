use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug)]
pub struct Args {
    gfa: PathBuf,
    tsv: Option<PathBuf>,
    annotations: Option<PathBuf>,
}

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    if let Ok(args) = parse_args() {
        if let Some(tsv) = args.tsv {
            let args_2d = waragraph::viewer_2d::Args {
                gfa: args.gfa,
                tsv,
                annotations: args.annotations,
            };

            if let Err(e) =
                pollster::block_on(waragraph::viewer_2d::run(args_2d))
            {
                log::error!("{:?}", e);
            }
        } else {
            let args_1d = waragraph::viewer_1d::Args { gfa: args.gfa };

            if let Err(e) =
                pollster::block_on(waragraph::viewer_1d::run(args_1d))
            {
                log::error!("{:?}", e);
            }
        }
    } else {
        let name = std::env::args().next().unwrap();
        println!("Usage: {name} <gfa> [tsv]");
        println!("4-column BED file can be provided using the --bed flag");
        std::process::exit(0);
    }


    Ok(())
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.opt_free_from_os_str(parse_path)?,
        annotations: pargs.opt_value_from_os_str("--bed", parse_path)?,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}
