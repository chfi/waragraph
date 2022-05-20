fn init_ruler(label_space) {
  ui::init_layer("rulers", 4, true);
  ui::allocate_sublayer("rulers", "rect-rgb", "rect");
  ui::allocate_text_sublayer(label_space, "rulers", "text");
  ui::allocate_sublayer("rulers", "line-rgb", "lines");
}


fn pos_to_rect(rect, view, p) {
  // let p0 = view.offset + 0.0;
  // let l = view.len + 0.0;
  let p0 = view.offset + 0.0;
  let l = view.len;
  let r = p0 + view.len;

  let p_ = (p - p0) / l;

  rect.x + rect.w * p_
}

fn draw_ruler(label_space, rect, view) {
  let labels = [];

  // let l = view.len;

  // adding 0.0 softly converts to float
  let pos = #{ x: rect.x + 0.0, y: rect.y + 0.0 };
  pos.y -= 8.0;
  let text = `offset: ${view.offset}`;
  labels.push(label_vertex(pos, text));

  let mag = view.len.to_float();

  let mag = mag.log();

  let order = 10.0 ** mag.to_int();

  pos.x += 200.0;
  let text = `mag: ${order}`;
  labels.push(label_vertex(pos, text));

  pos.x += 200.0;

  let count = view.len / order.to_int();

  // if count == 1 {
  //   count += 10;
  // }
  // let text = `count: ${count}`;
  let text = `max: ${view.offset + view.len}`;
  labels.push(label_vertex(pos, text));

  pos.x -= 400.0;
  pos.y += 8.0;

  let x0 = pos.x;
  let y0 = pos.y;

  let offset = view.offset.to_float();
  let r0 = offset.log();

  let r0_lo = 10.0 ** r0.floor();
  let r0_hi = 10.0 ** r0.ceiling();

  // let text = `offset ${offset}, mag: ${r0}`;
  // let text = `offset ${offset}, lo: ${r0_lo}, hi: ${r0_hi}`;
  let text = `r0(log) ${r0}, lo: ${r0_lo}, hi: ${r0_hi}`;
  pos.x = pos_to_rect(rect, view, r0_lo.to_int());
  labels.push(label_vertex(pos, text));

  /*
  for i in 0..count {
    let p = #{ x: x0 + offset, y: y0 };

    let text = `${i}`;

    labels.push(p, text);
  }
  */


  ui::update_sublayer(label_space, "rulers", "text", labels);
}


private fn label_vertex(pos, contents, color) {
  return pos + color + #{ contents: contents };
}

private fn label_vertex(pos, contents) {
  label_vertex(pos, contents, #{ r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
}
