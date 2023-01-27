use std::sync::Arc;

use egui::{mutex::Mutex, Color32, Context, Id, Response, Ui};

use crate::app::resource::FStats;

use super::{ColorMap, ColorScheme, ColorSchemeId};

pub struct ColorMapWidget<'a> {
    id: egui::Id,
    color_map: &'a mut super::ColorMap,
}

#[derive(Default, Clone)]
pub struct ColorMapWidgetState {
    // TODO: maybe store the WindowId here for reference, since the
    // egui contexts are paired with windows
    texture_handle: Arc<Mutex<Option<(ColorSchemeId, egui::TextureHandle)>>>,

    data_mode: String,
}

impl ColorMapWidgetState {
    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data().get_temp(id)
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data().insert_temp(id, self);
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

        if state.data_mode != data_mode {
            if let Some(stats) = data_stats(data_mode) {
                color_map.value_range = [stats.min, stats.max];
                color_map.color_range = [0.0, 1.0];
            }

            state.data_mode = data_mode.to_string();
        }

        state.store(ctx, id);

        Self { id, color_map }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let state =
            ColorMapWidgetState::load(ui.ctx(), self.id).unwrap_or_default();

        /*
        Instead of a bunch of sliders, at least the color range -- and maybe
        the value range -- should be represented as points that can be dragged
        along the X-axis of the color scheme image.

        Hovering over a point in the color scheme image should show the value
        that gets mapped to it (if any!)
        */

        let mut resp;

        let [min_v, max_v] = self.color_map.value_range;
        let val_range = 0f32..=max_v;

        {
            let s_min_v = egui::Slider::new(
                &mut self.color_map.value_range[0],
                val_range,
            );

            resp = ui.add(s_min_v);
        }

        {
            let val_range = min_v..=(max_v + 1.0);
            let s_max_v = egui::Slider::new(
                &mut self.color_map.value_range[1],
                val_range,
            );

            resp = resp.union(ui.add(s_max_v));
        }

        {
            let col_range = 0f32..=1f32;
            let s_min_v = egui::Slider::new(
                &mut self.color_map.color_range[0],
                col_range,
            );

            resp = resp.union(ui.add(s_min_v));
        }

        {
            let col_range = 0f32..=1f32;
            let s_max_v = egui::Slider::new(
                &mut self.color_map.color_range[1],
                col_range,
            );

            resp = resp.union(ui.add(s_max_v));
        }

        if let Some((_scheme_id, handle)) = state.texture_handle.lock().as_ref()
        {
            let [min_v, max_v] = self.color_map.value_range;
            let [min_c, max_c] = self.color_map.color_range;

            let size = [200f32, 40f32];

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

            if let Some(pos) = image.hover_pos() {
                let col_x = (pos.x - l) / (r - l);
                let val_x = min_v + (col_x * (max_v - min_v));
            }

            resp = resp.union(image);
        }

        resp
    }
}

impl<'a> egui::Widget for ColorMapWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}
