import init_module, * as wasm_bindgen from 'waragraph';

import type { WorkerCtxInterface } from './main_worker';

import type { WaragraphWorkerCtx } from './new_worker';

import type { Bp, Segment, Handle, PathIndex } from './types';

import {
  GraphViewer,
  initializeGraphViewer,
} from './graph_viewer';

import * as BedSidebar from './sidebar-bed';

import { wrapWasmPtr } from './wrap';

import * as Comlink from 'comlink';

import { BehaviorSubject } from 'rxjs';
import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

import Split from 'split-grid';

import { mat3, vec2 } from 'gl-matrix';


// there really should be a better name for this...
interface CoordinateSystem {
  coord_sys: wasm_bindgen.CoordSys;
}

interface View2D {
  center: vec2;
  size: vec2;
}

class Viewport2D {
  // not sure if it makes sense to store the segment positions as is in the viewport, even
  // though they are associated with it in a similar way to the 1D coordinate systems;
  // SegmentPositions is currently using owned Vecs, but here on the JS side we could share
  // however many pointers we want to it (it's immutable)
  segment_positions: wasm_bindgen.SegmentPositions;
  view: wasm_bindgen.View2D;
  subject: BehaviorSubject<View2D>; // doesn't make sense to use wasm pointers here

  constructor() {
  }

}

interface View1D {
  start: Bp;
  end: Bp;
}

// one instance of this would be shared by all 1D views that should be "synced up"
// including external tracks, eventually
class Viewport1D {
  coord_sys: wasm_bindgen.CoordSys;
  view: wasm_bindgen.View1D;
  subject: BehaviorSubject<View1D>;

  constructor(coord_sys: wasm_bindgen.CoordSys, view: wasm_bindgen.View1D) {
    this.coord_sys = coord_sys;
    this.view = view;

    let view_range = { start: view.start, end: view.end };

    this.subject = new BehaviorSubject(view_range as View1D);
  }

  segmentAtOffset(bp: Bp) {
    if (typeof bp === "number") {
      bp = BigInt(bp);
    }
    return this.coord_sys.segment_at_pos(bp);
  }

  segmentOffset(segment: Segment) {
    return this.coord_sys.offset_at(segment);
  }

  segmentRange(segment: Segment): { start: Bp, end: Bp } {
    return this.coord_sys.segment_range(segment) as { start: Bp, end: Bp };
  }

}

type ViewportDesc =
    { scope: "graph", name: string }
  | { scope: "path", path_name: string, name: string };

export class Waragraph {
  /*
  needs to hold coordinate systems, viewports, and data,
  in addition to the graph

  it should probably also have a reference to the wasm module & memory

  upon initialization, assuming only the graph is given (for now the graph
  will be mandatory and restricted to just one), all that should happen is:
   * worker pool initialized
   * graph parsed and loaded on worker
   * computable datasets table is set up with defaults


  the worker pool is probably a bit much to implement right now (i'm not 100%
  sure what the boundary looks like), so instead clean up and refine the worker module
  
  

   */

  worker_ctx: Comlink.Remote<WaragraphWorkerCtx>;

  graph: wasm_bindgen.ArrowGFAWrapped;
  path_index: wasm_bindgen.PathIndexWrapped;

  graph_viewer: GraphViewer | undefined;


  resize_obs: rxjs.Subject<unknown> | undefined;


  coordinate_systems: Map<string, wasm_bindgen.CoordSys>;
  viewports_1d: Map<string, Viewport1D>;
  

  // constructor(worker_ctx, graph_viewer) {
  constructor(worker_ctx, graph_ptr, path_index_ptr) {
    this.worker_ctx = worker_ctx;
    this.graph = wrapWasmPtr(wasm_bindgen.ArrowGFAWrapped, graph_ptr);
    this.path_index = wrapWasmPtr(wasm_bindgen.PathIndexWrapped, path_index_ptr);

    this.coordinate_systems = new Map;
    this.viewports_1d = new Map;
  }


