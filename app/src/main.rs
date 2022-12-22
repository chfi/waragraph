use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug)]
pub struct Args {
    gfa: PathBuf,
    tsv: Option<PathBuf>,
    annotations: Option<PathBuf>,

    init_range: Option<std::ops::Range<u64>>,
}

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    {
        let layout = waragraph::gui::test_layout()?;
        println!("-----------------");
        waragraph::gui::taffy_test()?;
        
        std::process::exit(0);
    }

    if let Ok(args) = parse_args() {
        dbg!();
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
            let args_1d = waragraph::viewer_1d::Args {
                gfa: args.gfa,
                init_range: args.init_range,
            };

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

    let annotations = pargs.opt_value_from_os_str("--bed", parse_path)?;
    let init_range = pargs.opt_value_from_fn("--range", parse_range)?;

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.opt_free_from_os_str(parse_path)?,

        annotations,
        init_range,
    };

    Ok(args)
}

fn parse_range(s: &str) -> Result<std::ops::Range<u64>> {
    const ERROR_MSG: &'static str = "Range must be in the format `start-end`,\
where `start` and `end` are nonnegative integers and `start` < `end`";

    let fields = s.trim().split('-').take(2).collect::<Vec<_>>();

    if fields.len() != 2 {
        anyhow::bail!(ERROR_MSG);
    }

    let start = fields[0].parse::<u64>()?;
    let end = fields[1].parse::<u64>()?;
    if start >= end {
        anyhow::bail!(ERROR_MSG);
    }

    Ok(start..end)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}
