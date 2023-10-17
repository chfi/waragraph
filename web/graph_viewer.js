import init_module, * as wasm_bindgen from './pkg/web.js';

/*
  wgpu/webgl seems to be troublesome with workers, and it's not clear
  what the current state of things is, exactly.
  The GraphViewer class initializes its own wasm module (same code as the rest of the web app),
  sharing the memory initialized in `main_worker.js`, but lives on the main thread,
  and takes care of rendering the 2D graph view.
*/

let wasm;
let _raving_ctx;

class GraphViewer {
    constructor(viewer) {
        // maybe just take the minimum raw data needed here
        this.graph_viewer = viewer;
    }

    draw() {
        this.graph_viewer.draw_to_surface(_raving_ctx);
    }
}

export { GraphViewer };

let _wasm;

// initializing raving/wgpu works when done here, but not when
// using the wasm memory shared from the worker
export async function initGraphViewer(wasm_mem, graph, layout_url) {
    console.log(">>>>>>>>>> in testRavingCtx");
    if (_wasm === undefined) {
        console.log("initializing with memory: ");
        console.log(wasm_mem);
        console.log(wasm_mem.buffer.byteLength);
        _wasm = await init_module(undefined, wasm_mem);
        wasm_bindgen.set_panic_hook();
    }

    if (_raving_ctx === undefined) {
        console.log("initializing raving ctx");

        let canvas = document.getElementById('graph-viewer-2d');

        _raving_ctx = await wasm_bindgen.RavingCtx.initialize_(canvas);
    }

    console.log("creating segment positions");

    // let layout_tsv = await fetch("./data/A-3105.layout.tsv").then(l => l.text());
    let layout_tsv = await fetch(layout_url).then(l => l.text());
    let seg_pos = wasm_bindgen.SegmentPositions.from_tsv(layout_tsv);

    console.log("created segment positions");

    let _graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph.__wbg_ptr);
    let seg_count = _graph.segment_count();
    console.log("segment count: " + seg_count);

    let canvas = document.getElementById("graph-viewer-2d");
    // let offscreen_canvas = canvas.transferControlToOffscreen();

    let viewer = wasm_bindgen.GraphViewer.new_dummy_data(_raving_ctx,
          _graph,
          seg_pos,
          canvas);

    viewer.draw_to_surface(_raving_ctx);

    return new GraphViewer(viewer);
}
