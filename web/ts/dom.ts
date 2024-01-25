import Split from "split-grid";
import { GraphViewer } from "./graph_viewer";
import { OverviewMap } from "./overview";
import { PathViewer, addOverviewEventHandlers } from "./path_viewer_ui";
import { Viewport1D, WaragraphOptions, globalSequenceTrack } from "./waragraph";

import * as rxjs from 'rxjs';


export function appendPathListElements(height, left_tag, right_tag) {
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


export function appendSvgViewport() {
  const body = `
<svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg"
     id="viz-svg-overlay"
>
</svg>
`;
  let parent = document.createElement('div');
  parent.id = 'svg-container';
  parent.style.setProperty('z-index', '10');
  parent.style.setProperty('grid-column', '1');
  parent.style.setProperty('grid-row', '1 / -1');
  parent.style.setProperty('background-color', 'transparent');
  parent.style.setProperty('pointer-events', 'none');
  document.getElementById('viz-container')?.append(parent);

  let el = document.createElement('svg');

  parent.append(el);

  el.outerHTML = body;
  el.style.setProperty('position', 'absolute');
}


export async function addViewRangeInputListeners(viewport: Viewport1D) {
  const start_el = document.getElementById('path-viewer-range-start') as HTMLInputElement;
  const end_el = document.getElementById('path-viewer-range-end') as HTMLInputElement;

  let init_view = viewport.get();

  start_el.value = String(init_view.start);
  end_el.value = String(init_view.end);

  const handler = (_event) => {
    const start = parseFloat(start_el.value);
    const end = parseFloat(end_el.value);
    if (!isNaN(start) && !isNaN(end)) {
      viewport.set(start, end);
    }
  };

  start_el.addEventListener('change', handler);
  end_el.addEventListener('change', handler);

  const view_subject = viewport.subject;

  view_subject.subscribe((view) => {
    start_el.value = String(Math.round(view.start));
    end_el.value = String(Math.round(view.end));
  });
}



export async function initializeTree(
  opts: WaragraphOptions,
  resize_observable: rxjs.Subject<void>,
  { graph_viewer, path_viewers }:
    { graph_viewer?: GraphViewer, path_viewers?: Array<PathViewer> }
) {

  const root = document.createElement('div');
  root.classList.add('root-grid');
  root.id = 'waragraph-root';


  root.innerHTML = `
  <div class="root-grid root-sidebar-open" id="root-container">
    <div class="sidebar" id="sidebar"></div>
    <div class="gutter-column gutter-column-sidebar"></div>

    <div class="viz-grid" id="viz-container">
      <div id="graph-viewer-container"></div>

      <div class="gutter-row gutter-row-1"></div>

      <div id="path-viewer-container">
        <div class="path-viewer-column" id="path-viewer-left-column"></div>

        <div class="gutter-column gutter-column-1"></div>

        <div class="path-viewer-column" id="path-viewer-right-column"></div>
      </div>
    </div>
  </div>`;

  appendSvgViewport();

  // this.graph_viewer?.container

  if (graph_viewer) {
    const container = document.getElementById('graph-viewer-container');

    container!.append(graph_viewer.gpu_canvas);
    container!.append(graph_viewer.overlay_canvas);

    graph_viewer.resize();
  }



  // await BedSidebar.initializeBedSidebarPanel(this);

  //
  // }

  // add splits
  // const sidebar_viz_gutter = document.createElement('div');

  // const viz_1d_2d_gutter = document.createElement('div');

  const path_name_col = document.getElementById('path-viewer-left-column');
  const path_data_col = document.getElementById('path-viewer-right-column');


  {
    // TODO: factor out overview & range input bits
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

    const viewport_key = opts.path_viewers!.viewport;
    const viewport = await this.get1DViewport({ name: viewport_key.name });

    const overview = new OverviewMap(overview_canvas, viewport!.max);
    await addOverviewEventHandlers(overview, viewport!);


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
    await addViewRangeInputListeners(viewport);

    // TODO: factor out sequence track bit maybe

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


    resize_observable
      .pipe(
        rxjs.throttleTime(500)
      )
      .subscribe(() => {
        this.graph_viewer?.resize();

        const doc_bounds = document.documentElement.getBoundingClientRect();
        // const win_width = window.innerWidth;
        const bounds = path_data_col.getBoundingClientRect();
        const width = doc_bounds.right - bounds.left;

        overview_canvas.width = width;
        seq_canvas.width = width;
        // overview_canvas.width = overview_slots.right.clientWidth;
        // seq_canvas.width = seq_slots.right.clientWidth;

        // overview_canvas.height = overview_slots.right.clientHeight;
        // seq_canvas.height = seq_slots.right.clientHeight;

        overview.draw();
        seq_track.draw_last();
      });

  }

  if (this.intersection_observer === undefined) {
    this.intersection_observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if ("path_viewer" in entry.target) {
          const viewer = entry.target.path_viewer as PathViewer;
          viewer.isVisible = entry.isIntersecting;
        }
      });
    },
      { root: document.getElementById('path-viewer-container') }
    );

  }

  for (const path_viewer of this.path_viewers) {
    path_data_col?.append(path_viewer.container);

    this.intersection_observer?.observe(path_viewer.data_canvas);

    path_viewer.container.classList.add('path-list-flex-item');
    // path_viewer.container.style.setProperty('overflow','hidden');
    // path_viewer.container.style.setProperty('position','absolute');
    console.warn(path_viewer.container);
    // name_el.classList.add('path-list-flex-item', 'path-name');
    // data_el.classList.add('path-list-flex-item');

    const name_el = document.createElement('div');
    name_el.classList.add('path-list-flex-item', 'path-name');
    name_el.innerHTML = path_viewer.pathName;

    path_name_col?.append(name_el);

    path_viewer.onResize();

    resize_observable
      .pipe(rxjs.throttleTime(500))
      .subscribe((_) => {
        path_viewer.onResize();

      })
  }

  // TODO: additional tracks

  const split_root = Split({
    columnGutters: [{
      track: 1,
      element: document.querySelector('.gutter-column-sidebar'),
    }],
    onDragEnd: (dir, track) => {
      // graph_viewer.resize();
      console.warn("resizing split!");
      resize_observable.next();
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
      resize_observable.next();
    },
  });

  rxjs.fromEvent(window, 'resize')
    .subscribe(() => {
      resize_observable.next();
    });
}
