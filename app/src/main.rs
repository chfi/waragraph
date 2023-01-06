use std::path::PathBuf;

use waragraph::window::WindowHandler;
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};

use anyhow::Result;

#[derive(Debug)]
pub struct Args {
    gfa: PathBuf,
    tsv: Option<PathBuf>,
    annotations: Option<PathBuf>,

    init_range: Option<std::ops::Range<u64>>,
}

pub async fn run(
    event_loop: EventLoop<()>,
    window: winit::window::Window,
    mut state: raving_wgpu::State,
    mut app: Box<dyn waragraph::AppWindow>,
) -> Result<()> {
    let mut first_resize = true;
    let mut prev_frame_t = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match &event {
            Event::WindowEvent { window_id, event } => {
                let mut consumed = false;

                let size = window.inner_size();
                consumed = app.on_event([size.width, size.height], event);

                if !consumed {
                    match &event {
                        WindowEvent::KeyboardInput { input, .. } => {
                            use VirtualKeyCode as Key;
                            if let Some(code) = input.virtual_keycode {
                                if let Key::Escape = code {
                                    *control_flow = ControlFlow::Exit;
                                }
                            }
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit
                        }
                        WindowEvent::Resized(phys_size) => {
                            let old_size = state.size;

                            // for some reason i get a validation error if i actually attempt
                            // to execute the first resize
                            // NB: i think there's another event I should wait on before
                            // trying to do any rendering and/or resizing
                            if first_resize {
                                first_resize = false;
                            } else {
                                state.resize(*phys_size);
                            }

                            let new_size = window.inner_size();

                            app.resize(
                                &state,
                                old_size.into(),
                                new_size.into(),
                            )
                            .unwrap();
                        }
                        WindowEvent::ScaleFactorChanged {
                            new_inner_size,
                            ..
                        } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }

            Event::RedrawRequested(window_id) if *window_id == window.id() => {
                app.render(&mut state).unwrap();
            }
            Event::MainEventsCleared => {
                let dt = prev_frame_t.elapsed().as_secs_f32();
                prev_frame_t = std::time::Instant::now();

                app.update(&state, &window, dt);

                window.request_redraw();
            }

            _ => {}
        }
    })
}

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

    let app_1d = {
        let args_1d = waragraph::viewer_1d::Args {
            gfa: args.gfa.clone(),
            init_range: args.init_range.clone(),
        };
        waragraph::viewer_1d::init(&event_loop, &window, &state, args_1d)?
    };

    let app_2d = {
        let args_2d = args.tsv.map(|tsv| waragraph::viewer_2d::Args {
            gfa: args.gfa,
            tsv,
            annotations: args.annotations,
        });

        args_2d
            .map(|args| {
                waragraph::viewer_2d::init(&event_loop, &window, &state, args)
            })
            .transpose()?
    };

    let mut window_handler = WindowHandler::init([app_1d]).unwrap();

    window_handler.add_windows_iter(app_2d);

    if let Err(e) =
        pollster::block_on(window_handler.run(event_loop, window, state))
    {
        log::error!("{e}");
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
