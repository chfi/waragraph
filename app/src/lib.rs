use anyhow::Result;
use ultraviolet::Vec2;
use std::io::prelude::*;
use std::io::BufReader;

pub mod viewer_1d;
pub mod viewer_2d;

pub mod gui;

pub mod annotations;

pub mod gpu_cache;