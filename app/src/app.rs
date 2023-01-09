mod window;

use tokio::runtime::Runtime;
use winit::event_loop::EventLoop;

use std::sync::Arc;

use anyhow::Result;

pub use window::WindowHandler;

pub struct App {
    window_handler: WindowHandler,

    tokio_rt: Arc<Runtime>,
}

pub trait AppWindow {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
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

impl App {
    pub fn init(window_handler: WindowHandler) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("waragraph-tokio")
            .build()?;

        let tokio_rt = Arc::new(runtime);

        Ok(Self {
            window_handler,
            tokio_rt,
        })
    }

    pub async fn run(
        self,
        event_loop: EventLoop<()>,
        window: winit::window::Window,
        state: raving_wgpu::State,
    ) -> Result<()> {
        let Self {
            window_handler,
            tokio_rt,
        } = self;

        window_handler
            .run(tokio_rt, event_loop, window, state)
            .await
    }
}
