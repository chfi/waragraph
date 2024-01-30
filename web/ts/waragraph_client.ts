
import init_module, * as wasm_bindgen from 'waragraph';

import * as rxjs from 'rxjs';

import { PathViewer, addOverviewEventHandlers, addPathViewerLogic, addPathViewerLogicClient, initializePathViewerClient } from './path_viewer_ui';
import { Viewport1D, globalSequenceTrack } from './waragraph';

import { tableFromIPC, tableFromArrays } from 'apache-arrow';
import { CoordSysArrow, CoordSysInterface } from './coordinate_system';
import { GraphViewer, graphViewerFromData } from './graph_viewer';
import { ArrowGFA, GraphLayout, PathIndex, serverAPIs } from './graph_api';

import { addViewRangeInputListeners, appendPathListElements, appendSvgViewport } from './dom';
import { OverviewMap } from './overview';

import * as chroma from 'chroma-js';
import { PathId } from './types';
import Split from 'split-grid';
import { initializeBedSidebarPanel } from './sidebar-bed';

export class Waragraph {
  graph_viewer: GraphViewer | undefined;
  path_viewers: Array<PathViewer>;

  graph: ArrowGFA;
  // path_index: PathIndex;

  graphLayout: GraphLayout | undefined;

  global_viewport: Viewport1D;

  resize_observable: rxjs.Subject<void>;
  intersection_observer: IntersectionObserver | undefined;

  api_base_url: URL;


  constructor(
    base_url: URL,
    viewers: { graph_viewer?: GraphViewer, path_viewers: Array<PathViewer> },
    graph: ArrowGFA,
    global_viewport: Viewport1D,
    graphLayout?: GraphLayout,
  ) {
    this.graph_viewer = viewers.graph_viewer;
    this.path_viewers = viewers.path_viewers;
    this.graph = graph;
    this.global_viewport = global_viewport;
    this.graphLayout = graphLayout;

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

    const split_root = Split({
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
    });

  }

  

  segmentScreenPos2d(segment: number) {
    let seg_pos = this.graph_viewer?.getSegmentScreenPos(segment);

    if (!seg_pos) {
      return null;
    }

    return seg_pos;
  }

  async segmentScreenPos1d(path: PathId | string, segment: number) {
    let path_name: string | undefined;

    if (typeof path === 'string') {
      path_name = path;
    } else {
      path_name = await this.graph.pathNameFromId(path);
    };

    let viewport = this.global_viewport;

    let seg_range = viewport.segmentRange(segment);

    let el = document.getElementById('viewer-' + path_name);

    if (!el) {
      return null;
    }

    let el_rect = el.getBoundingClientRect();

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


export async function initializeWaragraphClient(base_url: URL) {

  const wasm = await init_module();

  const graph_apis = await serverAPIs(base_url);
  
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

    let seg_seq_array = await graph_apis.arrowGFA.segmentSequencesArray();
    let seq_track = globalSequenceTrack(
      seg_seq_array,
      seq_canvas,
      global_viewport.subject
    );
  }


  const paths = await graph_apis.arrowGFA.pathMetadata();

  for (const path of paths) {
    console.log(path);

    const color_below = { r: 1.0, g: 1.0, b: 1.0 };
    const color_above = wasm_bindgen.path_name_hash_color_obj(path.name);

    const viewer = await initializePathViewerClient(
      path.name,
      global_viewport, 
      base_url,
      "depth",
      0.5,
      color_below,
      color_above
    );

    viewer.container.style.setProperty('flex-basis', '20px');
    viewer.container.path_viewer = viewer;

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


  console.log("fetching layout data");
  const layout_table = await fetch(new URL('/graph_layout', base_url))
    .then(r => r.arrayBuffer())
    .then(data => tableFromIPC(data));

  console.log(layout_table);

  // TODO get from the server; this will do for now
  const segment_count = layout_table.getChild('x')!.length / 2;

  console.log(layout_table);
  console.log("segment count: ", segment_count);

  const data_resp = await fetch(new URL('/graph_dataset/depth', base_url));
  const data_buffer = await data_resp.arrayBuffer();
  const depth_data = new Float32Array(data_buffer);

  const depth_color_buffer = new ArrayBuffer(depth_data.length * 4);
  // const depth_color = new Uint32Array(depth_data.length);
  const depth_color_bytes = new Uint8Array(depth_color_buffer);

  depth_data.forEach((val, i) => {
    let color = spectralScale(val);
    let [r, g, b] = color.rgb();
    depth_color_bytes[i * 4] = r;
    depth_color_bytes[i * 4 + 1] = g;
    depth_color_bytes[i * 4 + 2] = b;
    depth_color_bytes[i * 4 + 3] = 255;
  });

  const depth_color = new Uint32Array(depth_color_buffer);

  const graph_viewer = await graphViewerFromData(
    document.getElementById('graph-viewer-container'),
    layout_table,
    depth_color
  );

  console.log(graph_viewer);
  graph_viewer.draw();

  const waragraph = new Waragraph(
    base_url,
    { graph_viewer, path_viewers },
    graph_apis.arrowGFA,
    global_viewport
  );

  await initializeBedSidebarPanel(waragraph);

}

/*
static scheme_t Spectral =
{{[252,141,89],
  [255,255,191],
  [153,213,148]},
 {[215,25,28],
  [253,174,97],
  [171,221,164],
  [43,131,186]},
 {[215,25,28],
  [253,174,97],
  [255,255,191],
  [171,221,164],
  [43,131,186]},
 {[213,62,79],
  [252,141,89],
  [254,224,139],
  [230,245,152],
  [153,213,148],
  [50,136,189]},
 {[213,62,79],
  [252,141,89],
  [254,224,139],
  [255,255,191],
  [230,245,152],
  [153,213,148],
  [50,136,189]},
 {[213,62,79],
  [244,109,67],
  [253,174,97],
  [254,224,139],
  [230,245,152],
  [171,221,164],
  [102,194,165],
  [50,136,189]},
 {[213,62,79],
  [244,109,67],
  [253,174,97],
  [254,224,139],
  [255,255,191],
  [230,245,152],
  [171,221,164],
  [102,194,165],
  [50,136,189]},
 {[158,1,66],
  [213,62,79],
  [244,109,67],
  [253,174,97],
  [254,224,139],
  [230,245,152],
  [171,221,164],
  [102,194,165],
  [50,136,189],
  [94,79,162]},
 {[158,1,66],
  [213,62,79],
  [244,109,67],
  [253,174,97],
  [254,224,139],
  [255,255,191],
  [230,245,152],
  [171,221,164],
  [102,194,165],
  [50,136,189],
  [94,79,162] }};
*/

const spectralScale = chroma.scale([
  [64, 64, 64],
  [127, 127, 127],
  [158, 1, 66],
  [213, 62, 79],
  [244, 109, 67],
  [253, 174, 97],
  [254, 224, 139],
  [255, 255, 191],
  [230, 245, 152],
  [171, 221, 164],
  [102, 194, 165],
  [50, 136, 189],
  [94, 79, 162]
]).domain([0, 12]);
