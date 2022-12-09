use anyhow::Result;

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let args = if let Ok(args) = app::viewer_2d::parse_args() {
        args
    } else {
        let name = std::env::args().next().unwrap();
        println!("Usage: {name} <gfa> <layout tsv> <path name>");
        std::process::exit(0);
    };

    if let Err(e) = pollster::block_on(app::viewer_2d::run(args)) {
        log::error!("{:?}", e);
    }

    Ok(())
}