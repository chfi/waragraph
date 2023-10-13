import init_module, * as wasm_bindgen from './pkg/web.js';

/*
  wgpu/webgl seems to be troublesome with workers, and it's not clear
  what the current state of things is, exactly.
  The GraphViewer class initializes its own wasm module (same code as the rest of the web app),
  sharing the memory initialized in `main_worker.js`, but lives on the main thread,
  and takes care of rendering the 2D graph view.
*/

let wasm;

class GraphViewer {
    constructor() {
        // maybe just take the minimum raw data needed here
    }

    test_function() {
        console.log("in test_function");
        wasm_bindgen.test_hello_world();
        console.log("after call to test_hello_world");

        let val = wasm_bindgen.get_answer();
        console.log("answer: " + val);
    }
}

export { GraphViewer };

export async function initGraphViewer(memory) {
    console.log(">>>>>>>>>> in initGraphViewer");
    if (wasm === undefined) {
        wasm = await init_module(undefined, memory);
    }

    return new GraphViewer();
}
