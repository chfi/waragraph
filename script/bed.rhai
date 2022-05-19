/// Loads a BED file at the given path
/// `columns` should be a map from column indices in the BED file to names.
/// The indices that are given names will be loaded as visualization modes.
///
/// Returns a map where the `bed` value holds a reference to the
/// annotation set, and the other keys are the data source names for
/// the provided columns.
///
/// Example usage
/// ```
/// load_bed_file("some_file.bed",
///               #{ "Name": 3,
///                  "another column": 4 });
/// ```
fn load_bed_file(bed_path, bed_name, columns) {
  let g = graph::get_graph();
  let bed_file = slot::load_bed_file(g, bed_path);

  let ix_data_source_name = `${bed_name}_ix`;
  let ix_data_source = slot::create_data_source(bed_file,
                                                ix_data_source_name);

  let result = #{ bed: bed_file, ix_data_source: ix_data_source };

  for name in columns.keys() {
    let col = columns[name];

    let data_source_name = `${bed_name}:${col}`;
    slot::create_data_source(bed_file, col, data_source_name);
    let data_source = slot::get_data_source(data_source_name);

    let slot_fn_name = `${bed_name}:${name}`;
    let slot_fn = slot::new_slot_fn_from_data_source(data_source_name,
                                                     slot_fn_name);

    let col_type = bed_file.column_type(col);

    let key = `col-${name}`;
    result[key] = data_source_name;

    // set the viz. color scheme based on the column type
    if col_type == "string" {
      slot::set_slot_color_scheme(slot_fn_name,
                                  "gradient-category10");
    } else if col_type == "i64" {
      slot::set_slot_color_scheme(slot_fn_name,
                                  "gradient-colorbrewer-spectral");
    } else if col_type == "f32" {
      slot::set_slot_color_scheme(slot_fn_name,
                                  "gradient-colorbrewer-spectral");
    } else {
      slot::set_slot_color_scheme(slot_fn_name,
                                  "gradient-colorbrewer-spectral");
    }
  }

  result
}
