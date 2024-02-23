import init_module, * as wasm_bindgen from 'waragraph';

import 'bootstrap/dist/css/bootstrap.min.css';

import type { WaragraphWorkerCtx, PathViewerCtx } from './worker';

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui';
import type { PathViewer } from './path_viewer_ui';
import { OverviewMap } from './overview';

import * as CanvasTracks from './canvas_tracks';
import * as BedSidebar from './sidebar-bed';

import type { Bp, Segment, Handle, PathId, RGBAObj, RGBObj } from './types';

import {
  GraphViewer,
} from './graph_viewer';

import { type WithPtr, wrapWasmPtr } from './wrap';

import * as Comlink from 'comlink';

import { BehaviorSubject } from 'rxjs';
import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

import Split from 'split-grid';

import { mat3, vec2 } from 'gl-matrix';
import { CoordSysInterface } from './coordinate_system';
import { ArrowGFA, PathIndex } from './graph_api';
import { addViewRangeInputListeners, appendPathListElements, appendSvgViewport, updateSVGMasks } from './dom';
import { GraphLayoutTable } from './graph_layout';
import { export2DViewportSvg } from './svg_export';

export interface View1D {
  start: number;
  end: number;
}

// one instance of this would be shared by all 1D views that should be "synced up"
// including external tracks, eventually
export class Viewport1D {
  // coord_sys: wasm_bindgen.CoordSys;
  coord_sys: CoordSysInterface;
  view: wasm_bindgen.View1D;
  subject: BehaviorSubject<View1D>;

  constructor(coord_sys: CoordSysInterface, view?: wasm_bindgen.View1D) {
    this.coord_sys = coord_sys;

    if (view) {
      this.view = view;
    } else {
      const max = coord_sys.max();
      this.view = wasm_bindgen.View1D.new(0, max, BigInt(max))
    }

    let view_range = { start: this.view.start, end: this.view.end };
    this.subject = new BehaviorSubject(view_range as View1D);
  }

  get length() {
    return this.view.len;
  }

  get max() {
    return this.view.max;
  }

  get(): View1D {
    return this.subject.value;
  }

  set(start: Bp, end: Bp) {
    let s = Number(start);
    let e = Number(end);

    this.view.set(s, e);
    this.subject.next({ start: s, end: e });
  }

  push() {
    let start = this.view.start;
    let end = this.view.end;
    this.subject.next({ start, end });
  }

  zoomNorm(norm_x, scale) {
    this.view.zoom_with_focus(norm_x, scale);
    this.push();
  }

  zoomViewCentered(scale) {
    this.view.zoom_with_focus(0.5, scale);
    this.push();
  }

  translateView(delta_bp) {
    this.view.translate(delta_bp);
    console.log("translating view");
    this.push();
  }

  centerAt(bp) {
    // console.log("centering view around " + bp);
    let len = this.view.len;
    let start = bp - (len / 2);
    let end = bp + (len / 2);
    // console.log("left: " + left + ", right: " + right);
    this.view.set(start, end);
    this.push();
  }

  segmentAtOffset(bp: Bp) {
    if (typeof bp === "number") {
      bp = BigInt(bp);
    }
    return this.coord_sys.segmentAtOffset(bp);
  }

  segmentOffset(segment: Segment) {
    return this.coord_sys.segmentOffset(segment);
  }

  segmentRange(segment: Segment): { start: Bp, end: Bp } {
    return this.coord_sys.segmentRange(segment) as { start: Bp, end: Bp };
  }

}
        
/*
      function resizeView() {
        const doc_bounds = document.documentElement.getBoundingClientRect();
        // const win_width = window.innerWidth;
        const bounds = path_data_col.getBoundingClientRect();
        var width = doc_bounds.right - bounds.left;
        // account for scroll bar
        const containter = document.getElementById('path-viewer-container');
        const border = document.getElementById('1d-border');

        const isChrome = /Chrome/.test(navigator.userAgent) && /Google Inc/.test(navigator.vendor);

        if (isChrome) { // handles ugly chrome scroll bars
          if (containter.scrollHeight > containter.clientHeight) {
            width = width - 19;
          }
          else {  // gets the view off the side of the screen a bit
            width = width - 3;
          }
        }
        else { 
          width = width - 3;
        }
        
        overview_canvas.width = width;
        seq_canvas.width = width;
        // overview_canvas.width = overview_slots.right.clientWidth;
        // seq_canvas.width = seq_slots.right.clientWidth;

        // overview_canvas.height = overview_slots.right.clientHeight;
        // seq_canvas.height = seq_slots.right.clientHeight;
        overview.draw();
        seq_track.draw_last();
      }
*/

