import init_module, * as wasm_bindgen from 'waragraph';

// import BED from '@gmod/bed';

// import {computePosition} from '@floating-ui/dom';

// import { preparePathHighlightOverlay } from '../graph_viewer';

import { mat3, vec2, vec3 } from 'gl-matrix';

import { BEDRecord } from './sidebar-bed';

// import { WaragraphViz } from './index';
import { type Waragraph } from './waragraph_client';

import * as CanvasTracks from './canvas_tracks';
import { PathViewer } from './path_viewer_ui';
import { ArrowGFA } from './graph_api';
import { PathInterval } from './types';



function createSVGElement(tag) {
  return document.createElementNS('http://www.w3.org/2000/svg', tag);
}


export interface AnnotationGeometry {
  start_world_p: vec2,
  end_world_p: vec2,

  path_steps: Uint32Array;

  start_bp_1d: number;
  end_bp_1d: number;
  blocks_1d_bp: Array<number>;
}

interface AnnotationRecord {
  svg_g: SVGGElement;
  record: BEDRecord;
  enabled: boolean;

  // handles
  step_endpoints: { first: number, last: number } | undefined;
  // in the viz/global coordinate system
  start_bp: number | undefined;
  end_bp: number | undefined;

  path_steps: Uint32Array | undefined;
  
  start_world_2d: vec2 | undefined;
  end_world_2d: vec2 | undefined;
  
  global_ranges: Array<{ start: number, end: number }> | undefined;
  // cached_path: wasm_bindgen.CanvasPathTrace | null;
  // cached_path: Float32Array | undefined;
  cached_path: CachedPath | undefined;

  color?: string;
}

interface CachedPath {
  path: Float32Array;
  tolerance: number;
}

let _wasm;

export class AnnotationPainter {
  callback_key: string;
  waragraph: Waragraph;
  
  arrowGFA: ArrowGFA;
  prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>;

  svg_root: SVGSVGElement;
  record_states: AnnotationRecord[];

  last_2d_view: { x: number, y: number, width: number, height: number } | null;

  constructor(
    waragraph: Waragraph,
    prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>,
    name: string,
    records: Iterable<BEDRecord>
  ) {
    this.callback_key = "painter-" + name;

    this.waragraph = waragraph;
    this.arrowGFA = waragraph.graph;
    this.prepareAnnotationRecords = prepareAnnotationRecords;

    this.svg_root = createSVGElement('g');
    this.svg_root.id = this.callback_key;

    this.record_states = [];

    this.last_2d_view = null;

    // this.last_2d_view_scale = null;
    // this.last_2d_view_center = null;
    // this.last_1d_view = null;

    for (const record of records) {
      const g_el = createSVGElement('g');

      const g_1d = createSVGElement('g');
      g_1d.classList.add('svg-overlay-1d');
      g_1d.append(createSVGElement('text'));

      const g_2d = createSVGElement('g');
      g_2d.classList.add('svg-overlay-2d');
      g_2d.append(createSVGElement('path'));
      g_2d.append(createSVGElement('text'));

      const g_link_start = createSVGElement('line');
      const g_link_end = createSVGElement('line');
      g_link_start.classList.add('svg-overlay-link-start');
      g_link_end.classList.add('svg-overlay-link-end');

      g_1d.setAttribute('display', 'none');
      g_2d.setAttribute('display', 'none');

      g_link_start.setAttribute('stroke-width', '0.3');
      g_link_end.setAttribute('stroke-width', '0.3');
      g_link_start.setAttribute('display', 'none');
      g_link_end.setAttribute('display', 'none');

      g_el.append(g_1d);
      g_el.append(g_2d);
      g_el.append(g_link_start);
      g_el.append(g_link_end);

      this.record_states.push({
          svg_g: g_el,
          record,

          enabled: false,

          global_ranges: undefined,
          cached_path: undefined,

          step_endpoints: undefined,
          start_bp: undefined,
          end_bp: undefined,
          start_world_2d: undefined,
          end_world_2d: undefined
      });

      this.svg_root.append(g_el);
    }
  }

  async prepareRecords() {

    const viewport = this.waragraph.global_viewport;

    const annotation_ranges = this.record_states.map((state) => state.record.path_interval);

    const prepared = await this.prepareAnnotationRecords(annotation_ranges);

    if (prepared === undefined) {
      console.error("Error preparing annotations");
    }

    console.log(prepared);

    prepared.forEach((annot, i) => {
      const state = this.record_states[i];

      // const { bed_record, path_name, path_interval } = state.record;

      const bed = state.record.bed_record;

      //// set the stroke and fill colors on the root svg element

      let color;

      if (typeof bed.itemRgb === "string") {
        let [r, g, b] = bed.itemRgb.split(',');
        color = `rgb(${r * 255},${g * 255},${b * 255})`;
      } else {
        let { r, g, b } = wasm_bindgen.path_name_hash_color_obj(bed.name);
        color = `rgb(${r * 255},${g * 255},${b * 255})`;
      }

      state.color = color;

      // state.svg_g.setAttribute('stroke', color);
      state.svg_g.setAttribute('color', color);

      //// global coordinate space rectangles for the 1D path views
      const global_ranges = [];

      for (const [start, end] of annot.blocks_1d_bp) {
        global_ranges.push({ start, end });
      }

      state.global_ranges = global_ranges;

      state.step_endpoints = { first: annot.first_step, last: annot.last_step };
      state.start_bp = annot.start_bp;
      state.end_bp = annot.end_bp;

      state.path_steps = Uint32Array.from(annot.path_steps);

      state.start_world_2d = vec2.fromValues(annot.start_world_x, annot.start_world_y);
      state.end_world_2d = vec2.fromValues(annot.end_world_x, annot.end_world_y);

    });
  }


