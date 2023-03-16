use anyhow::Result;
use std::io::prelude::*;
use std::io::BufReader;
use ultraviolet::Vec2;

pub mod simple_2d;
pub mod viewer_1d;
pub mod viewer_2d;

pub mod app;

pub mod annotations;
pub mod color;
pub mod gui;
pub mod list;

pub mod util;
