/// Displays a tooltip at the provided position, mapping each value in
/// `list` to a string.
///
/**
   ```
   show_tooltip(label_space,
                #{ x: 100.0, y: 100.0 },
                [true, 3.14, "a string"]);
   ```
*/
///
/// NB: This will generally be called in a function that already
/// receives a label space, such as `gui_update()` in paths.rhai
fn show_tooltip(label_space, pos, list) {
  ui::toggle_layer("tooltip", true);

  let x0 = pos.x + 8.0;
  let y0 = pos.y + 0.0;

  let row_h = 8.0;

  let total_height = list.len() * row_h;

  let dims = ui::get_window_size();

  if y0 + total_height > dims.height {
    let diff = y0 + total_height - dims.height;
    y0 -= diff + 2.0;
  }

  let rects = [];
  let labels = [];

  let max_len = 0;

  for (val, ix) in list {
    let text = `${val}`;

    if max_len < text.len() {
      max_len = text.len();
    }

    let x = x0 + 1.0;
    let y = y0 + row_h * ix;

    let label = #{ x: x, y: y,
                   r: 0.0, g: 0.0, b: 0.0, a: 1.0,
                   contents: text };

    labels.push(label);
  }

  let t_w = 8.0 * max_len;

  let height = labels.len().to_float() * 8.0;
  let width = t_w + 4.0;

  let col0 = rgb(0.85, 0.85, 0.85);
  let col1 = rgb(0.75, 0.75, 0.75);

  rects.push(#{ x: x0 - 2.0, y: y0 - 2.0,
                w: width + 4.0, h: height + 4.0
              } + rgb(0.55, 0.55, 0.55));

  rects.push(#{ x: x0 - 1.0, y: y0 - 1.0,
                w: width + 2.0, h: height + 2.0
              } + col0 );

  for (val, ix) in list {

    let x = x0 - 1.0;
    let y = y0 + row_h * ix;

    let color = if ix % 2 == 0 {
      col0
    } else {
      col1
    };

    rects.push(#{ x: x, y: y, w: width, h: row_h } + color);
  }

  ui::update_sublayer("tooltip", "rect", rects);
  ui::update_sublayer(label_space, "tooltip", "text", labels);
}


/// Initialize the tooltip compositor layer. Called in paths.rhai's `gui_init()`
fn init_tooltip(label_space) {
  ui::init_layer("tooltip", 10, false);
  ui::allocate_sublayer("tooltip", "rect-rgb", "rect");
  ui::allocate_text_sublayer(label_space, "tooltip", "text");
}
