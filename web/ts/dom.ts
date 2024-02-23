import Split from "split-grid";
import { GraphViewer } from "./graph_viewer";
import { OverviewMap } from "./overview";
import { PathViewer, addOverviewEventHandlers } from "./path_viewer_ui";
import { Viewport1D, Waragraph, WaragraphOptions, globalSequenceTrack } from "./waragraph";

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
     preserveAspectRatio="none meet"
     style="width: 100%; height: 100%"
>
<defs>
  <mask id="mask-2d-view">
    <rect fill="white" x="0" y="0" width="100" height="50" />
  </mask>

  <mask id="mask-path-viewers">
    <rect fill="white" x="0" y="50" width="100" height="50" />
  </mask>
</defs>
</svg>
`;
  let parent = document.createElement('div');
  parent.id = 'svg-container';
  parent.style.setProperty('z-index', '10');
  parent.style.setProperty('grid-column', '1');
  parent.style.setProperty('grid-row', '1 / -1');
  parent.style.setProperty('background-color', 'transparent');
  parent.style.setProperty('pointer-events', 'none');
  parent.style.setProperty('width', '100%');
  parent.style.setProperty('height', '100%');

  document.getElementById('viz-container')?.append(parent);

  let el = document.createElementNS('http://www.w3.org/2000/svg', 'svg') as SVGSVGElement;

  parent.append(el);

  el.outerHTML = body;

  el.style.setProperty('position', 'absolute');
}

export function updateSVGMasks() {
  const svg_div = document.getElementById('svg-container')!;

  const rect_cont = svg_div.getBoundingClientRect();

  const mask_2d = svg_div.querySelector('#mask-2d-view');
  const mask_1d = svg_div.querySelector('#mask-path-viewers');

  // get the 2D view canvas
  const rect_2d = document
    .getElementById('graph-viewer-container')!
    .getBoundingClientRect();

  const x_2d = (rect_2d.left - rect_cont.left) / rect_cont.width;
  const y_2d = (rect_2d.top - rect_cont.top) / rect_cont.height;
  const width_2d = rect_2d.width / rect_cont.width;
  const height_2d = rect_2d.height / rect_cont.height;

  mask_2d.innerHTML =
    `<rect fill="white" x="${x_2d * 100}" y="${y_2d * 100}" width="${width_2d * 100}" height="${height_2d * 100}"/>`;

  // get the right path viewer column
  const rect_1d = document
    .getElementById('path-viewer-right-column')!
    .getBoundingClientRect();

  const x_1d = (rect_1d.left - rect_cont.left) / rect_cont.width;
  const y_1d = (rect_1d.top - rect_cont.top) / rect_cont.height;
  const width_1d = rect_1d.width / rect_cont.width;
  const height_1d = rect_1d.height / rect_cont.height;

  mask_1d.innerHTML =
    `<rect fill="white" x="${x_1d * 100}" y="${y_1d * 100}" width="${width_1d * 100}" height="${height_1d * 100}"/>`;

}


/*
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
  */
export async function addViewRangeInputListeners(viewport: Viewport1D) {

  // control pane inputs
  const start_input = document.getElementById('control-input-range-start') as HTMLInputElement;
  const end_input = document.getElementById('control-input-range-end') as HTMLInputElement;
  const go_el = document.getElementById('control-input-range-button') as HTMLInputElement;

  let init_view = viewport.get();

  start_input.value = String(init_view.start);
  end_input.value = String(init_view.end);

  // graph view inputs (disabled), for displaying the current selection only
  const start_view = document.getElementById('path-viewer-range-start') as HTMLInputElement;
  const end_view = document.getElementById('path-viewer-range-end') as HTMLInputElement;

  // init view range
  start_view.value = String(init_view.start);
  end_view.value = String(init_view.end);

  const handler = (_event) => {
    var start = parseFloat(start_input.value);
    var end = parseFloat(end_input.value);
    
    if (!isNaN(start) && !isNaN(end)) {

      // bound checking
      
      if (start < 0) {
        start = 0;
        start_input.value = String(start);
      }
      if (start >= viewport.max) {
        start = viewport.max - 1;
        start_input.value = String(start);
      }
      if (end <= 0) {
        end = start + 1;
        end_input.value = String(end);
      }
      if (end > viewport.max) {
        end = viewport.max;
        end_input.value = String(end);
      }
      if (start >= end) {
        start = end -1;
        start_input.value = String(start);
      }

      viewport.set(start, end);
    }
  };

  go_el.addEventListener('click', handler);

  const view_subject = viewport.subject;

  view_subject.subscribe((view) => {
    start_view.value = String(Math.round(view.start));
    end_view.value = String(Math.round(view.end));
  });

  // potential fix for offscreen bars not rendering until resize
  // drawback: causes ugly stutter on firefox while scrolling

  const pathViewerContainer = document.getElementById('path-viewer-container');

  pathViewerContainer.addEventListener('scroll', () => {
    window.dispatchEvent(new Event('resize'));
  });
  
}



// Segment jump function on control panel
// export async function addSegmentJumpInputListeners(graph_viewer) {
export async function addSegmentJumpInputListeners(waragraph: Waragraph) {

  const segment_input = document.getElementById('control-input-segment-start') as HTMLInputElement;
  const segment_button = document.getElementById('control-input-segment-button') as HTMLInputElement;

  const graph_viewer = waragraph.graph_viewer;

  const handler = (_event) => {

    var segment = parseInt(segment_input.value);

    if (!isNaN(segment)) {

      // TODO: bounds check
      const position = graph_viewer.graph_layout.segmentPosition(segment);

      if (position !== null) {
        // Center of segment
        const midpoint = {
          x: (position.p0[0] + position.p1[0]) / 2.0,
          y: (position.p0[1] + position.p1[1]) / 2.0
        };

        // Jump to segment
        const view = document.getElementById('graph-viewer-2d');
        view.style.display = 'none';
        graph_viewer.resetView();
        graph_viewer.setViewCenter(midpoint.x, midpoint.y);

        setTimeout(() => {
          const screen_position = graph_viewer.getSegmentScreenPos(segment);

          const midpoint_screen = {
            x: (screen_position.start[0]),
            y: (screen_position.start[1])
          }

          let zoom_x = midpoint_screen.x / graph_viewer.overlay_canvas.width;
          let zoom_y = midpoint_screen.y / graph_viewer.overlay_canvas.height;
          graph_viewer.zoom(zoom_x, zoom_y, .05);
          view.style.display = 'block';
      }, 100);

      }
      else {
        console.warn('Segment is null');
      }
    }
  }
  
  segment_button.addEventListener('click', handler);
}
