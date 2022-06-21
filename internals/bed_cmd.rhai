set_name("BED");
set_desc("BED & Annotations");


/// Load a BED file
///
/// @bed_path: PathBuf(bed)
/// @->: BED
fn load_bed(bed_path) {
  import "script/bed" as bed;
  let file_name = bed_path.file_name();
  bed::load_bed_file(bed_path.to_string(), file_name);
}


/// Visualize a BED column
///
/// @bed: BED
/// @column: BEDColumn
fn display_bed_column(bed, column) {
  // logic here
  let col = column;
  let name = col;

  let data_source_name = `${bed.name}:${col}`;
  slot::create_data_source(bed, col, data_source_name);
  let data_source = slot::get_data_source(data_source_name);

  let slot_fn_name = `${bed.name}:${name}`;
  let slot_fn = slot::new_slot_fn_from_data_source(data_source_name,
                                                   slot_fn_name);

  let col_type = bed.column_type(col);

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

  let cfg = viewer::cfg;
  let props = viewer::props;

  cfg.update("viz.slot_function", || {
    props.set("viz.previous", this);
    this = slot_fn_name;
  })
}

/*
/// TEST
///
/// @bed_path: PathBuf
/// @bed: BED
/// @a_bool: bool
/// @->: BED
fn load_bed_test(bed_path, bed, a_bool) {
  print(`bed_path: ${bed_path}, bed: ${bed}, a_bool: ${a_bool}`);
  // import "script/bed" as bed;
  // let file_name = bed_path1.file_name();
  // bed::load_bed_file(bed_path.to_string(), file_name);
}
*/


/*
add_command("cmd_name", "cmd_desc", || {
  print("hello from a command!");
});

add_command("load_bed", "load a bed file", || {
  import "script/bed" as bed;
  let result = bed::load_bed_file("betaglobin.bed",
                                    "betaglobin",
                                 #{ "name": 3 });
});
*/
