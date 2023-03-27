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

pub(crate) fn spinner(
    stroke: egui::Stroke,
    offset: egui::Vec2,
    t: f32,
) -> egui::Shape {
    use std::f32::consts::TAU;

    let n = 16usize;
    let rad = 7.0;

    let point = |i: usize| {
        let a = TAU * (i as f32 / n as f32);
        let x = rad * (a + t).cos();
        let y = rad * (a + t).sin();
        egui::pos2(x, y)
    };

    let line = |i0: usize| {
        let p0 = point(i0) + offset;
        let p1 = point(i0 + 1) + offset;
        egui::Shape::line_segment([p0, p1], stroke.clone())
    };

    let shapes = (0..(n / 2)).map(|i| line(i * 2)).collect::<Vec<_>>();

    egui::Shape::Vec(shapes)
}
