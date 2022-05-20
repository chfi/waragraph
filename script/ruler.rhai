fn init_ruler(label_space) {
  ui::init_layer("rulers", 4, true);
  ui::allocate_sublayer("rulers", "rect-rgb", "rect");
  ui::allocate_text_sublayer(label_space, "rulers", "text");
  ui::allocate_sublayer("rulers", "line-rgb", "lines");
}


fn pos_to_rect(rect, view, p) {
  let p0 = view.offset + 0.0;
  let l = view.len;
  let r = p0 + view.len;

  let p_ = (p - p0) / l;

  rect.x + rect.w * p_
}

fn draw_ruler(label_space, rect, view) {
  let labels = [];

  let mouse_pos = mouse::get_pos();

  let mouse_over = !(mouse_pos.x < rect.x || mouse_pos.x > rect.x + rect.w);

  // adding 0.0 softly converts to float
  let pos = #{ x: rect.x + 0.0, y: rect.y + 0.0 };
  pos.y -= 8.0;
  let text = `offset: ${view.offset}`;
  labels.push(label_vertex(pos, text));

  let mag = view.len.to_float();

  let mag = mag.log();

  // let order = 10.0 ** mag.to_int();
  let order = 10.0 ** mag.to_int();

  pos.x += 200.0;
  let text = `mag: ${mag}`;
  labels.push(label_vertex(pos, text));

  pos.x += 200.0;

  let count = view.len / order.to_int();

  let text = `max: ${view.offset + view.len}`;
  labels.push(label_vertex(pos, text));

  pos.x -= 400.0;
  pos.y += 8.0;

  let x0 = pos.x;
  let y0 = pos.y;

  let offset = view.offset.to_float();

  let r0 = mag;

  if offset == 0 {
    r0 = 0.0;
  }

  let r0_lo = 10.0 ** r0.floor();
  let r0_hi = 10.0 ** r0.ceiling();

  let aa = r0;

  let lhs_pos = (10.0 ** r0.floor()).to_int();

  let text = `order: ${order} -- lhs_pos: ${lhs_pos}`;

  let step = order;

  let v0 = view.offset + 0.0;

  let a0 = 10.0 ** v0.log().to_int();
  let i0 = view.len / a0.to_int();

  // print(`v0: ${v0}, v0.log(): ${v0.log()}, a0: ${a0}, i0: ${i0}`);

  for i in i0..(i0+10) {
    let x0 = pos_to_rect(rect, view, i * step);
    let pos = pos + #{ x: x0 };
    let text = `${i * step}`;
    labels.push(label_vertex(pos, text));
    // let p = #{ x: x0 +
  }

  let lines = [];

  if mouse_over {

    let dims = ui::get_window_size();

    let l0 = #{ x0: mouse_pos.x, y0: rect.y + 0.0, w0: 1.0 };
    l0 += #{ x1: l0.x0, y1: dims.height - 10.0, w1: 1.0 };
    l0 += #{ r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

    pos.y += 8.0;
    lines.push(l0);
  }



  ui::update_sublayer(label_space, "rulers", "text", labels);
  ui::update_sublayer("rulers", "lines", lines);
}


private fn label_vertex(pos, contents, color) {
  return pos + color + #{ contents: contents };
}

private fn label_vertex(pos, contents) {
  label_vertex(pos, contents, #{ r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
}