  async get1DViewport(desc: ViewportDesc) {
    let viewport = this.viewports_1d.get(desc.name);

    if (viewport !== undefined) {
      return viewport;
    }

    let cs_key: string;
    if (desc.scope === "graph") {
      cs_key = desc.scope;
    } else {
      cs_key = desc.scope + ":" + desc.path_name;
    }

    const cs = await this.getCoordinateSystem(cs_key);

    if (cs === undefined) {
      // should probably throw
      return;
    }

    const view_max = cs.max();
    const view = wasm_bindgen.View1D.new(0, Number(view_max), view_max)

    viewport = new Viewport1D(cs, view);

    if (viewport !== undefined) {
      this.viewports_1d.set(desc.name, viewport);
    }

    return viewport;

  }

  async getCoordinateSystem(key: string): Promise<wasm_bindgen.CoordSys | undefined> {
    let cs = this.coordinate_systems.get(key);

    if (cs !== undefined) {
      return cs;
    }

    if (key === "graph") {
      cs = await this.worker_ctx.buildGlobalCoordinateSystem();
    } else if (key.startsWith("path:") && key.length > 5) {
      const path_name = key.substring(5);
      cs = await this.worker_ctx.buildPathCoordinateSystem(path_name);
    }

    if (cs !== undefined) {
      this.coordinate_systems.set(key, cs);
    }

    return cs;
  }


  async initializeTree(opts: WaragraphOptions) {
    this.resize_obs = new rxjs.Subject();

    // sidebar

    // await BedSidebar.initializeBedSidebarPanel(warapi);

    {
      // TODO: factor out overview & range input bits
      const overview_slots = appendPathListElements(40, 'div', 'div');

      /*
      const cs_view = await worker_obj.globalCoordSysView();
      const view_max = await cs_view.viewMax();
      // const view_subject = await cs_view.viewSubject();
      const overview_canvas = document.createElement('canvas');
      overview_canvas.style.setProperty('position', 'absolute');
      overview_canvas.style.setProperty('overflow', 'hidden');
      overview_canvas.width = overview_slots.right.clientWidth;
      overview_canvas.height = overview_slots.right.clientHeight;
      overview_slots.right.append(overview_canvas);
      const overview = new OverviewMap(overview_canvas, view_max);
      await addOverviewEventHandlers(overview, cs_view);
       */

      // range input
      const range_input = document.createElement('div');
      // range_input.classList.add('path-name');
      range_input.id = 'path-viewer-range-input';

      overview_slots.left.append(range_input);

      for (const id of ["path-viewer-range-start", "path-viewer-range-end"]) {
        const input = document.createElement('input');
        input.id = id;
        input.setAttribute('type', 'text');
        input.setAttribute('inputmode', 'numeric');
        input.setAttribute('pattern', '\d*');
        input.style.setProperty('height', '100%');
        range_input.append(input);
      }

      // TODO
      // await addViewRangeInputListeners(cs_view);

      // TODO: factor out sequence track bit maybe

      const seq_slots = appendPathListElements(20, 'div', 'div');

      const seq_canvas = document.createElement('canvas');
      seq_canvas.width = seq_slots.right.clientWidth;
      seq_canvas.height = seq_slots.right.clientHeight;
      seq_canvas.style.setProperty('position', 'absolute');
      seq_canvas.style.setProperty('overflow', 'hidden');

      seq_slots.right.append(seq_canvas);

      /*
      let view_subject = await cs_view.viewSubject();

      let graph = wrapWasmPtr(wasm_bindgen.ArrowGFAWrapped, graph_raw.__wbg_ptr);
      // let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
      let seq_track = globalSequenceTrack(
        graph,
        seq_canvas,
        view_subject
      );

      resize_obs.subscribe(() => {
        overview_canvas.width = overview_slots.right.clientWidth;
        overview_canvas.height = overview_slots.right.clientHeight;
        seq_canvas.width = seq_slots.right.clientWidth;
        seq_canvas.height = seq_slots.right.clientHeight;

        overview.draw();
        seq_track.draw_last();
      });
       */

    }

    // for (const path_name of names) {
    //   appendPathView(worker_obj, resize_obs, path_name);
    // }

    // TODO: additional tracks

    const split_root = Split({
      columnGutters: [{
        track: 1,
        element: document.querySelector('.gutter-column-sidebar'),
      }],
      onDragEnd: (dir, track) => {
        // graph_viewer.resize();
        resize_obs.next(null);
      },
    });

    const split_viz = Split({
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
        if (dir === "row" && track === 1) {
          // 2D view resize
          // graph_viewer.resize();
        } else if (dir === "column" && track === 1) {
          // 1D view resize
          resize_obs.next(null);
        }
      },
    });

