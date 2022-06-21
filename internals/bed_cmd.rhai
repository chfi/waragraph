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
  let file_name = bed_path.file_name();
  bed::load_bed_file(bed_path.to_string(), file_name);
}


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
