set_name("BED");
set_desc("BED & Annotations");

/*
/// Load a BED file
///
/// @bed_path: PathBuf
fn another_command(bed_path) {
  import "script/bed" as bed;
  bed::load_bed_file(bed_path)
}
*/

/// Load a BED file
///
/// @bed_path: PathBuf
/// @->: BED
fn load_bed(bed_path) {
  import "script/bed" as bed;
  bed::load_bed_file(bed_path);
}


/// Change visualization mode to a BED column
///
/// @bed: BED
/// @column: BEDColumn
fn display_column(bed, column) {
  // logic here
}

/// This is a zero-arity command
fn command_test() {
  print("hello world");
  return 13;
}

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
