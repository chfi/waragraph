set_name("BED");
set_desc("BED & Annotations");

/// This is a doc
/// comment!!!
///
/// @path: PathBuf
fn another_command(path) {
  print("yes hello");
}

fn load_bed() {
  import "script/bed" as bed;
  let result = bed::load_bed_file("betaglobin.bed",
                                    "betaglobin",
                                 #{ "name": 3 });
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