export class Waragraph {
  graph_viewer: GraphViewer | undefined;
  path_viewers: Array<PathViewer>;

  graph: ArrowGFA;
  // path_index: PathIndex;

  graphLayoutTable: GraphLayoutTable | undefined;

  global_viewport: Viewport1D;

  resize_observable: rxjs.Subject<void>;
  intersection_observer: IntersectionObserver | undefined;

  api_base_url: URL | undefined;

  // only used for SVG export
  // kinda hacky but fine for now; might copy the buffers back from GPU when needed later
  color_buffers: Map<string, Uint32Array>;

  constructor(
    viewers: { graph_viewer?: GraphViewer, path_viewers: Array<PathViewer> },
    graph: ArrowGFA,
    global_viewport: Viewport1D,
    layout: 
    { graphLayoutTable?: GraphLayoutTable },
    base_url?: URL,
  ) {
    this.graph_viewer = viewers.graph_viewer;
    this.path_viewers = viewers.path_viewers;
    this.graph = graph;
    this.global_viewport = global_viewport;
    this.graphLayoutTable = layout.graphLayoutTable;

    this.color_buffers = new Map();

    console.warn(`setting api_base_url to ${base_url}`);
    this.api_base_url = base_url;

    this.resize_observable = new rxjs.Subject();

    this.intersection_observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if ("path_viewer" in entry.target) {
          const viewer = entry.target.path_viewer as PathViewer;
          const shouldRefresh = !viewer.isVisible && entry.isIntersecting;
          viewer.isVisible = entry.isIntersecting;

          if (shouldRefresh) {
            viewer.sampleAndDraw();
          }
        }
      });
    },
      { root: document.getElementById('path-viewer-container') }
    );

    for (const viewer of this.path_viewers) {
      this.intersection_observer.observe(viewer.container);
    }

    const _split_root = Split({
      columnGutters: [{
        track: 1,
        element: document.querySelector('.gutter-column-sidebar'),
      }],
      onDragEnd: (dir, track) => {
        // graph_viewer.resize();
        console.warn("resizing split!");
        this.resize_observable.next();
      },
    });

    const _split_viz = Split({
      rowGutters: [{
        track: 1,
        element: document.querySelector('.gutter-row-1')
      }],
      columnGutters: [{
        track: 1,
        element: document.querySelector('.gutter-column-1')
      }],
      rowMinSizes: { 0: 200 },
      onDragEnd: (dir, track) => {
        console.warn("resizing split!");
        this.resize_observable.next();
      },
    });

    rxjs.fromEvent(window, 'resize')
      .subscribe(() => {
        this.resize_observable.next();
      });

    this.resize_observable.subscribe(() => {
      this.graph_viewer?.resize();
      for (const viewer of this.path_viewers) {
        viewer.onResize();
      }

      const doc_bounds = document.documentElement.getBoundingClientRect();
      // const win_width = window.innerWidth;
      const bounds = document.getElementById('path-viewer-right-column').getBoundingClientRect();
      const isChrome = /Chrome/.test(navigator.userAgent) && /Google Inc/.test(navigator.vendor);

      const container = document.getElementById('path-viewer-container')!;

      let width = doc_bounds.right - bounds.left;

      // handles ugly chrome scroll bars
      if (isChrome && container.scrollHeight > container.clientHeight) {
        width = width - 19;
      } else {
        // gets the view off the side of the screen a bit
        width = width - 3;
      }

      updateSVGMasks();
    });

  }
  

  export2DSVG() {

    // const options = {
    //   min_world_length: // derive from viewport size & (desired?) canvas size
    // };

    // const color = (_) => {
    //   return { r: 0.1, g: 0.7, b: 0.0, a: 1.0 };
    // };

    const color_buf = this.color_buffers.get('depth');

    const color = (seg: number) => {
      let val = color_buf.at(seg);
      let r = ((val >> 0) & 0xFF) / 255;
      let g = ((val >> 8) & 0xFF) / 255;
      let b = ((val >> 16) & 0xFF) / 255;
      let a = ((val >> 24) & 0xFF) / 255;
      return { r, g, b, a };
    };

    let svg = export2DViewportSvg(this.graph_viewer, this.graphLayoutTable, color);
    console.log(svg);

    if (svg instanceof SVGSVGElement) {
      svg.setAttribute('xmlns', 'http://www.w3.org/2000/svg');

      // save to file
      const blob = new Blob([svg.outerHTML], { type: 'image/svg+xml;charset=utf-8' });
      const url = URL.createObjectURL(blob);

      const downloadLink = document.createElement('a');
      downloadLink.href = url;
      downloadLink.download = "waragraph.svg";

      document.body.appendChild(downloadLink);
      downloadLink.click();

      document.body.removeChild(downloadLink);
      URL.revokeObjectURL(url);
    }
  }

  async segmentScreenPos2d(segment: number) {
    if (this.graph_viewer === undefined) {
      return;
    }

    let world_pos = this.graphLayoutTable!.segmentPosition(segment);

    if (!world_pos) {
      return;
    }

    let mat = this.graph_viewer!.getViewMatrix();

    let p0 = vec2.create();
    let p1 = vec2.create();
    vec2.transformMat3(p0, world_pos.p0, mat);
    vec2.transformMat3(p1, world_pos.p1, mat);

    return { p0, p1 };
  }

  globalBpToPathViewerPos(path_name: string, global_bp: number) {
    const el = document.getElementById('viewer-' + path_name);

    if (!el) {
      return null;
    }

    let el_rect = el.getBoundingClientRect();

    let viewport = this.global_viewport;

    let view = viewport.get();
    let view_len = viewport.length;

    let x_norm = (global_bp - view.start) / view_len;

    let width = el_rect.width;
    let y0 = el_rect.y;
    let y1 = el_rect.y + el_rect.height;

    let x = el_rect.left + x_norm * width;

    return { x, y0, y1 };
  }

  async segmentScreenPos1d(path: PathId | string, segment: number) {
    let path_name: string | undefined;

    if (typeof path === 'string') {
      path_name = path;
    } else {
      path_name = await this.graph.pathNameFromId(path);
    };


    // let seg_range = viewport.segmentRange(segment);
    // TODO this should probably be done in TS instead; store the coordinate systems
    // that are *used for 1D visualizations* as arrow tables in TS & compute on those
    const seg_range = await this.graph.segmentGlobalRange(segment);

    let el = document.getElementById('viewer-' + path_name);

    if (!el || !seg_range) {
      return null;
    }

    let el_rect = el.getBoundingClientRect();

    let viewport = this.global_viewport;

    let view = viewport.get();
    let view_len = viewport.length;

    // segmentRange returns BigInts
    let seg_s = Number(seg_range.start);
    let seg_e = Number(seg_range.end);

    let seg_start = (seg_s - view.start) / view_len;
    let seg_end = (seg_e - view.start) / view_len;

    let width = el_rect.width;
    let y0 = el_rect.y;
    let y1 = el_rect.y + el_rect.height;

    let x0 = el_rect.left + seg_start * width;
    let x1 = el_rect.left + seg_end * width;

    return { x0, y0, x1, y1 };
  }

}




