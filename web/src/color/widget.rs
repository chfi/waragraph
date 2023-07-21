use std::sync::Arc;

use raving_wgpu::egui;

use crossbeam_utils::atomic::AtomicCell;
use egui::{mutex::Mutex, Color32, Context, Id, Response, Ui};
use parking_lot::RwLock;

use crate::app::resource::FStats;

use super::{ColorMap, ColorScheme, ColorSchemeId, ColorStore};

pub struct ColorMapWidgetShared {
    colors: Arc<RwLock<ColorStore>>,

    id: egui::Id,
    data_stats: FStats, // TODO: use stats to add reset value range button
    data_mode: String,
    scheme_id: ColorSchemeId,
    color_map: Arc<AtomicCell<super::ColorMap>>,
}

impl ColorMapWidgetShared {
    pub fn new(
        colors: Arc<RwLock<ColorStore>>,
        id: Id,
        data_stats: FStats,
        data_mode: &str,
        scheme_id: ColorSchemeId,
        color_map: Arc<AtomicCell<ColorMap>>,
    ) -> Self {
        let result = Self {
            colors,
            id,
            data_stats,
            data_mode: data_mode.to_string(),
            scheme_id,
            color_map,
        };

        result
    }

    pub fn update(
        &mut self,
        data_stats: impl Fn(&str) -> Option<FStats>,
        data_mode: &str,
        scheme_id: ColorSchemeId,
    ) {
        let colors = self.colors.read();

        if self.data_mode != data_mode {
            let mut color_map = self.color_map.load();
            if let Some(stats) = data_stats(data_mode) {
                color_map.value_range = [stats.min, stats.max];
                color_map.color_range = [0.0, 1.0];
            }
            self.color_map.store(color_map);

            self.data_mode = data_mode.to_string();
        }
        self.scheme_id = scheme_id;
    }
}

// impl SettingsWidget for ColorMapWidgetShared {
//     fn show(
//         &mut self,
//         ui: &mut egui::Ui,
//         settings_ctx: &SettingsUiContext,
//     ) -> SettingsUiResponse {
//         {
//             let ctx = ui.ctx();
//             let state =
//                 ColorMapWidgetState::load(ctx, self.id).unwrap_or_default();

//             let colors = self.colors.blocking_read();
//             let color_scheme = colors.get_color_scheme(self.scheme_id);

//             let scheme_name = colors.get_scheme_name(self.scheme_id);

//             state.prepare_color_scheme(ctx, scheme_name, color_scheme);

//             state.store(ctx, self.id);
//         }

//         let mut color_map = self.color_map.load();
//         let widget = ColorMapWidget {
//             id: self.id,
//             color_map: &mut color_map,
//         };
//         let response = widget.show(ui);
//         self.color_map.store(color_map);

//         SettingsUiResponse { response }
//     }
// }

pub struct ColorMapWidget<'a> {
    id: egui::Id,
    color_map: &'a mut super::ColorMap,
}

#[derive(Default, Clone)]
pub struct ColorMapWidgetState {
    // TODO: maybe store the WindowId here for reference, since the
    // egui contexts are paired with windows
    texture_handle: Arc<Mutex<Option<(ColorSchemeId, egui::TextureHandle)>>>,
    // data_mode: String,
}

impl ColorMapWidgetState {
    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data_mut(|data| data.get_temp(id))
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data_mut(|data| data.insert_temp(id, self));
    }

    pub fn cached_color_scheme(&self) -> Option<ColorSchemeId> {
        let state = self.texture_handle.lock();
        let (scheme, _) = state.as_ref()?;
        Some(*scheme)
    }

    pub fn prepare_color_scheme(
        &self,
        ctx: &Context,
        scheme_name: &str,
        scheme: &ColorScheme,
    ) {
        if self.cached_color_scheme() == Some(scheme.id) {
            return;
        }

        let pixels: Vec<Color32> = scheme
            .colors
            .iter()
            .map(|c| {
                let [r, g, b, a] = *c;
                // let rgb = egui::Rgba::from_rgba_unmultiplied(r, g, b, a);
                let rgb = egui::Rgba::from_rgb(r, g, b);
                Color32::from(rgb)
            })
            .collect();

        let width = pixels.len();

        let image = egui::ColorImage {
            size: [width, 1],
            pixels,
        };

        let handle =
            ctx.load_texture(scheme_name, image, egui::TextureOptions::LINEAR);

        let mut state = self.texture_handle.lock();
        *state = Some((scheme.id, handle));
    }
}

