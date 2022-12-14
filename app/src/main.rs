use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug)]
pub struct Args {
    gfa: PathBuf,
    tsv: Option<PathBuf>,
    path_name: Option<String>,
    annotations: Option<PathBuf>,
}

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    if let Ok(args) = parse_args() {
        if args.tsv.is_some() && args.path_name.is_some() {
            let args_2d = waragraph::viewer_2d::Args {
                gfa: args.gfa,
                tsv: args.tsv.unwrap(),
                path_name: args.path_name.unwrap(),
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
    }


    Ok(())
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.opt_free_from_os_str(parse_path)?,
        path_name: pargs.opt_free_from_str()?,
        annotations: pargs.opt_value_from_os_str("--bed", parse_path)?,
    };

    Ok(args)
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}
