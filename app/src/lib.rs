use anyhow::Result;
use std::io::prelude::*;
use std::io::BufReader;
use ultraviolet::Vec2;

#[cfg(not(target_arch = "wasm32"))]
pub mod simple_2d;
#[cfg(not(target_arch = "wasm32"))]
pub mod viewer_1d;
pub mod viewer_2d;

#[cfg(not(target_arch = "wasm32"))]
pub mod app;
pub mod tile_app;

pub mod context;

pub mod annotations;
pub mod color;
pub mod gui;
pub mod list;

pub mod util;
