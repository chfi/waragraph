fn init_ruler(label_space) {
  ui::init_layer("rulers", 4, true);
  ui::allocate_sublayer("rulers", "rect-rgb", "rect");
  ui::allocate_text_sublayer(label_space, "rulers", "text");
  ui::allocate_sublayer("rulers", "line-rgb", "lines");
}

fn draw_ruler(label_space, rect, view) {


  //
}