    rxjs.fromEvent(window, 'resize').pipe(
      rxjs.throttleTime(100),
    ).subscribe(() => {
      // let svg = document.getElementById('svg-container');
      // if (svg) {

      // }
      // graph_viewer.resize();
      resize_obs.next(null);
    });
  }


}



export interface WaragraphOptions {
  gfa_url?: URL | string,
  graph_layout_url?: URL | string,

  // TODO: tree
}

interface ContainerElements {

}

// export async function initializeWaragraph({ } = {}): Waragraph {
export async function initializeWaragraph(opts: WaragraphOptions = {}) {
  const wasm = await init_module();

  const WaragraphWorkerCtx = Comlink.wrap(
    new Worker(new URL("new_worker.ts", import.meta.url), { type: 'module' }));

  const waragraph_worker: Comlink.Remote<WaragraphWorkerCtx> =
    await new WaragraphWorkerCtx((init_module as any).__wbindgen_wasm_module, wasm.memory);

  const { gfa_url,
    graph_layout_url
  } = opts;

  // initialize/prepare DOM



  await waragraph_worker.loadGraph(gfa_url);

  const graph_ptr = await waragraph_worker.getGraphPtr();
  const path_index_ptr = await waragraph_worker.getPathIndexPtr();

  const waragraph = new Waragraph(waragraph_worker, graph_ptr, path_index_ptr);

  const graph_px = await waragraph.worker_ctx.graphProxy();
  console.warn(graph_px);
  console.warn("!!!!!");

  console.warn(await graph_px.segment_count());

  // initialize 2D viewer

  // const graph_viewer = await initializeGraphViewer(wasm.memory, graph_raw, layout_path);


  // 1D viewer


  await waragraph.initializeTree(opts);

  //   }
  // };



  return waragraph;
}


function appendPathListElements(height, left_tag, right_tag) {
  const left = document.createElement(left_tag);
  const right = document.createElement(right_tag);

  const setStyles = (el) => {
    el.style.setProperty("flex-basis", height + "px");
  };

  setStyles(left);
  setStyles(right);

  document.getElementById("path-viewer-left-column")?.append(left);
  document.getElementById("path-viewer-right-column")?.append(right);

  return { left, right };
}


async function addViewRangeInputListeners(cs_view) {
  const start_el = document.getElementById('path-viewer-range-start') as HTMLInputElement;
  const end_el = document.getElementById('path-viewer-range-end') as HTMLInputElement;

  let init_view = await cs_view.get();

  start_el.value = init_view.start;
  end_el.value = init_view.end;

  start_el.addEventListener('change', (event) => {
    cs_view.set({ start: start_el.value, end: end_el.value });
  });

  end_el.addEventListener('change', (event) => {
    cs_view.set({ start: start_el.value, end: end_el.value });
  });

  const view_subject = await cs_view.viewSubject();

  view_subject.subscribe((view) => {
    start_el.value = String(Math.round(view.start));
    end_el.value = String(Math.round(view.end));
  });
}
