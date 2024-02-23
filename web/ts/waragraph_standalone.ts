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

import { addSegmentJumpInputListeners, addViewRangeInputListeners, appendPathListElements, appendSvgViewport, updateSVGMasks } from './dom';
import { OverviewMap } from './overview';

import * as chroma from 'chroma-js';
import { PathId, PathInterval } from './types';
import Split from 'split-grid';
import { initializeBedSidebarPanel } from './sidebar-bed';
import { vec2 } from 'gl-matrix';
import { GraphLayoutTable, graphLayoutFromTSV } from './graph_layout';
import { export2DViewportSvg } from './svg_export';
import { WaragraphWorkerCtx } from './worker';
import { applyColorScaleToBuffer, spectralScale } from './color';
import { Waragraph } from './waragraph';
import { AnnotationGeometry } from './annotations';




// input should be options including files; lightweight version of the Options in waragraph.ts, roughly
export async function initializeWaragraphStandalone(
  gfa: URL | string | Blob,
  graph_layout: Blob,
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

  console.warn("preparing global coordinate system");
  const global_cs = await waragraph_worker.getGlobalCoordinateSystem();
    // .then((cs) => new CoordSysArrow(cs.table));

  console.warn(typeof global_cs);
  console.warn(global_cs instanceof CoordSysArrow);
  console.warn(global_cs);
  console.warn(global_cs.max());

  console.warn("preparing global viewport");

  const global_viewport = new Viewport1D(global_cs as CoordSysInterface);

  // then it should be pretty much the same as the rest of the init function in waragraph_client.ts
  
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
    const path_data =
      await waragraph_worker.getComputedPathDataset('depth', path.name);

    console.warn(path_data);

    const color = 
      wasm_bindgen.path_name_hash_color_obj(path.name);
    console.warn(color);

    const viewer = await initializePathViewer(
      waragraph_worker,
      path.name,
      global_viewport,
      path_data,
      0.5,
      { r: 1, g: 1, b: 1, a: 1.0 },
      color
    );

    viewer.container.style.setProperty('flex-basis', '20px');
    viewer.container.path_viewer = viewer;

    path_data_col.append(viewer.container);

    await addPathViewerLogicClient(graph_apis.arrowGFA, graph_apis.pathIndex, viewer);

    viewer.onResize();

    viewer.container.classList.add('path-list-flex-item');

    const name_el = document.createElement('div');
    name_el.classList.add('path-list-flex-item', 'path-name');
    name_el.innerHTML = viewer.pathName;

    path_name_col.append(name_el);

    path_viewers.push(viewer);
  });

  await Promise.all(path_promises);

  // compute graph depth data colors for 2D
  const depth_data = await waragraph_worker.getComputedGraphDataset('depth');
  const depth_color = new Uint32Array(depth_data.length);
  applyColorScaleToBuffer(spectralScale, depth_data, depth_color)


  const graph_layout_table = await graphLayoutFromTSV(graph_layout);

  const graph_viewer = await graphViewerFromData(
    document.getElementById('graph-viewer-container'),
    graph_layout_table,
    depth_color
  );

  console.log(graph_viewer);
  graph_viewer.draw();

  console.log("creating Waragraph");
  const waragraph = new Waragraph(
    { graph_viewer, path_viewers },
    graph_apis.arrowGFA,
    global_viewport,
    { graphLayoutTable: graph_layout_table }
  );

  waragraph.color_buffers.set('depth', depth_color);

  waragraph.resize_observable.subscribe(() => {
    const doc_bounds = document.documentElement.getBoundingClientRect();
    const bounds = path_data_col.getBoundingClientRect();
    const width = doc_bounds.right - bounds.left;
    overview_canvas.width = width;
    seq_canvas.width = width;

    overview.draw();
    seq_track.draw_last();
  });

  appendSvgViewport();

  await waragraph_worker.setGraphLayoutTable(graph_layout_table);

  const prepareAnnotationRecords = async (intervals: PathInterval[]): Promise<AnnotationGeometry[] | undefined> => {
    return waragraph_worker.prepareAnnotationRecords(intervals);
  };

  console.log("initializing sidebar");
  await initializeBedSidebarPanel(waragraph, prepareAnnotationRecords);

  // TODO
  await addViewRangeInputListeners(global_viewport);
  await addSegmentJumpInputListeners(waragraph);

  console.log("done");

  return waragraph;

}
