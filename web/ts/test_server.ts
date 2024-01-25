
import init_module, * as wasm_bindgen from 'waragraph';

import * as rxjs from 'rxjs';

import { PathViewer, addOverviewEventHandlers, addPathViewerLogic, addPathViewerLogicClient, initializePathViewerClient } from './path_viewer_ui';
import { Viewport1D } from './waragraph';

import { tableFromIPC, tableFromArrays } from 'apache-arrow';
import { CoordSysArrow, CoordSysInterface } from './coordinate_system';
import { GraphViewer, graphViewerFromData } from './graph_viewer';
import { ArrowGFA, PathIndex } from './graph_api';

import { addViewRangeInputListeners, appendPathListElements, appendSvgViewport } from './dom';
import { OverviewMap } from './overview';


export class Waragraph {
  graph_viewer: GraphViewer | undefined;
  path_viewers: Array<PathViewer>;

  // graph: ArrowGFA;
  // path_index: PathIndex;
  
  resize_obs: rxjs.Subject<unknown> | undefined;

  global_viewport: Viewport1D;

  resize_observable: rxjs.Subject<void> | undefined;
  intersection_observer: IntersectionObserver | undefined;
}

export async function testPathViewer(base_url: URL) {

  const wasm = await init_module();

  let paths_resp = await fetch(new URL('/path_metadata', base_url));
  let paths = await paths_resp.json();

  
  let cs_resp = await fetch(new URL('/coordinate_system/global', base_url));

  if (!cs_resp.ok) {
    return;
  }
  let cs = await tableFromIPC(cs_resp);

  let step_offsets = cs.getChild('step_offsets')!;
  let max = step_offsets.get(step_offsets.length - 1);

  let cs_arrow = new CoordSysArrow(cs);

  let global_viewport = new Viewport1D(cs_arrow as CoordSysInterface);

  const path_name_col = document.getElementById('path-viewer-left-column')!;
  const path_data_col = document.getElementById('path-viewer-right-column')!;

  const path_viewers: Array<PathViewer> = [];

  for (const path of paths) {
    console.log(path);
    const viewer = await initializePathViewerClient(
      path.name,
      global_viewport, 
      base_url,
      "depth",
      0.5,
      { r: 1.0, g: 1.0, b: 1.0 },
      { r: 1.0, g: 0.0, b: 0.0 }
    );

    viewer.container.style.setProperty('flex-basis', '20px');

    path_data_col.append(viewer.container);

    await addPathViewerLogicClient(viewer);

    viewer.onResize();
    console.log(viewer);

    viewer.isVisible = true;
    viewer.sampleAndDraw(global_viewport.get());

    viewer.container.classList.add('path-list-flex-item');

    const name_el = document.createElement('div');
    name_el.classList.add('path-list-flex-item', 'path-name');
    name_el.innerHTML = viewer.pathName;

    path_name_col.append(name_el);

    path_viewers.push(viewer);

      // this.resize_obs
      //   .pipe(rxjs.throttleTime(500))
      //   .subscribe((_) => {
      //     path_viewer.onResize();

      //   })

  }

  {
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

    let seg_seq_array = await this.graph.segmentSequencesArray();
    let seq_track = globalSequenceTrack(
      seg_seq_array,
      seq_canvas,
      viewport!.subject
    );

  }

  console.log("fetching layout data");
  const layout_table = await fetch(new URL('/graph_layout', base_url))
    .then(r => r.arrayBuffer())
    .then(data => tableFromIPC(data));

  console.log(layout_table);

  // TODO get from the server; this will do for now
  const segment_count = layout_table.getChild('x')!.length / 2;

  console.log(layout_table);
  console.log("segment count: ", segment_count);

  const graph_viewer = await graphViewerFromData(
    document.getElementById('graph-viewer-container'),
    layout_table
  );

  console.log(graph_viewer);
  graph_viewer.draw();

}