  async resample2DPaths() {

    const canvas = document.getElementById("graph-viewer-2d") as HTMLCanvasElement;
    const svg_rect =
      document.getElementById('viz-svg-overlay')
        .getBoundingClientRect();

    const svg_height_prop = canvas.height / svg_rect.height;


    let resample = false;

    // if (view_2d_obj === null && this.last_2d_view === null) {
    //   return;
    // }

    let view_2d_obj = this.waragraph.graph_viewer!.getView();

    if (this.last_2d_view === null) {
      resample = true;
    } else {
      if (this.last_2d_view !== view_2d_obj) {
        resample = true;
      }
    }

    if (resample === false) {
      return;
    }

    this.last_2d_view = view_2d_obj;

    const viewMatrix = this.waragraph.graph_viewer!.getViewMatrix();

    // let's say tolerance should be 5px
    const view_obj = this.waragraph.graph_viewer!.getView();
    const world_per_px = view_obj.width / canvas.width;
    const tolerance = 5.0 * world_per_px;

    const map_canvas_to_svg = (v) => {
      let x_ = 100 * v[0] / canvas.width;
      let y_ = 100 * svg_height_prop * v[1] / canvas.height;
      return vec2.fromValues(x_, y_);
    };

    // for (let { record, cached_path, enabled } of this.record_states) {
    for (const state of this.record_states) {
      if (!state.enabled) {
        continue;
      }

      const { path_name, path_interval, bed_record } = state.record;

      let update_path = state.cached_path === undefined;

      if (state.cached_path !== undefined) {
        // TODO tune
        if (Math.abs(tolerance - state.cached_path.tolerance) > 10.0) {
          update_path = true;
        }
      }

      // TODO: asynchronously update the SVG path string, rather than wait on each
      // in a loop (use SVG transform for translations)
      if (update_path) {
        const path = 
          this.waragraph.graphLayoutTable!
            .sample2DPath(state.path_steps, tolerance);

        if (path === undefined) {
          console.error("Error sampling 2D path, ignoring");
          continue;
        }

        state.cached_path = { path, tolerance };

      }

      let svg_path = "";

      if (state.cached_path !== undefined) {
        for (let i = 0; i < state.cached_path.path.length; i += 2) {
          let x = state.cached_path.path[i];
          let y = state.cached_path.path[i + 1];
          let p = vec2.fromValues(x, y);
          // these are world space; need to apply 2D view matrix 
          let q = vec2.create();
          vec2.transformMat3(q, p, viewMatrix);

          let r = map_canvas_to_svg(q);

          if (svg_path.length === 0) {
            svg_path += `M ${r[0]},${r[1]}`;
          } else {
            svg_path += ` L ${r[0]},${r[1]}`;
          }
        }
      }

      state.svg_g.querySelector('.svg-overlay-2d > path').outerHTML =
        // svg_g.innerHTML =
        // `<path d="${svg_path}" stroke-width="0.5" fill="none" />`;
        `<path d="${svg_path}" mask="url(#mask-2d-view)" stroke-width="0.5" stroke="${state.color}" fill="none" />`;
      // `<path d="${svg_path}" stroke-width="0.5" stroke="red" fill="none" />`;
    }
  }

