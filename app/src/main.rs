use waragraph::app::App;

use waragraph::tile_app;

use anyhow::Result;

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        // .filter_level(log::LevelFilter::Debug)
        .init();

    let app = tile_app::App::init()?;
}

pub fn main_old() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        // .filter_level(log::LevelFilter::Debug)
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

    let mut app = App::init(&state, args)?;

    app.init_viewer_1d(&event_loop, &state)?;

    if app.shared.workspace.blocking_read().tsv_path().is_some() {
        app.init_viewer_2d(&event_loop, &state)?;
    }

    app.run(event_loop, state)
}
