import init_module, * as wasm_bindgen from 'waragraph';

// import BED from '@gmod/bed';

// import {computePosition} from '@floating-ui/dom';

// import { preparePathHighlightOverlay } from '../graph_viewer';

import { BEDRecord } from './sidebar-bed';

// import { WaragraphViz } from './index';
import { type Waragraph } from './waragraph';

import * as CanvasTracks from './canvas_tracks';
import { PathViewer } from './path_viewer_ui';


function createSVGElement(tag) {
  return document.createElementNS('http://www.w3.org/2000/svg', tag);
}

interface AnnotationRecord {
  svg_g: SVGGElement;
  record: BEDRecord;
  enabled: boolean;

  global_ranges: Array<{ start: number, end: number }> | null;
  cached_path: wasm_bindgen.CanvasPathTrace | null;

  color?: string;
}

let _wasm;

export class AnnotationPainter {
  callback_key: string;
  waragraph: Waragraph;
  svg_root: SVGSVGElement;
  record_states: AnnotationRecord[];

  last_2d_view: { x: number, y: number, width: number, height: number } | null;

  constructor(waragraph: Waragraph, name: string, records: Iterable<BEDRecord>) {
    this.callback_key = "painter-" + name;

    if (!_wasm) {
      init_module(undefined, waragraph.wasm.memory)
        .then((module) => {
          _wasm = module;
          wasm_bindgen.set_panic_hook();
        });
    }

    // this.record_svg_gs = [];
    // this.svg_parent = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    // this.record_canvas_paths = [];

    this.waragraph = waragraph;

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

        global_ranges: null,
        cached_path: null,
      });

      this.svg_root.append(g_el);
    }
  }

  async prepareRecords() {

    const viewport = await this.waragraph.get1DViewport();
    if (viewport === undefined) {
      throw new Error("No viewport available");
    }
    // const cs_view = await this.waragraph.worker_obj.globalCoordSysView();

    for (const state of this.record_states) {
      const { bed_record, path_name, path_step_slice } = state.record;

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

      const record_ranges = wasm_bindgen.path_slice_to_global_adj_partitions(path_step_slice);

      const ranges_arr = record_ranges.ranges_as_u32_array();
      const range_count = ranges_arr.length / 2;

      const global_ranges = [];


      for (let ri = 0; ri < range_count; ri++) {
        let start_seg = ranges_arr.at(2 * ri);
        let end_seg = ranges_arr.at(2 * ri + 1);

        // if (end_seg === undefined) {
        //   end_seg = start_seg + 1;
        // }

        if (start_seg !== undefined && end_seg !== undefined) {
          let start = viewport.segmentOffset(start_seg);
          let end = viewport.segmentOffset(end_seg);

          global_ranges.push({ start, end });
        }
      }

      state.global_ranges = global_ranges;
    }
  }


  resample2DPaths() {

    const canvas = document.getElementById("graph-viewer-2d") as HTMLCanvasElement;
    const svg_rect =
      document.getElementById('viz-svg-overlay')
        .getBoundingClientRect();

    const svg_height_prop = canvas.height / svg_rect.height;

    const map_canvas_to_svg = ({ x, y }) => {
      let x_ = 100 * x / canvas.width;
      let y_ = 100 * svg_height_prop * y / canvas.height;
      return { x: x_, y: y_ };
    };

    const path_tolerance = 8;

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
      // let { width, height } = this.last_2d_view;
      // if (view_2d_obj.width !== width
      //   || view_2d_obj.height !== height) {
      //   resample = true;
      // }
    }

    if (resample === false) {
      return;
    }

    this.last_2d_view = view_2d_obj;

    // for (let { record, cached_path, enabled } of this.record_states) {
    for (const state of this.record_states) {
      if (!state.enabled) {
        continue;
      }

      const { path_name, path_step_slice, bed_record } = state.record;

      state.cached_path =
        this.waragraph.graph_viewer!
          .sampleCanvasSpacePath(path_step_slice, path_tolerance);


      let svg_path = "";

      state.cached_path.with_points((x, y) => {
        const p = map_canvas_to_svg({ x, y });

        if (svg_path.length === 0) {
          svg_path += `M ${p.x},${p.y}`;
        } else {
          svg_path += ` L ${p.x},${p.y}`;
        }
      });

      state.svg_g.querySelector('.svg-overlay-2d > path').outerHTML =
        // svg_g.innerHTML =
        // `<path d="${svg_path}" stroke-width="0.5" fill="none" />`;
        `<path d="${svg_path}" stroke-width="0.5" stroke="${state.color}" fill="none" />`;
      // `<path d="${svg_path}" stroke-width="0.5" stroke="red" fill="none" />`;

      // console.warn(state.cached_path);
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

      if (global_ranges === null) {
        continue;
      }

      const is_1d_visible = pathSlotVisible(record.path_name);

      const link_start = svg_g.querySelector('.svg-overlay-link-start') as SVGLineElement;
      const link_end = svg_g.querySelector('.svg-overlay-link-end') as SVGLineElement;

      const first_seg = record.path_step_slice.at(0) >> 1;
      const last_seg = record.path_step_slice.at(-1) >> 1;

      const first_pos = await this.waragraph.segmentScreenPos1d(record.path_name, first_seg);
      const last_pos = await this.waragraph.segmentScreenPos1d(record.path_name, last_seg);

      let f_p, l_p;
      if (is_1d_visible) {
        f_p = map_pos(first_pos.x0, first_pos.y0);
        l_p = map_pos(last_pos.x1, last_pos.y1);
      } else {
        f_p = map_pos(first_pos.x0, data_list_rect.top);
        l_p = map_pos(last_pos.x1, data_list_rect.top);
      }


      link_start.setAttribute('x2', String(f_p.x));
      link_end.setAttribute('x2', String(l_p.x));
      link_start.setAttribute('y2', String(f_p.y));
      link_end.setAttribute('y2', String(l_p.y));

      const svg_g_1d = svg_g.querySelector('.svg-overlay-1d');

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

  updateSVGPaths() {
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

    // for (const { svg_g, record, cached_path, enabled, color } of this.record_states) {
    for (const record_state of this.record_states) {
      const { svg_g, record, cached_path, enabled, color } = record_state;

      if (!enabled || cached_path === null) {
        // svg_g.innerHTML = '';
        // svg_g.style.setProperty('display', 'none');
        continue;
      }

      const link_start = svg_g.querySelector('.svg-overlay-link-start') as SVGLineElement;
      const link_end = svg_g.querySelector('.svg-overlay-link-end') as SVGLineElement;

      const first_seg = record.path_step_slice.at(0) >> 1;
      const last_seg = record.path_step_slice.at(-1) >> 1;

      const first_pos = this.waragraph.segmentScreenPos2d(first_seg);
      const last_pos = this.waragraph.segmentScreenPos2d(last_seg);

      const f_p = map_canvas_to_svg({ x: first_pos.start[0], y: first_pos.start[1] });
      const l_p = map_canvas_to_svg({ x: last_pos.end[0], y: last_pos.end[1] });

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

      // console.warn(svg_path);

      /*
      let svg_path = "";

      cached_path.with_points((x, y) => {
        const p = map_canvas_to_svg({ x, y });

        if (svg_path.length === 0) {
          svg_path += `M ${p.x},${p.y}`;
        } else {
          svg_path += ` L ${p.x},${p.y}`;
        }
      });

      svg_g.querySelector('.svg-overlay-2d').innerHTML =
        // svg_g.innerHTML =
        // `<path d="${svg_path}" stroke-width="0.5" fill="none" />`;
        `<path d="${svg_path}" stroke-width="0.5" stroke="${color}" fill="none" />`;
      // `<path d="${svg_path}" stroke-width="0.5" stroke="red" fill="none" />`;
       */
    }

  }


  // this would be nice to have, but the cached paths are entirely
  // canvas space -- what i want is to have the sampled paths in world
  // space (i.e. the number of samples should depend on the apparent
  // size of the path on the canvas, but the points are in world
  // space)
  /*
followAnnotationPath(record_name) {
  const record = this.record_states.find((state) => state.record.bed_record.name == record_name);
  if (record === undefined) {
    return;
  }

  const cached_path = record.cached_path;

  if (cached_path === null) {
    return;
  }


}
   */

}
