import init_module, * as wasm_bindgen from './pkg/web.js';

/*
  wgpu/webgl seems to be troublesome with workers, and it's not clear
  what the current state of things is, exactly.
  The GraphViewer class initializes its own wasm module (same code as the rest of the web app),
  sharing the memory initialized in `main_worker.js`, but lives on the main thread,
  and takes care of rendering the 2D graph view.
*/

/*
class GraphViewer {
    constructor(wasm, memory) {
        let wasm = init_module(undefined, memory);
        this.wasm = wasm;
    }

    test_function() {
        wasm_bindgen.test_hello_world();
    }
}

export { GraphViewer,  };

*/

export async function initGraphViewer(memory) {
    let wasm = init_module(undefined, memory);
}
