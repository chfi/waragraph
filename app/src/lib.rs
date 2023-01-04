use anyhow::Result;
use std::io::prelude::*;
use std::io::BufReader;
use ultraviolet::Vec2;

pub mod viewer_1d;
pub mod viewer_2d;

pub mod gui;

pub mod annotations;

pub mod gpu_cache;

pub trait AppWindow {
    fn update(
        &mut self,
        state: &raving_wgpu::State,
        window: &winit::window::Window,
        dt: f32,
    );

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool;

    fn resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()>;

    fn render(&mut self, state: &mut raving_wgpu::State) -> anyhow::Result<()>;
}
