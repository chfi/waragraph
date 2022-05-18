/// Loads a BED file at the given path
/// `columns` should be a map from column indices in the BED file to names.
/// The indices that are given names will be loaded as visualization modes.
///
///
///
/// Example usage
/// ```
/// load_bed_file("some_file.bed",
///               #{ "Name": 3,
///                  "another column": 4 });
/// ```
fn load_bed_file(bed_path, columns) {
  let g = graph::get_graph();
  let bed_file = slot::load_bed_file(g, bed_path);

  print(`${columns}`);

  for name in columns.keys() {
    let col = columns[name];
    print(`${name} -> ${col}`);
    let data_source_name = slot::create_data_source(bed_file, col);
    let data_source = slot::get_data_source(data_source_name);

    let slot_fn = slot::new_slot_fn_from_data_source(data_source_name, name);

    let col_type = bed_file.column_type(col);

    // set the viz. color scheme based on the column type
    if col_type == "string" {
      slot::set_slot_color_scheme(name, "gradient-category10");
    } else if col_type == "i64" {
      slot::set_slot_color_scheme(name, "gradient-colorbrewer-spectral");
    } else if col_type == "f32" {
      slot::set_slot_color_scheme(name, "gradient-colorbrewer-spectral");
    } else {
      slot::set_slot_color_scheme(name, "gradient-colorbrewer-spectral");
    }
  }
}
