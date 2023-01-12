use std::path::PathBuf;
use std::sync::Arc;

use waragraph::app::{Args, NewApp};
use waragraph_core::graph::PathIndex;
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};

use anyhow::Result;

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let args = waragraph::app::parse_args();

    if args.is_err() {
        let name = std::env::args().next().unwrap();
        println!("Usage: {name} <gfa> [tsv]");
        println!("4-column BED file can be provided using the --bed flag");
        std::process::exit(0);
    }

    let args = args?;

    let (event_loop, state) =
        pollster::block_on(raving_wgpu::initialize_no_window())?;

    let mut app = NewApp::init(args)?;

    app.init_viewer_1d(&event_loop, &state)?;

    if app.shared.tsv_path.is_some() {
        app.init_viewer_2d(&event_loop, &state)?;
    }

    app.run(event_loop, state)
}

/*
pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let args = parse_args();

    if args.is_err() {
        let name = std::env::args().next().unwrap();
        println!("Usage: {name} <gfa> [tsv]");
        println!("4-column BED file can be provided using the --bed flag");
        std::process::exit(0);
    }

    let args = args.unwrap();

    let (event_loop, window, state) =
        pollster::block_on(raving_wgpu::initialize())?;

    let path_index = PathIndex::from_gfa(&args.gfa)?;
    let path_index = Arc::new(path_index);

    let app_1d = {
        let args_1d = waragraph::viewer_1d::Args {
            gfa: args.gfa.clone(),
            init_range: args.init_range.clone(),
        };
        waragraph::viewer_1d::init(
            &event_loop,
            &window,
            &state,
            path_index.clone(),
            args_1d,
        )?
    };

    let app_2d = {
        let args_2d = args.tsv.map(|tsv| waragraph::viewer_2d::Args {
            gfa: args.gfa,
            tsv,
            annotations: args.annotations,
        });

        args_2d
            .map(|args| {
                waragraph::viewer_2d::init(
                    &event_loop,
                    &window,
                    &state,
                    path_index,
                    args,
                )
            })
            .transpose()?
    };

    let mut window_handler = WindowHandler::init([app_1d]).unwrap();
    window_handler.add_windows_iter(app_2d);

    let app = waragraph::app::App::init(window_handler)?;

    if let Err(e) = pollster::block_on(app.run(event_loop, window, state)) {
        log::error!("{e}");
    }

    Ok(())
}
*/
