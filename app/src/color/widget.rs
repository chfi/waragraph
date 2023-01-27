use std::sync::Arc;

use egui::{mutex::Mutex, Color32, Context, Id, Response, Ui};

use super::{ColorMap, ColorScheme, ColorSchemeId};

pub struct ColorMapWidget<'a> {
    id: egui::Id,
    // scheme_name: String,
    color_scheme: ColorSchemeId,
    color_map: &'a mut super::ColorMap,
}

#[derive(Default, Clone)]
pub struct ColorMapWidgetState {
    // TODO: maybe store the WindowId here for reference, since the
    // egui contexts are paired with windows
    texture_handle: Arc<Mutex<Option<(ColorSchemeId, egui::TextureHandle)>>>,
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
        scheme_name: &str,
        color_scheme: &ColorScheme,
        color_map: &'a mut ColorMap,
    ) -> Self {
        let state = ColorMapWidgetState::load(ctx, id).unwrap_or_default();

        // just upload the state here/on creation -- no need to
        // try to do it as part of show(), which will be limited
        // by the Widget trait
        state.prepare_color_scheme(ctx, scheme_name, color_scheme);

        state.store(ctx, id);

        Self {
            id,
            color_scheme: color_scheme.id,
            color_map,
        }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let state =
            ColorMapWidgetState::load(ui.ctx(), self.id).unwrap_or_default();

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
            let size = [200f32, 40f32];
            resp = resp.union(ui.image(handle.id(), size));
        }

        resp
    }
}

impl<'a> egui::Widget for ColorMapWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}
