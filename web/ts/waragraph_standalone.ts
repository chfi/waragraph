import init_module, * as wasm_bindgen from 'waragraph';

import * as Comlink from 'comlink';

import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

import { PathViewer, addOverviewEventHandlers, addPathViewerLogic, addPathViewerLogicClient, initializePathViewerClient } from './path_viewer_ui';
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


  // then it should be pretty much the same as the rest of the init function in waragraph_client.ts
}
