set_name("BED");
set_desc("BED & Annotations");

/// This is a doc
/// comment!!!
fn another_command() {
  print("yes hello");
}

add_command("cmd_name", "cmd_desc", || {
  print("hello from a command!");
});

add_command("load_bed", "load a bed file", || {
  import "script/bed" as bed;
  let result = bed::load_bed_file("betaglobin.bed",
                                    "betaglobin",
                                 #{ "name": 3 });
});



/*
let builder = command_module("BED", "BED & Annotations");

builder.add_command("cmd_name", "cmd_desc", || {
  print("hello from a command!");
});

builder.add_command("load_bed", "load a bed file", || {
  import "script/bed" as bed;
  let result = bed::load_bed_file("betaglobin.bed",
                                    "betaglobin",
                                 #{ "name": 3 });
});


builder
*/