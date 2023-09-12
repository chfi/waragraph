
import init_wasm, * as wasm_bindgen from './pkg/web.js';
import * as Comlink from './comlink.mjs';
// importScripts('./pkg/web.js');
// importScripts("./comlink.js");


console.log(wasm_bindgen);
console.log(typeof wasm_bindgen);

let _graph;

let _state;

let wasm;



class PathViewerCtx {
    constructor(coord_sys, data, { bins, color_0, color_1}) {


        // TODO set view based on coord_sys, or take an optional argument



        let view = { left: 0, right: coord_sys.max() };
        this.path_viewer = wasm_bindgen.PathViewer.new(coord_sys, data, 512, color_0, color_1);
        this.view = view;
    }

    connectCanvas(offscreen_canvas) {
        this.path_viewer.set_canvas(offscreen_canvas);
    }

    setView(left, right) {
        this.view = { left, right };
    }

    sample() {
        this.path_viewer.sample_range(this.view.left, this.view.right);
        this.path_viewer.draw_to_canvas();
    }
    
}



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
    console.log("constructing coordinate system");
    let coord_sys = wasm_bindgen.CoordSys.global_from_arrow_gfa(graph);
    console.log("deriving depth data");
    let data = wasm_bindgen.arrow_gfa_depth_data(graph, path_name);

    console.log("initializing path viewer");

    let color_0 = 
        { r: 0.8, g: 0.3, b: 0.3, a: 1.0 };
    let color_1 =
        { r: 0.2, g: 0.2, b: 0.2, a: 1.0 };

    let opts = { bins: 512, color_0, color_1 };

    let path_viewer = new PathViewerCtx(coord_sys, data, opts);

    _state = { path_name, path_viewer };

    console.log(_state);

    _graph = graph;
    console.log("worker node count: " + _graph.segment_count());


    Comlink.expose({

        connectCanvas(offscreen_canvas) {
            _state.path_viewer.connectCanvas(offscreen_canvas);
        },

        sampleRange(left, right) {
            _state.path_viewer.setView(left, right);
            _state.path_viewer.sample();
        },

        // path_viewer: Comlink.proxy(_state.path_viewer),
        // connect_canvas(offscreen_canvas) {
        //     // _state.path_viewer.transfer_canvas_control_to_self(canvas
        //     console.log("set_canvas");
        //     _state.path_viewer.set_canvas(offscreen_canvas);
        //     console.log("sample_range");
        //     _state.path_viewer.sample_range(0, 10000);
        //     console.log("draw_to_canvas");
        //     _state.path_viewer.draw_to_canvas();
        //     // console.log("canvas_test");
        //     // _state.path_viewer.canvas_test();
        // },
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

