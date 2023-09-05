

// const comlink_url = "https://unpkg.com/comlink/dist/umd/comlink.js";


// importScripts();
// importScripts("../../../dist/umd/comlink.js");


// function getWorkerURL(url) {
//     const content = `importScripts("${url}");`;
//     return URL.createObjectURL(new Blob([content], { type: "text/javascript" }));
// }

          // console.log("eh");


// import * as wasm from './pkg/web.js';
// import * as Comlink from "https://unpkg.com/comlink/dist/esm/comlink.mjs";
// import * as Comlink from "./comlink.mjs";

          // console.log("hm");

// console.log(typeof wasm);
// console.log(wasm());
// console.log(wasm.wasm_bindgen);


// console.log("what: " + wasm);
// console.log(wasm_bindgen);

          // console.log("??????");

importScripts('./pkg/web.js');
// importScripts(comlink_url);
importScripts("./comlink.js");


console.log(wasm_bindgen);
console.log(typeof wasm_bindgen);

let _graph;

let _state;

wasm_bindgen('./pkg/web_bg.wasm')
    .then((w) => {
        console.log("done???");
        console.log(w);

        console.log(wasm_bindgen);

        const gfa_path = '../data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa';
        // const tsv_path = '../data/A-3105.layout.tsv';

        console.log("fetching GFA");

        let gfa = fetch(gfa_path);
        // let tsv = fetch(tsv_path);

        
        console.log("parsing GFA");

        // let ctx = wasm_bindgen.initialize_with_data_fetch(gfa, tsv
        let graph = wasm_bindgen.load_gfa_path_index(gfa);

        // Comlink.expose(wasm_bindgen);

        return graph;
    })
    .then((graph) => {
        console.log("GFA loaded");
        console.log("exposing interface");
        console.log(graph);
        console.log(graph.node_count());

        let path_name = "gi|528476637:29857558-29915771";
        let coord_sys = wasm_bindgen.CoordSys.global_from_graph(graph);
        let data = wasm_bindgen.generate_depth_data(graph, path_name);

        _state = { coord_sys, path_name, data };

        console.log(_state);

        _graph = graph;
        console.log("worker node count: " + _graph.node_count());

        postMessage("graph-ready");

    });

const obj = {
  counter: 0,
  inc() {
    this.counter++;
  },
};

Comlink.expose({
    sample_data(left, right, bin_count) {
        let bins = new Float32Array(bin_count);
        _state.coord_sys.sample_range(left, right, _state.data, bins);
        return Comlink.transfer(bins, [bins.buffer]);
    },
    get_graph() {
        return Comlink.proxy(_graph);
    }
});

// Comlink.expose {
//     __graph,
//     node_count() {
//     }
// }

