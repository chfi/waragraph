use std::sync::Arc;

use egui::{mutex::Mutex, Color32, Context, Id};

use super::{ColorScheme, ColorSchemeId};

pub struct ColorMapWidget<'a> {
    color_scheme: ColorSchemeId,
    texture_handle: Option<(ColorSchemeId, egui::TextureHandle)>,
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

    // pub fn prepare_texture
}

// impl<'a> ColorMapWidget<'a> {
//     pub fn new(
//     //
// }

impl<'a> egui::Widget for ColorMapWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let [min_v, max_v] = self.color_map.value_range;

        let val_range = 0f32..=max_v;

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
        todo!()
    }
}