  async updateSVG1D(view_1d) {

    const svg_rect = document.getElementById('viz-svg-overlay').getBoundingClientRect();
    const data_list_rect =
      document.getElementById('path-viewer-container').getBoundingClientRect();

    const view_len = view_1d.end - view_1d.start;


    const map_pos = (x, y) => {
      return {
        x: 100 * (x - svg_rect.left) / svg_rect.width,
        y: 100 * (y - svg_rect.top) / svg_rect.height
      };
    };

    const pathSlotVisible = (path_name) => {
      const data_canvas = document.getElementById('viewer-' + path_name);
      if (data_canvas && 'path_viewer' in data_canvas) {
        const viewer = data_canvas.path_viewer as PathViewer;
        return viewer.isVisible;
      }

      return false;
    };

    // for (const { svg_g, record, global_ranges, enabled, color } of this.record_states) {
    for (const record_state of this.record_states) {
      const { svg_g, record, global_ranges, color } = record_state;

      if (global_ranges === undefined) {
        continue;
      }

      const is_1d_visible = pathSlotVisible(record.path_name);

      const link_start = svg_g.querySelector('.svg-overlay-link-start') as SVGLineElement;
      const link_end = svg_g.querySelector('.svg-overlay-link-end') as SVGLineElement;

      const start_pos = this.waragraph.globalBpToPathViewerPos(record.path_name, record_state.start_bp);
      const end_pos = this.waragraph.globalBpToPathViewerPos(record.path_name, record_state.end_bp);

      // if (start_pos === null || end_pos === null) {
      //   continue;
      // }

      let f_p, l_p;
      if (is_1d_visible) {
        f_p = map_pos(start_pos.x, start_pos.y0);
        l_p = map_pos(end_pos.x, end_pos.y1);
      } else {
        f_p = map_pos(start_pos.x, data_list_rect.top);
        l_p = map_pos(end_pos.x, data_list_rect.top);
      }


      link_start.setAttribute('x2', String(f_p.x));
      link_end.setAttribute('x2', String(l_p.x));
      link_start.setAttribute('y2', String(f_p.y));
      link_end.setAttribute('y2', String(l_p.y));

      const svg_g_1d = svg_g.querySelector('.svg-overlay-1d');
      svg_g_1d.setAttribute('mask', 'url(#mask-path-viewers)');

      const data_canvas = document.getElementById('viewer-' + record.path_name);
      const data_rect = data_canvas.getBoundingClientRect();

      svg_g_1d.innerHTML = "";
      for (const { start, end } of global_ranges) {
        const el_rect = createSVGElement('rect');

        if (!is_1d_visible) {
          continue;
        }

        svg_g_1d.append(el_rect);

        // map global range to `data_rect` via `view_1d`
        let r_start = (start - view_1d.start) / view_len;
        let r_end = (end - view_1d.start) / view_len;

        let screen_rs_xl = data_rect.left + r_start * data_rect.width;
        let screen_rs_xr = data_rect.left + r_end * data_rect.width;

        let y = 100 * (data_rect.top - svg_rect.top) / svg_rect.height;
        let xl = 100 * (screen_rs_xl - svg_rect.left) / svg_rect.width;
        let xr = 100 * (screen_rs_xr - svg_rect.left) / svg_rect.width;

        let width = xr - xl;
        let height = 100 * data_rect.height / svg_rect.height;

        el_rect.outerHTML = `<rect x="${xl}" y="${y}" width="${width}" height="${height}"
fill="${color}"
/>`;
      }

    }

  }

  async updateSVGPaths() {
    const canvas = document.getElementById("graph-viewer-2d") as HTMLCanvasElement;
    const w = canvas.width;
    const h = canvas.height;

    // const canvas_rect = canvas.getBoundingClientRect();

    const svg_rect =
      document.getElementById('viz-svg-overlay')
        .getBoundingClientRect();

    const svg_height_prop = canvas.height / svg_rect.height;

    const map_canvas_to_svg = ({ x, y }) => {
      let x_ = 100 * x / canvas.width;
      let y_ = 100 * svg_height_prop * y / canvas.height;
      return { x: x_, y: y_ };
    };

    const view_mat = this.waragraph.graph_viewer!.getViewMatrix();

    // for (const { svg_g, record, cached_path, enabled, color } of this.record_states) {
    for (const record_state of this.record_states) {
      const { svg_g, record, cached_path, enabled, color } = record_state;

      if (!enabled || cached_path === undefined) {
        // svg_g.innerHTML = '';
        // svg_g.style.setProperty('display', 'none');
        continue;
      }

      const link_start = svg_g.querySelector('.svg-overlay-link-start') as SVGLineElement;
      const link_end = svg_g.querySelector('.svg-overlay-link-end') as SVGLineElement;

      let interval = record.path_interval;

      const first_pos = vec2.create();
      const last_pos = vec2.create();
      vec2.transformMat3(first_pos, record_state.start_world_2d, view_mat);
      vec2.transformMat3(last_pos, record_state.end_world_2d, view_mat);
      const f_p = map_canvas_to_svg({ x: first_pos[0], y: first_pos[1] });
      const l_p = map_canvas_to_svg({ x: last_pos[0], y: last_pos[1] });

      link_start.setAttribute('x1', String(f_p.x));
      link_start.setAttribute('y1', String(f_p.y));
      link_end.setAttribute('x1', String(l_p.x));
      link_end.setAttribute('y1', String(l_p.y));

      if (color !== undefined) {
        link_start.setAttribute('stroke', color);
        link_end.setAttribute('stroke', color);
      }

      const label_2d = svg_g.querySelector('.svg-overlay-2d > text');
      label_2d.setAttribute('x', `${f_p.x}`);
      label_2d.setAttribute('y', `${f_p.y}`);
      label_2d.setAttribute('color', 'red');
      label_2d.setAttribute('font-size', '1.7');
      label_2d.setAttribute('font-family', 'sans-serif');
      label_2d.innerHTML = `${record.bed_record.name}`;

      const svg_path =
        svg_g.querySelector('.svg-overlay-2d > path');

    }

  }

}

