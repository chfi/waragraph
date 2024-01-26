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

