import init_wasm, * as wasm_bindgen from 'waragraph';

let wasm;


// first thing is to wait for the wasm memory (and compiled module)
// & initialize wasm_bindgen








declare var DedicatedWorkerGlobalScope: any;

// TODO this (and other) worker files need to be in a separate folder
// with its own tsconfig.json, with `lib` including `webworker` but not `dom`
if (DedicatedWorkerGlobalScope) {
  import('./transfer_handlers.js').then((handler) => {
    handler.setTransferHandlers(rxjs, Comlink);
    postMessage("WORKER_INIT");
  });

  self.onmessage = async (event) => {
    self.onmessage = undefined;
    // console.log(event);
    // console.log("received message");
    // console.log(typeof event.data);
    // console.log(event.data);

    wasm = await init_wasm(undefined, event.data);
    wasm_bindgen.set_panic_hook();

    // TODO create & expose WaragraphWorker
  }
}




class WaragraphWorker {
  graph: wasm_bindgen.ArrowGFAWrapped | undefined;
  path_index: wasm_bindgen.PathIndexWrapped | undefined;

  async loadGraph(gfa_url) {
    let gfa = fetch(gfa_url);
    let graph = await wasm_bindgen.load_gfa_arrow(gfa);
    let path_index = graph.generate_path_index();

    this.graph = graph;
    this.path_index = path_index;
  }

  getGraph() {
    return this.graph;
  }

  getPathIndex() {
    return this.path_index;
  }

  createPathViewer(
    offscreen_canvas: OffscreenCanvas,
    path_name: string,
    data_values: Float32Array,
    data_indices: Uint32Array,
  ) {
    // TODO wrap data in SparseData to be consumed by PathViewerCtx

  }


}






class PathViewerCtx {
  path_viewer: wasm_bindgen.PathViewer;
  coord_sys: wasm_bindgen.CoordSys;

  view: { start: number, end: number } | null;

  constructor(coord_sys, data, { bins, color_0, color_1 }) {
    this.path_viewer = wasm_bindgen.PathViewer.new(coord_sys, data, bins, color_0, color_1);
    this.coord_sys = coord_sys;
    this.view = null;
  }

  connectCanvas(offscreen_canvas) {
    console.log(offscreen_canvas);
    this.path_viewer.set_target_canvas(offscreen_canvas);
  }

  setCanvasWidth(width) {
    this.path_viewer.set_offscreen_canvas_width(width);
  }

  forceRedraw(resample) {
    if (resample && this.view !== null) {
      this.path_viewer.sample_range(this.view.start, this.view.end);
    }
    this.path_viewer.draw_to_canvas();
  }

  resizeTargetCanvas(width: number, height: number) {
    const valid = (v) => Number.isInteger(v) && v > 0;
    if (valid(width) && valid(height)) {
      this.path_viewer.resize_target_canvas(width, height);
    }
  }

  coordSys() {
    return this.path_viewer.coord_sys;
  }

  setView(start: number, end: number) {
    this.view = { start, end };
  }

  sample() {
    if (this.view !== null) {
      this.path_viewer.sample_range(this.view.start, this.view.end);
    }
  }

}
