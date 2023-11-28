import init_module, * as wasm_bindgen from 'waragraph';

import { initializePathViewer, addOverviewEventHandlers, addPathViewerLogic } from './path_viewer_ui';
import { OverviewMap } from './overview';
import {
  GraphViewer,
  initializeGraphViewer,
  preparePathHighlightOverlay
} from './graph_viewer';
import * as CanvasTracks from './canvas_tracks';
import * as BedSidebar from './sidebar-bed';
import { wrapWasmPtr } from './wrap';

import * as Comlink from 'comlink';
import { Observable } from 'rxjs';
import * as rxjs from 'rxjs';

import * as handler from './transfer_handlers';
handler.setTransferHandlers(rxjs, Comlink);

import Split from 'split-grid';

import type { WorkerCtxInterface } from './main_worker';

import type { Bp } from './types';


const gfa_path = "./data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa";
const layout_path = "./data/A-3105.layout.tsv";
const path_names = undefined;

// const path_names = ["gi|568815592:29942469-29945883"];

// const gfa_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.gfa";
// const layout_path = "./MHC/HPRCy1v2.MHC.fa.ce6f12f.417fcdf.0ead406.smooth.final.og.lay.tsv";
// const path_names = [
//     "chm13#chr6:28385000-33300000",
//     "grch38#chr6:28510128-33480000",
//     "HG02717#2#h2tg000061l:22650152-27715000",
//     "HG03516#1#h1tg000073l:22631064-27570000",
//     "HG00733#1#h1tg000070l:28540000-33419448",
//     "HG02055#1#h1tg000074l:0-4714592",
//     "HG01978#1#h1tg000035l:28455000-33469848",
//     "HG02886#2#h2tg000003l:25120800-30214744",
// ];

