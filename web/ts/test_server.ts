
import init_module, * as wasm_bindgen from 'waragraph';

import * as rxjs from 'rxjs';

import { PathViewer, addPathViewerLogic, addPathViewerLogicClient, initializePathViewerClient } from './path_viewer_ui';
import { Viewport1D } from './waragraph';

import { tableFromIPC, tableFromArrays } from 'apache-arrow';
import { CoordSysArrow, CoordSysInterface } from './coordinate_system';
import { GraphViewer, graphViewerFromData } from './graph_viewer';
import { ArrowGFA, PathIndex } from './graph_api';



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

  console.log(cs);

  console.log("???");

  let viewport = new Viewport1D(cs_arrow as CoordSysInterface);


  console.log(viewport);
  console.log(viewport.length);

  for (const path of paths) {
    console.log(path);
    const viewer = await initializePathViewerClient(
      path.name,
      viewport, 
      base_url,
      "depth",
      0.5,
      { r: 1.0, g: 1.0, b: 1.0 },
      { r: 1.0, g: 0.0, b: 0.0 }
    );

    viewer.container.style.setProperty('flex-basis', '20px');

    document.getElementById('path-viewer-right-column').append(viewer.container);

    await addPathViewerLogicClient(viewer);

    viewer.onResize();
    console.log(viewer);

    viewer.isVisible = true;
    viewer.sampleAndDraw(viewport.get());

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
