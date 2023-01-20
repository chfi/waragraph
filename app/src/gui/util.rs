use std::sync::Arc;

use egui::{text::Fonts, FontId, Galley};

/// `text` should be a single line
pub(crate) fn fit_text_ellipsis(
    fonts: &Fonts,
    text: &str,
    font_id: FontId,
    color: egui::Color32,
    available_width: f32,
) -> Arc<Galley> {
    let galley = fonts.layout_no_wrap(text.to_string(), font_id.clone(), color);

    if galley.size().x <= available_width {
        return galley;
    }

    let ellipsis =
        fonts.layout_no_wrap("...".to_string(), font_id.clone(), color);

    let available = available_width - ellipsis.size().x;
    let start_t = available;
    let row = &galley.rows[0];

    let start_i = row.char_at(start_t);

    let string = format!("{}...", &text[..start_i]);

    let galley = fonts.layout_no_wrap(string, font_id, color);

    galley
}
