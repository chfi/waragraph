import "script/bed" as bed;

let props = viewer::props;

let result = bed::load_bed_file("betaglobin.bed",
                                "betaglobin",
                                #{ "name": 3 });

let ix_data_source = slot::get_data_source(result.ix_data_source);

print(ix_data_source);

let tooltip = #{ ix_data_source: ix_data_source,
                 bed: result.bed,
                 name: "name",
                 col: 3 };

props.set("tooltip_data", [tooltip]);

result
