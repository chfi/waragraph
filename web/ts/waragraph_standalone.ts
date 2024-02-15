import init_module, * as wasm_bindgen from 'waragraph';

import * as Comlink from 'comlink';

import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

import { PathViewer, addOverviewEventHandlers, addPathViewerLogic, addPathViewerLogicClient, initializePathViewer, initializePathViewerClient } from './path_viewer_ui';
import { Viewport1D, globalSequenceTrack } from './waragraph';

import { tableFromIPC, tableFromArrays } from 'apache-arrow';
import { CoordSysArrow, CoordSysInterface } from './coordinate_system';
import { GraphViewer, graphViewerFromData } from './graph_viewer';
import { ArrowGFA, GraphLayout, PathIndex, serverAPIs, standaloneAPIs } from './graph_api';

import { addViewRangeInputListeners, appendPathListElements, appendSvgViewport, updateSVGMasks } from './dom';
import { OverviewMap } from './overview';

import * as chroma from 'chroma-js';
import { PathId } from './types';
import Split from 'split-grid';
import { initializeBedSidebarPanel } from './sidebar-bed';
import { vec2 } from 'gl-matrix';
import { GraphLayoutTable } from './graph_layout';
import { export2DViewportSvg } from './svg_export';
import { WaragraphWorkerCtx } from './worker';




// input should be options including files; lightweight version of the Options in waragraph.ts, roughly
export async function initializeWaragraphStandalone(
  gfa: URL | string | Blob,
  graph_layout?: URL | string | Blob,
) {
  const wasm = await init_module();

  const WorkerCtx: Comlink.Remote<typeof WaragraphWorkerCtx> = Comlink.wrap(
    new Worker(new URL("worker.ts", import.meta.url), { type: 'module' }));

  const waragraph_worker: Comlink.Remote<WaragraphWorkerCtx> =
    await new WorkerCtx((init_module as any).__wbindgen_wasm_module, wasm.memory);

  await waragraph_worker.loadGraph(gfa);

  // create ArrowGFA and PathIndex API providers via the worker

  const graph_apis = await standaloneAPIs(waragraph_worker);

  // create TS coordinate system (global)...

  const global_cs = await waragraph_worker.getGlobalCoordinateSystem();

  const global_viewport = new Viewport1D(global_cs as CoordSysInterface);

  console.warn(global_cs);
  // then it should be pretty much the same as the rest of the init function in waragraph_client.ts

  const graph_depth = await waragraph_worker.getComputedGraphDataset('depth');
  
  // use color map from waragraph_client

  const path_name_col = document.getElementById('path-viewer-left-column')!;
  const path_data_col = document.getElementById('path-viewer-right-column')!;

  const path_viewers: Array<PathViewer> = [];

  const overview_slots = appendPathListElements(40, 'div', 'div');
  overview_slots.left.classList.add('path-list-header');
  overview_slots.right.classList.add('path-list-header');
  overview_slots.left.style.setProperty('top', '0px');
  overview_slots.right.style.setProperty('top', '0px');

  const overview_canvas = document.createElement('canvas');
  overview_canvas.style.setProperty('position', 'relative');
  overview_canvas.style.setProperty('overflow', 'hidden');
  overview_canvas.width = overview_slots.right.clientWidth;
  overview_canvas.height = overview_slots.right.clientHeight;
  overview_slots.right.append(overview_canvas);

  // const viewport_key = opts.path_viewers!.viewport;
  // const viewport = await this.get1DViewport({ name: viewport_key.name });

  const overview = new OverviewMap(overview_canvas, global_viewport.max);
  await addOverviewEventHandlers(overview, global_viewport);

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
  await addViewRangeInputListeners(global_viewport);


  const seq_slots = appendPathListElements(20, 'div', 'div');
  seq_slots.left.classList.add('path-list-header');
  seq_slots.right.classList.add('path-list-header');
  seq_slots.left.style.setProperty('top', '40px');
  seq_slots.right.style.setProperty('top', '40px');

  const seq_canvas = document.createElement('canvas');
  seq_canvas.width = seq_slots.right.clientWidth;
  seq_canvas.height = seq_slots.right.clientHeight;
  seq_canvas.style.setProperty('position', 'absolute');
  seq_canvas.style.setProperty('overflow', 'hidden');

  seq_slots.right.append(seq_canvas);

  let seg_seq_array = await graph_apis.arrowGFA.segmentSequencesArray();
  let seq_track = globalSequenceTrack(
    seg_seq_array,
    seq_canvas,
    global_viewport.subject
  );

  let paths = await graph_apis.arrowGFA.pathMetadata();

  const path_promises = paths.map(async (path) => {
    const color_below = { r: 1.0, g: 1.0, b: 1.0 };
    const color_above = wasm_bindgen.path_name_hash_color_obj(path.name);

    const viewer = await initializePathViewer(
      waragraph_worker,
      path.name,
      global_viewport,
      // data,
    );
    /*
    const viewer = await initializePathViewerClient(
      path.name,
      global_viewport, 
      base_url,
      "depth",
      0.5,
      color_below,
      color_above
    );
     */

    viewer.container.style.setProperty('flex-basis', '20px');
    viewer.container.path_viewer = viewer;

    path_data_col.append(viewer.container);

    await addPathViewerLogicClient(graph_apis.arrowGFA, graph_apis.pathIndex, viewer);

    viewer.onResize();
    // console.log(viewer);
    // viewer.isVisible = true;
    // viewer.sampleAndDraw(global_viewport.get());

    viewer.container.classList.add('path-list-flex-item');

    const name_el = document.createElement('div');
    name_el.classList.add('path-list-flex-item', 'path-name');
    name_el.innerHTML = viewer.pathName;

    path_name_col.append(name_el);

    path_viewers.push(viewer);
  });

  await Promise.all(path_promises);
}
