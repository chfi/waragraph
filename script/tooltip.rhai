/// Given a loaded BED file and column key, prepares a config object
/// that can be inserted into `tooltip_data` global,which then shows
/// the column data in a tooltip when the cursor is over a
/// corresponding node.
///
/// `bed_result` should be a map returned by `bed::load_bed_file`
/// `key` is a key into the `columns` map in `bed_result`, which
/// is the same as the `columns` argument in the
/// `bed::load_bed_file()` function.
/**
   Example usage (e.g. in a script loaded at startup with `--run`)
```
import "script/bed" as bed;
import "script/tooltip" as tooltip;

let bed_result = bed::load_bed_file("betaglobin.bed",
                                    "betaglobin",
                                    #{ "name": 3 });

let data = tooltip::bed_tooltip_data(bed_result, "name");

globals.tooltip_data = [data];
```

*/
fn bed_tooltip_data(bed_result, key) {
  let ix_data_source = slot::get_data_source(bed_result.ix_data_source);

  return #{ ix_data_source: ix_data_source,
            bed: bed_result.bed,
            name: key,
            col: bed_result.columns[key],
          };
}



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
  }

  let t_w = 8.0 * max_len;

  let height = labels.len().to_float() * 8.0;
  let width = t_w + 4.0;

  if x0 + width > dims.width {
    x0 = dims.width - width - 1.0;
  }

  if y0 + height > dims.height {
    y0 = dims.height - height - 1.0;
  }


  for (val, ix) in list {
    let text = `${val}`;

    let x = x0 + 1.0;
    let y = y0 + row_h * ix;

    let label = #{ x: x, y: y,
                   r: 0.0, g: 0.0, b: 0.0, a: 1.0,
                   contents: text };

    labels.push(label);
  }

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
