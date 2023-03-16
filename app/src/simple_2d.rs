/*
A simpler 2D graph viewer, designed for viewing subgraphs
*/

use raving_wgpu::gui::EguiCtx;
use ultraviolet::Vec2;

use crate::{
    app::{AppWindow, SharedState},
    viewer_2d::view::View2D,
};

pub struct SimpleLayout {
    // seg_vx_map: BTreeM
    // M
}

pub struct Simple2D {
    view: View2D,

    // node positions
    //
    shared: SharedState,
}

impl AppWindow for Simple2D {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        dt: f32,
    ) {
        egui_ctx.begin_frame(&window.window);

        {
            let ctx = egui_ctx.ctx();

            let main_area = egui::Area::new("main_area_2d")
                .fixed_pos([0f32, 0.0])
                .movable(false)
                .constrain(true);

            let screen_rect = ctx.available_rect();
            let dims = Vec2::new(screen_rect.width(), screen_rect.height());

            main_area.show(ctx, |ui| {
                ui.set_width(screen_rect.width());
                ui.set_height(screen_rect.height());

                let area_rect = ui
                    .allocate_rect(screen_rect, egui::Sense::click_and_drag());

                if area_rect.dragged_by(egui::PointerButton::Primary) {
                    let delta =
                        Vec2::from(mint::Vector2::from(area_rect.drag_delta()));
                    let mut norm_delta = -1.0 * (delta / dims);
                    norm_delta.y *= -1.0;
                    self.view.translate_size_rel(norm_delta);
                }

                let painter = ui.painter();
                // painter.extend(annot_shapes);
            });
        }

        egui_ctx.end_frame(&window.window);

        // todo!();
    }

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        // TODO: only using egui for now
        Ok(())
    }

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool {
        false
        // todo!()
    }

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
