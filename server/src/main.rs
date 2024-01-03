use rocket::State;
use rocket::{get, launch, routes};

use waragraph_core::arrow_graph::{ArrowGFA, PathIndex};

#[get("/world")]
fn world() -> &'static str {
    "hello world"
}

// #[get("/args")]
// fn args_route(args_s: &State<ArgsVec>) -> String {
//     let args = args_s.0.join("\n");
//     args
//     // let args = args_vec.join("\n");
// }

// #[derive(Debug, Clone)]
// struct ArgsVec(Vec<String>);

#[launch]
fn rocket() -> _ {
    let args = std::env::args().collect::<Vec<_>>();

    let gfa = &args[1];
    // let tsv = args[2];

    let gfa = std::fs::File::open(gfa)
        .map(std::io::BufReader::new)
        .unwrap();
    // let tsv = std::fs::File::open(tsv).unwrap();

    let agfa =
        waragraph_core::arrow_graph::parser::arrow_graph_from_gfa(gfa).unwrap();

    rocket::build().manage(agfa).mount("/hello", routes![world])
    // .mount("/", routes![args_route])
}

// fn main() {
//     println!("Hello, world!");
// }
