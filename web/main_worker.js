
import init_wasm, * as wasm_bindgen from './pkg/web.js';
import * as Comlink from './comlink.mjs';
// importScripts('./pkg/web.js');
// importScripts("./comlink.js");


console.log(wasm_bindgen);
console.log(typeof wasm_bindgen);

let _graph;

let _state;

let wasm;

async function run() {
    wasm = await init_wasm();

    console.log(wasm_bindgen);

    const gfa_path = '../data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa';

    console.log("fetching GFA");

    let gfa = fetch(gfa_path);
    
    console.log("parsing GFA");

    // let ctx = wasm_bindgen.initialize_with_data_fetch(gfa, tsv
    let graph = await wasm_bindgen.load_gfa_arrow(gfa);

    let path_name = "gi|528476637:29857558-29915771";
    let coord_sys = wasm_bindgen.CoordSys.global_from_arrow_gfa(graph);
    let data = wasm_bindgen.generate_depth_data(graph, path_name);

    let path_viewer = wasm_bindgen.PathViewer.new(coord_sys, data, 512,
                                                  { r: 0.8, g: 0.3, b: 0.3, a: 1.0 },
                                                  { r: 0.2, g: 0.2, b: 0.2, a: 1.0 });
                                                  

    _state = { path_name, path_viewer };

    console.log(_state);

    _graph = graph;
    console.log("worker node count: " + _graph.node_count());


    Comlink.expose({
        connect_canvas(offscreen_canvas) {
            console.log("hello??");
            // _state.path_viewer.transfer_canvas_control_to_self(canvas
            console.log("set_canvas");
            _state.path_viewer.set_canvas(offscreen_canvas);
            console.log("sample_range");
            _state.path_viewer.sample_range(0, 10000);
            console.log("draw_to_canvas");
            _state.path_viewer.draw_to_canvas();
            // console.log("canvas_test");
            // _state.path_viewer.canvas_test();
        },
        // sample_data(left, right, bin_count) {
        //     _state.path_viewer.sample_range(left, right);
        //     // let data = _state.path_viewer.get_bin_data();
        //     return;
        // },
        get_graph() {
            return Comlink.proxy(_graph);
        }
    });

    postMessage("graph-ready");
}

run();

/*

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
*/

// Comlink.expose {
//     __graph,
//     node_count() {
//     }
// }