function globalSequenceTrack(graph, canvas, view_subject) {

  const min_px_per_bp = 8.0;
  const seq_array = graph.segment_sequences_array();

  let last_view = null;

  const draw_view = (view) => {
    let view_len = view.end - view.start;
    let px_per_bp = canvas.width / view_len;
    let ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    if (px_per_bp > min_px_per_bp) {
      let seq = seq_array.slice(view.start, view.end);
      let subpixel_offset = view.start - Math.trunc(view.start);
      CanvasTracks.drawSequence(canvas, seq, subpixel_offset);

      last_view = view;
    }
  };

  view_subject.pipe(
    rxjs.distinct(),
    rxjs.throttleTime(10)
  ).subscribe((view) => {
    requestAnimationFrame((time) => {
      draw_view(view);
    });
  });

  const draw_last = () => {
    if (last_view !== null) {
      draw_view(last_view);
    }
  };

  return { draw_last };
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

async function appendPathView(worker_obj, resize_subject, path_name) {

  const name_column = document.getElementById('path-viewer-left-column');
  const data_column = document.getElementById('path-viewer-right-column');

  const name_el = document.createElement('div');
  const data_el = document.createElement('div');

  name_el.classList.add('path-list-flex-item', 'path-name');
  data_el.classList.add('path-list-flex-item');

  name_el.innerHTML = path_name;

  let cs_view = await worker_obj.globalCoordSysView();

  name_column?.append(name_el);
  data_column?.append(data_el);

  let path_viewer = await initializePathViewer(worker_obj,
    cs_view,
    path_name,
    data_el,
    resize_subject);


  addPathViewerLogic(worker_obj, path_viewer, cs_view);

}

export class WaragraphViz {
  wasm: wasm_bindgen.InitOutput;
  worker_obj: Comlink.Remote<WorkerCtxInterface>;
  graph_viewer: GraphViewer;

  constructor(
    wasm: wasm_bindgen.InitOutput,
    worker_obj: Comlink.Remote<WorkerCtxInterface>,
    graph_viewer: GraphViewer,
  ) {
    this.wasm = wasm;
    this.worker_obj = worker_obj;
    this.graph_viewer = graph_viewer;
  }



  /*
// Temporary test
async updateSvgLink(path_name, segment) {
    const svg = document.getElementById('viz-svg-overlay');

    const id = 'segment-link-' + segment;

    let g_el = svg.getElementById(id);

    if (!g_el) {
        g_el = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        g_el.id = id;
        svg.append(g_el);
    }

    const svg_rect = svg.getBoundingClientRect();

    const pos_2d = this.segmentScreenPos2d(segment);
    const pos_1d = this.segmentScreenPos1d(path_name, segment);

    let svg_pos_2d;
    let svg_pos_1d;

    if (pos_2d !== null) {
        let canv_2d = document.getElementById('graph-viewer-2d-overlay') as HTMLCanvasElement;

        let x2d = pos_2d.start[0];
        let y2d = pos_2d.start[1];

        let height_prop = canv_2d.height / svg.clientHeight;

        let cx = (x2d / canv_2d.width) * 100;
        let cy = (y2d / canv_2d.height) * 100 * height_prop;

        let el = svg.querySelector('circle');
        if (!el) {
            el = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
        }

        const svg_2d = `
<circle cx="${cx}" cy="${cy}" r="1" fill="transparent" stroke="red"
stroke-width="0.5" />
`;

        svg_pos_2d = { cx, cy };

        g_el.append(el);
        el.outerHTML = svg_2d;
    }


    let svg_rect_1d;
    {
        const canvas_1d = document.getElementById("viewer-" + path_name);
        const rect = canvas_1d.getBoundingClientRect();

        let x0 = 100 * (rect.left - svg_rect.left) / svg_rect.width;
        let y0 = 100 * (rect.top - svg_rect.top) / svg_rect.height;

        let width = 100 * (rect.width / svg_rect.width)
        let height = 100 * (rect.height / svg_rect.height);

        let x1 = x0 + width;
        let y1 = y0 + height;

        svg_rect_1d = { x0, y0, x1, y1, width, height};
    }

    // console.warn(svg_rect_1d);

    if (pos_1d !== null) {
        let { x0, y0, x1, y1 } = pos_1d;

        const canvas_1d = document.getElementById("viewer-" + path_name);
        const rect = canvas_1d.getBoundingClientRect();

        // console.log(pos_1d);
        // console.log(svg);
        let left = svg.clientLeft;
        // console.log(left);

        let x = 100 * (x0 - svg_rect.left) / svg_rect.width;
        let y = 100 * (y0 - svg_rect.top) / svg_rect.height;

        let width = 100 * (x1 - x0) / svg_rect.width;
        let height = 100 * (y1 - y0) / svg_rect.height;

        let el = svg.querySelector('rect');
        if (!el) {
            el = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
        }

        // let clip_el = svg.querySelector('clipPath');
        // if (!clip_el) {
        //     clip_el = document.createElementNS('http://www.w3.org/2000/svg', 'clipPath');
        //     clip_el.id = 'clip-path-' + path_name;
        //     clip_el.innerHTML = `<circle cx="1" cy="1" r="1" />`;
        //     g_el.append(clip_el);
        // }
  // clip-path="url(#${'clip_el.id'})"

        let r = svg_rect_1d;
        const svg_1d = `
<rect x="${x}" y="${y}" width="${width}" height="${height}"
  clip-path="polygon(0% 0%, 100% 0%, 100% 100%, 0% 100%)"
  stroke="red"
/>`;

        svg_pos_1d = { x, y, width, height };

        g_el.append(el);
        el.outerHTML = svg_1d;
    }

    if (svg_pos_1d && svg_pos_2d) {
        let el = svg.getElementById('link-path');
        if (!el) {
            // el = document.createElementNS('http://www.w3.org/2000/svg', 'path');
            el = document.createElementNS('http://www.w3.org/2000/svg', 'g');
            el.id = 'link-path';
            g_el.append(el);
        }

        let cx = svg_pos_1d.x;
        let cy = svg_pos_1d.y - 1;
        el.innerHTML = `
<path d="M ${svg_pos_2d.cx},${svg_pos_2d.cy} S ${cx},${cy} ${svg_pos_1d.x},${svg_pos_1d.y}"
stroke-width="0.1"
stroke="red"
fill="none"
/>
<path d="M ${svg_pos_2d.cx},${svg_pos_2d.cy} S ${cx + svg_pos_1d.width},${cy} ${svg_pos_1d.x + svg_pos_1d.width},${svg_pos_1d.y}"
stroke-width="0.1"
stroke="red"
fill="none"
/>
`;


    }


}
*/

  centerViewOnSegment2d(segment) {
    let seg_pos = this.graph_viewer.getSegmentPos(segment);

    if (seg_pos === null) {
      return null;
    }

    let { x0, y0, x1, y1 } = seg_pos;

    let x = (x1 + x0) / 2;
    let y = (y1 + y0) / 2;

    this.graph_viewer.setViewCenter(x, y);
  }

  segmentScreenPos2d(segment) {
    let seg_pos = this.graph_viewer.getSegmentScreenPos(segment);

    if (seg_pos === null) {
      return null;
    }

    return seg_pos;
  }

  async segmentScreenPos1d(path_name: string, segment) {
    let cs_view = await this.worker_obj.globalCoordSysView();
    let view = await cs_view.get();
    let seg_range = await cs_view.segmentRange(segment);

    let el = document.getElementById('viewer-' + path_name);

    let el_rect = el.getBoundingClientRect();

    if (!el) {
      return null;
    }

    // segmentRange returns BigInts
    let seg_s = Number(seg_range.start);
    let seg_e = Number(seg_range.end);

    let seg_start = (seg_s - view.start) / view.len;
    let seg_end = (seg_e - view.start) / view.len;

    let width = el_rect.width;
    let y0 = el_rect.y;
    let y1 = el_rect.y + el_rect.height;

    let x0 = el_rect.left + seg_start * width;
    let x1 = el_rect.left + seg_end * width;

    return { x0, y0, x1, y1 };
  }

}

const init = async () => {

  const wasm = await init_module();
  const worker = new Worker(new URL("main_worker.ts", import.meta.url), { type: 'module' });

  svgViewport();

  // window.wasm_bindgen = wasm;

  worker.onmessage = async (event) => {
    if (event.data === "WORKER_INIT") {
      worker.postMessage([wasm.memory, gfa_path]);
    } else if (event.data === "GRAPH_READY") {
      worker.onmessage = null;

      const worker_obj = Comlink.wrap(worker) as Comlink.Remote<WorkerCtxInterface>;

      const graph_raw = await worker_obj.getGraph();

      const graph_viewer = await initializeGraphViewer(wasm.memory, graph_raw, layout_path);

      const warapi = new WaragraphViz(wasm, worker_obj, graph_viewer);

      // window.getPathCoordSys = async (path_name) => {
      //     return await worker_obj.pathCoordSys(path_name);
      // };

      // window.waragraph_viz = warapi;

      // {
      //     let iv_id;
      //     window.testSvgLink = () => {
      //         if (iv_id) {
      //             window.clearInterval(iv_id);
      //             iv_id = undefined;
      //         } else {
      //             iv_id = window.setInterval(() => {
      //                 waragraph_viz.updateSvgLink("gi|528476637:29857558-29915771", 1772);
      //             }, 100);
      //         }
      //     };
      // }

      // getPathRange("grch38#chr6:28510128-33480000", 1841288n, 1841422n)
      // window.getPathRange = async (path_name, start, end) => {
      //     let cs_raw = await worker_obj.pathCoordSys(path_name);
      //     let cs = wasm_bindgen.CoordSys.__wrap(cs_raw.__wbg_ptr);
      //     return cs.bp_to_step_range(start, end);
      // };

      await BedSidebar.initializeBedSidebarPanel(warapi);

      const resize_obs = new rxjs.Subject();

      let names;
      if (path_names) {
        names = path_names;
      } else {
        names = await worker_obj.getPathNames();
      }

      {
        // TODO: factor out overview & range input bits
        const overview_slots = appendPathListElements(40, 'div', 'div');

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

        await addViewRangeInputListeners(cs_view);

        // TODO: factor out sequence track bit maybe

        const seq_slots = appendPathListElements(20, 'div', 'div');

        const seq_canvas = document.createElement('canvas');
        seq_canvas.width = seq_slots.right.clientWidth;
        seq_canvas.height = seq_slots.right.clientHeight;
        seq_canvas.style.setProperty('position', 'absolute');
        seq_canvas.style.setProperty('overflow', 'hidden');

        seq_slots.right.append(seq_canvas);

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

      }

      for (const path_name of names) {
        appendPathView(worker_obj, resize_obs, path_name);
      }

      // TODO: additional tracks

      const split_root = Split({
        columnGutters: [{
          track: 1,
          element: document.querySelector('.gutter-column-sidebar'),
        }],
        onDragEnd: (dir, track) => {
          graph_viewer.resize();
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
            graph_viewer.resize();
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
        graph_viewer.resize();
        resize_obs.next(null);
      });
    }
  };

};

const svgViewport = () => {
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

window.onload = init;
