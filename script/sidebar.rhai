fn init_sidebar(label_space) {
  ui::init_layer("sidebar", 3, true);
  ui::allocate_sublayer("sidebar", "rect-rgb", "rect");
  ui::allocate_text_sublayer(label_space, "sidebar", "text");
  ui::allocate_sublayer("sidebar", "line-rgb", "line");
}
