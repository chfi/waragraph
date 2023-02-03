use crate::app::AppWindow;

pub struct SpokeViewer {
    //
}

impl AppWindow for SpokeViewer {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut raving_wgpu::gui::EguiCtx,
        dt: f32,
    ) {
        todo!()
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        todo!()
    }

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        todo!()
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
