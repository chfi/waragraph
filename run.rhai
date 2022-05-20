import "script/bed" as bed;
import "script/tooltip" as tooltip;

let result = bed::load_bed_file("betaglobin.bed",
                                "betaglobin",
                                #{ "name": 3 });

globals.tooltip_data = [tooltip::bed_tooltip_data(result, "name")];

result
