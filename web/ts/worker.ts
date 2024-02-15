import init_wasm, * as wasm_bindgen from 'waragraph';

import { wrapWasmPtr, type WithPtr } from './wrap';
import type { Bp, Segment, Handle, PathId, RGBObj } from './types';

import * as Comlink from 'comlink';
import * as rxjs from 'rxjs';
import * as handler from './transfer_handlers';
import { PathMetadata } from './graph_api';
import { CoordSysArrow, coordSysFromBuffers } from './coordinate_system';
handler.setTransferHandlers(rxjs, Comlink);

let wasm;


export class WaragraphWorkerCtx {
  graph: wasm_bindgen.ArrowGFAWrapped | undefined;
  path_index: wasm_bindgen.PathIndexWrapped | undefined;


  global_coord_sys: CoordSysArrow;
  path_coord_sys_cache: Map<string, CoordSysArrow>;

  constructor(wasm_module, wasm_memory) {
    if (wasm === undefined) {
      // wasm = await init_wasm(undefined, wasm_memory);
      wasm = wasm_bindgen.initSync(wasm_module, wasm_memory);
      wasm_bindgen.set_panic_hook();
      console.warn("initialized wasm on worker");
    }

    this.path_coord_sys_cache = new Map();

  }

  async loadGraph(gfa: URL | string | Blob) {
    let graph: wasm_bindgen.ArrowGFAWrapped | undefined;

    if (gfa instanceof Blob) {
      graph = await wasm_bindgen.load_gfa_arrow_blob(gfa);
    } else {
      let gfa_data = fetch(gfa);
      graph = await wasm_bindgen.load_gfa_arrow_response(gfa_data);
    }

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

  // buildGlobalCoordinateSystem(): wasm_bindgen.CoordSys & WithPtr | undefined {
  getGlobalCoordinateSystem(): CoordSysArrow | undefined {
    if (this.global_coord_sys !== undefined) {
      return this.global_coord_sys;
    }

    if (this.graph) {
      const [node_order, step_offsets] =
        wasm_bindgen.CoordSys.global_from_arrow_gfa(this.graph).to_shared_arrays();
      const csys = coordSysFromBuffers(node_order, step_offsets);

      this.global_coord_sys = csys;
      return csys;
    }
  }

  getPathCoordinateSystem(path: string | number): CoordSysArrow | undefined {
    let path_name: string;

    if (typeof path === "number") {
      path_name = this.graph!.path_name(path);
    } else {
      path_name = path;
    }

    let csys = this.path_coord_sys_cache.get(path_name);

    if (csys !== undefined) {
      return csys;
    }

    const path_id = this.graph?.path_id(path_name);

    if (this.graph && path_id) {

      const [node_order, step_offsets] =
        wasm_bindgen.CoordSys.path_from_arrow_gfa(this.graph, path_id).to_shared_arrays();
      const path_cs = coordSysFromBuffers(node_order, step_offsets);

      this.path_coord_sys_cache.set(path_name, path_cs);
      return path_cs;
    }
  }

  createPathViewer(
    offscreen_canvas: OffscreenCanvas,
    coord_sys_: WithPtr,
    path_name: string,
    data: wasm_bindgen.SparseData,
    threshold: number,
    color_below: RGBObj,
    color_above: RGBObj,
  ) {

    const coord_sys = wrapWasmPtr(wasm_bindgen.CoordSys, coord_sys_.__wbg_ptr) as wasm_bindgen.CoordSys;

    const data_wrapped = wrapWasmPtr(wasm_bindgen.SparseData, data.__wbg_ptr);

    // TODO configurable bins
    const viewer_ctx = new PathViewerCtx(coord_sys, data_wrapped, { bins: 1024, color_0: color_below, color_1: color_above });

    viewer_ctx.connectCanvas(offscreen_canvas);

    return Comlink.proxy(viewer_ctx);
  }

  getComputedGraphDataset(
    dataset: "depth" | "test", // TODO expand/move to type; later use registered callbacks
  ): Uint32Array {
  // ): ArrayBufferView {
    // TODO compute depth
    // path index matrix probably

    // still need to map this to colors!

    let colors;

    if (dataset === "depth") {
      // TODO apply color map -- this is a vector of 32 bit integers,
      // but the colors are a vector of 4 8-bit color channels
      colors = this.graph.graph_depth_vector();
    } else if (dataset === "test") {
      colors = new Uint32Array(this.graph?.segment_count());
      colors.fill(0xAAAAAAFF);
    }

    return Comlink.transfer(colors, [colors.buffer]);
  }

  getComputedPathDataset(
    dataset: "depth",
    path_name: string,
  ): wasm_bindgen.SparseData & WithPtr | undefined {

    if (this.graph) {
      return wasm_bindgen.arrow_gfa_depth_data(this.graph, path_name);
    }
  }

  segmentSequencesArray(): Uint8Array {
    return this.graph!.segment_sequences_array();
  }

  pathIdFromName(name: string): number | undefined {
    try {
      return this.graph?.path_id(name);
    } catch (e) {
      return undefined;
    }
  }
  
  pathNameFromId(id: number): string | undefined {
    try {
      return this.graph?.path_name(id);
    } catch (e) {
      return undefined;
    }
  }
  
  pathMetadata(): [PathMetadata] {
    return this.graph.path_metadata();
  }
  
  pathSteps(id: number): Uint32Array | undefined {
    let name = this.pathNameFromId(id);
    return this.graph?.path_steps(name);
  }
  
  segmentAtPathPosition(path: PathId, pos: Bp): number | undefined {
    const csys = this.buildPathCoordinateSystem(path)
    return csys?.segment_at_pos(BigInt(pos))
  }
  
  segmentAtGlobalPosition(pos: Bp): number | undefined {
    const csys = this.buildGlobalCoordinateSystem();
    return csys?.segment_at_pos(BigInt(pos));
  }
  
  segmentGlobalRange(segment: number): { start: bigint, end: bigint } | undefined {
    // really needs just the sequences array
    const csys = this.buildGlobalCoordinateSystem();
    return csys?.segment_range(segment);
  }

  pathsOnSegment(segment: number): Uint32Array | undefined {
    return this.path_index?.paths_on_segment(segment);
  }

}



export class PathViewerCtx {
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

  forceRedraw(resample?: boolean) {
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
  Comlink.expose(WaragraphWorkerCtx);
}