// export async function initializeWaragraph({ } = {}): Waragraph {
async function initializeWaragraph(opts: WaragraphOptions = {}) {
  const wasm = await init_module();

  const WorkerCtx: Comlink.Remote<typeof WaragraphWorkerCtx> = Comlink.wrap(
    new Worker(new URL("worker.ts", import.meta.url), { type: 'module' }));

  const waragraph_worker: Comlink.Remote<WaragraphWorkerCtx> =
    await new WorkerCtx((init_module as any).__wbindgen_wasm_module, wasm.memory);

  const { gfa,
  } = opts;

  if (gfa === undefined) {
    throw new Error("TODO: defer loading GFA");
  }



  await waragraph_worker.loadGraph(gfa);

  const graph_ptr = await waragraph_worker.getGraphPtr();
  const path_index_ptr = await waragraph_worker.getPathIndexPtr();

  const waragraph = new Waragraph(wasm, waragraph_worker, graph_ptr, path_index_ptr);

  const graph_px = await waragraph.worker_ctx.graphProxy();


  if (opts.graph_viewer !== undefined) {
    // initialize 2D viewer

    const cfg = opts.graph_viewer;

    let data = cfg.data;

    if (typeof data === "string") {
      if (data === "depth" || data === "test") {

        data = await waragraph.worker_ctx.getComputedGraphDataset(data);
      } else {
        throw `Unknown data '${data}'`;
      }
    }

    waragraph.createGraphViewer(cfg.graph_layout, data);
  }

  if (opts.path_viewers !== undefined) {
    // initialize 1D viewers

    const cfg = opts.path_viewers;

    const viewport = await waragraph.get1DViewport({
      scope: 'graph',
      name: cfg.viewport.name
    });

    console.warn(viewport);

    if (viewport === undefined) {
      throw new Error("Viewport not found");
    }

    const path_names: string[] = [];

    if (cfg.path_names === '*') {
      // use all path names
      let path_metadata = await waragraph.graph.pathMetadata();

      path_metadata.forEach((path) => {
        path_names.push(path.name);
      });
    } else {
      for (const path_name of cfg.path_names) {
        path_names.push(path_name);
      }
    }

    const data_key = opts.path_viewers.data;

    if (data_key !== 'depth') {
      throw "unsupported path data";
    }

    // if (data_key === 'depth') {
    // } else {
    // TODO support custom data
    // }

    // const path_data_arrays = [];

    for (const path_name of path_names) {
      let data = await waragraph.worker_ctx.getComputedPathDataset(data_key, path_name);

      console.warn("Computed dataset:");
      console.warn(data);

      if (data === undefined) {
        throw new Error(`Computed path data ${data_key} not found (for path ${path_name})`);
      }

      data = wrapWasmPtr(wasm_bindgen.SparseData, data.__wbg_ptr);

      const color_above = reifyColor(opts.path_viewers.color_above, path_name);
      const color_below = reifyColor(opts.path_viewers.color_below, path_name);

      const path_viewer = await waragraph.createPathViewer(
        path_name,
        viewport,
        data,
        opts.path_viewers.threshold,
        color_below,
        color_above
      );

      waragraph.path_viewers.push(path_viewer);

    }


  }

  // add the viewer elements to the DOM
  await waragraph.initializeTree(opts);

  return waragraph;
}




export function globalSequenceTrack(
  // graph: wasm_bindgen.ArrowGFAWrapped,
  seq_array: Uint8Array,
  canvas: HTMLCanvasElement,
  view_subject: rxjs.Subject<View1D>
) {

  const min_px_per_bp = 8.0;

  let last_view = null;

  const draw_view = (view) => {
    let view_len = view.end - view.start;
    let px_per_bp = canvas.width / view_len;
    let ctx = canvas.getContext('2d');
    ctx?.clearRect(0, 0, canvas.width, canvas.height);

    if (px_per_bp > min_px_per_bp) {
      let seq = seq_array.slice(view.start, view.end);
      let subpixel_offset = view.start - Math.trunc(view.start);
      CanvasTracks.drawSequence(canvas, seq, subpixel_offset);

      last_view = view;
    }
  };

  view_subject.pipe(
    rxjs.distinct(),
    rxjs.throttleTime(10)
  ).subscribe((view) => {
    requestAnimationFrame((time) => {
      draw_view(view);
    });
  });

  const draw_last = () => {
    if (last_view !== null) {
      draw_view(last_view);
    }
  };

  return { draw_last };
}
