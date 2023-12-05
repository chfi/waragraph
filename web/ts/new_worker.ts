import init_wasm, * as wasm_bindgen from 'waragraph';

import type { WithPtr } from './wrap';

import * as Comlink from 'comlink';
import * as rxjs from 'rxjs';
import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

let wasm;

export class WaragraphWorkerCtx {
  graph: wasm_bindgen.ArrowGFAWrapped | undefined;
  path_index: wasm_bindgen.PathIndexWrapped | undefined;

  constructor(wasm_module, wasm_memory) {
    if (wasm === undefined) {
      // wasm = await init_wasm(undefined, wasm_memory);
      wasm = wasm_bindgen.initSync(wasm_module, wasm_memory);
      wasm_bindgen.set_panic_hook();
      console.warn("initialized wasm on worker");
    }

  }

  async loadGraph(gfa_url) {
    let gfa = fetch(gfa_url);
    let graph = await wasm_bindgen.load_gfa_arrow(gfa);
    let path_index = graph.generate_path_index();

    this.graph = graph;
    this.path_index = path_index;
  }

  getGraphPtr(): number {
    return (this.graph as wasm_bindgen.ArrowGFAWrapped & WithPtr).__wbg_ptr;
  }

  // graphProxy(): Comlink.Remote<wasm_bindgen.ArrowGFAWrapped> {
  graphProxy() {
    return Comlink.proxy(this.graph);
  }

  getPathIndexPtr(): number {
    return (this.path_index as wasm_bindgen.PathIndexWrapped & WithPtr).__wbg_ptr;
  }

  buildGlobalCoordinateSystem(): wasm_bindgen.CoordSys & WithPtr | undefined {
    if (this.graph) {
      return wasm_bindgen.CoordSys.global_from_arrow_gfa(this.graph) as wasm_bindgen.CoordSys & WithPtr;
    }
  }

  buildPathCoordinateSystem(path_name: string): wasm_bindgen.CoordSys & WithPtr | undefined {
    const path_id = this.graph?.path_index(path_name);

    if (this.graph && path_id) {
      const path_cs = wasm_bindgen.CoordSys.path_from_arrow_gfa(this.graph, path_id);
      return path_cs as wasm_bindgen.CoordSys & WithPtr;
    }
  }

  setGraphSegmentData(data_name, data) {
  }

  setPathSegmentData(data_name, data_values, data_indices: Uint32Array) {
  }

  createGraphViewer(
    container: HTMLDivElement,
    segment_colors: Uint32Array,
  ) {
  }

  createPathViewer(
    offscreen_canvas: OffscreenCanvas,
    path_name: string,
    data_id: "depth",
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


// first thing is to wait for the wasm memory (and compiled module)
// & initialize wasm_bindgen


declare var DedicatedWorkerGlobalScope: any;

// TODO this (and other) worker files need to be in a separate folder
// with its own tsconfig.json, with `lib` including `webworker` but not `dom`
if (DedicatedWorkerGlobalScope) {

  // Comlink.expose({ initWorkerCtx });

  Comlink.expose(WaragraphWorkerCtx);

  /*
  self.onmessage = async (event) => {
    self.onmessage = undefined;
    // console.log(event);
    // console.log("received message");
    // console.log(typeof event.data);
    // console.log(event.data);

    wasm = await init_wasm(undefined, event.data);
    wasm_bindgen.set_panic_hook();

    // TODO create & expose WaragraphWorker

    // const wg_worker = new WaragraphWorker();
    // Comlink.expose(wg_worker);
    
  }
  */


}


/*
async function initWorkerCtx(wasm_memory: WebAssembly.Memory): Promise<WaragraphWorkerCtx> {
  wasm = await init_wasm(undefined, wasm_memory);
  wasm_bindgen.set_panic_hook();

  // const ctx = new WaragraphWorkerCtx();

  console.warn(ctx);
  return ctx;
}
  */



