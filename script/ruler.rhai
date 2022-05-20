fn init_ruler(label_space) {
  ui::init_layer("rulers", 4, true);
  ui::allocate_sublayer("rulers", "rect-rgb", "rect");
  ui::allocate_text_sublayer(label_space, "rulers", "text");
  ui::allocate_sublayer("rulers", "line-rgb", "lines");
}

fn draw_ruler(label_space, rect, view) {
  let labels = [];

  // let l = view.len;

  // adding 0.0 softly converts to float
  let pos = #{ x: rect.x + 0.0, y: rect.y + 0.0 };
  let text = `offset: ${view.offset}`;
  labels.push(label_vertex(pos, text));

  let mag = view.len.to_float();

  let mag = mag.log();

  let order = 10.0 ** mag.to_int();

  pos.x += 200.0;
  let text = `mag: ${order}`;
  labels.push(label_vertex(pos, text));

  pos.x += 200.0;

  let count = view.len / order;
  let text = `count: ${count}`;
  labels.push(label_vertex(pos, text));


  ui::update_sublayer(label_space, "rulers", "text", labels);
}


private fn label_vertex(pos, contents, color) {
  return pos + color + #{ contents: contents };
}

private fn label_vertex(pos, contents) {
  label_vertex(pos, contents, #{ r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
}