impl<'a> ColorMapWidget<'a> {
    pub fn new(
        ctx: &Context,
        id: Id,
        data_stats: impl Fn(&str) -> Option<FStats>,
        data_mode: &str,
        scheme_name: &str,
        color_scheme: &ColorScheme,
        color_map: &'a mut ColorMap,
    ) -> Self {
        let mut state = ColorMapWidgetState::load(ctx, id).unwrap_or_default();

        // just upload the state here/on creation -- no need to
        // try to do it as part of show(), which will be limited
        // by the Widget trait
        state.prepare_color_scheme(ctx, scheme_name, color_scheme);

        state.store(ctx, id);

        Self { id, color_map }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let state =
            ColorMapWidgetState::load(ui.ctx(), self.id).unwrap_or_default();

        // allocate space, then insert the sliders "on top of" the image... kind of.
        // probably good enough to make them aligned, at least

        // remember to set ui.spacing.slider_width

        // let top_left = ui.cursor().min;

        let height = 192.0;
        let width = ui.available_width().min(300.0);
        let size = egui::vec2(width, height);

        // let rect = egui::Rect::from_min_size(top_left, size);

        // let resp = ui.allocate_ui_at_rect(size, |ui| {
        // let resp = ui.allocate_ui(size, |ui| {
        let resp = ui.allocate_ui_with_layout(
            size,
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let top_left = ui.cursor().min;
                let size = ui.available_size();
                ui.spacing_mut().slider_width = size.x;

                ui.add_space(64.0);

                let value_sliders_rect = egui::Rect::from_min_size(
                    top_left,
                    egui::vec2(size.x, 64.0),
                );

                // let img_top_left = ui.cursor().min;

                let img_size = egui::vec2(size.x, 48.0);

                Self::show_color_scheme_image(
                    &state,
                    &self.color_map,
                    img_size,
                    ui,
                );

                ui.add_space(4.0);
                let img_bottom_left = ui.cursor().min;

                let color_sliders_rect = egui::Rect::from_min_size(
                    img_bottom_left,
                    egui::vec2(size.x, 48.0),
                );

                let [min_v, max_v] = self.color_map.value_range;
                let val_range = 0f32..=max_v;

                ui.allocate_ui_at_rect(value_sliders_rect, |ui| {
                    // ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label("Value range");
                    {
                        let s_min_v = egui::Slider::new(
                            &mut self.color_map.value_range[0],
                            val_range,
                        );
                        ui.add(s_min_v);
                    }

                    {
                        let val_range = min_v..=(max_v + 1.0);
                        let s_max_v = egui::Slider::new(
                            &mut self.color_map.value_range[1],
                            val_range,
                        );
                        ui.add(s_max_v);
                    }
                });

                ui.allocate_ui_at_rect(color_sliders_rect, |ui| {
                    // ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label("Color range");
                    {
                        let col_range = 0f32..=1f32;
                        let s_min_v = egui::Slider::new(
                            &mut self.color_map.color_range[0],
                            col_range,
                        );
                        ui.add(s_min_v);
                    }

                    {
                        let col_range = 0f32..=1f32;
                        let s_max_v = egui::Slider::new(
                            &mut self.color_map.color_range[1],
                            col_range,
                        );
                        ui.add(s_max_v);
                    }
                });
            },
        );

        resp.response
    }

    fn show_color_scheme_image(
        state: &ColorMapWidgetState,
        color_map: &ColorMap,
        size: egui::Vec2,
        ui: &mut egui::Ui,
    ) {
        if let Some((_scheme_id, handle)) = state.texture_handle.lock().as_ref()
        {
            let [min_v, max_v] = color_map.value_range;
            let [min_c, max_c] = color_map.color_range;

            let image = ui.image(handle.id(), size);

            let rect = image.rect;
            let (l, r) = rect.x_range().into_inner();

            let len = r - l;

            let l_ = l + len * min_c;
            let r_ = l + len * max_c;

            let paint = ui.painter();

            let draw_line = |x: f32| {
                let (y0, y1) = rect.y_range().into_inner();

                let p0 = egui::pos2(x, y0);
                let p1 = egui::pos2(x, y1);

                paint.line_segment(
                    [p0, p1],
                    egui::Stroke::new(3.0, egui::Color32::GRAY),
                );

                paint.line_segment(
                    [p0, p1],
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                );
            };

            draw_line(l_);
            draw_line(r_);

            // if let Some(pos) = image.hover_pos() {
            //     let col_x = (pos.x - l) / (r - l);
            //     let val_x = min_v + (col_x * (max_v - min_v));
            // }

            // resp = resp.union(image);
        }
    }
}

impl<'a> egui::Widget for ColorMapWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}
