import init_module, * as wasm_bindgen from 'waragraph';

import { preparePathHighlightOverlay } from '../graph_viewer';
import { AnnotationPainter } from './annotations';
import * as CanvasTracks from '../canvas_tracks';
import { wrapWasmPtr } from './wrap';

import { type Waragraph } from './waragraph';

import BED from '@gmod/bed';

import * as rxjs from 'rxjs';
import { computePosition } from '@floating-ui/dom';

import '../sidebar-bed.css';

let pathNamesMap = new Map();

let _context_menu_entry = null;

function findPathName(cand: string): string {
  if (pathNamesMap.has(cand)) {
    return cand;
  }

  for (const path_name of pathNamesMap.keys()) {
    if (path_name.startsWith(cand)) {
      return path_name;
    }
  }
}


// let waragraph_viz: WaragraphViz;
let wasm;

function createPathOffsetMap(path_name: string): (bp: number) => number {
  const regex = /.+:(\d+)-(\d+)$/;
  if (!path_name) {
    throw `Path ${path_name} not found`;
  }
  const found = path_name.match(regex);

  const start = found === null ? 0 : Number(found[1]);

  return (bp) => bp - start;
}

interface PathRange {
  start: number,
  end: number,
}

function bedToPathRange(bed_entry, path_name: string): PathRange {
  if (!path_name) {
    throw `Path ${path_name} not found`;
  }

  const regex = /.+:(\d+)-(\d+)$/;

  const found = path_name.match(regex);

  if (found === null) {
    return bed_entry;
  }

  const start = Number(found[1]);

  let chromStart = bed_entry.chromStart - start;
  let chromEnd = bed_entry.chromEnd - start;

  if (chromStart < 0) {
    chromStart = 0;
  }

  return { start: chromStart, end: chromEnd };
}

function transformBedRange(bed_entry) {
  let name = bed_entry.chrom;

  let path_name = findPathName(name);

  if (!path_name) {
    throw `Path ${path_name} not found`;
  }

  const regex = /.+:(\d+)-(\d+)$/;

  const found = path_name.match(regex);

  if (found === null) {
    return bed_entry;
  }

  const start = Number(found[1]);

  let chromStart = bed_entry.chromStart - start;
  let chromEnd = bed_entry.chromEnd - start;

  if (chromStart < 0) {
    chromStart = 0;
  }

  // TODO pass coordinate max in here, maybe;
  // if (chromEnd > max) {
  // }

  const new_entry = Object.assign({}, bed_entry, { chromStart, chromEnd });

  return new_entry;
}

function bedEntryColorOrFn(bed_entry, color_fn) {
  let color;

  if (typeof bed_entry.itemRgb === "string") {
    let [r, g, b] = bed_entry.itemRgb.split(',');
    color = `rgb(${r * 255},${g * 255},${b * 255})`;
  } else if (bed_entry.color) {
    color = bed_entry.color;
  } else {
    let { r, g, b } = wasm_bindgen.path_name_hash_color_obj(bed_entry.name);
    color = `rgb(${r * 255},${g * 255},${b * 255})`;
  }

  if (typeof color_fn === 'function') {
    color = color_fn(bed_entry);
  }

  return color;
}

/*
async function createDrawBedEntryFn1d(bed_entry, color_fn) {
    //
    let path_name = findPathName(bed_entry.chrom);
    let path_offset_map = createPathOffsetMap(path_name);

    let graph_raw = await waragraph_viz.worker_obj.getGraph();
    let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
    let path_steps = graph.path_steps(path_name);

    // let color = bedEntryColorOrFn(bed_entry, color_fn);

    let entry = { start: bed_entry.chromStart,
                  end: bed_entry.chromEnd,
                  label: bed_entry.name };

    entry.color = bedEntryColorOrFn(bed_entry, color_fn);

    const cs_view = await waragraph_viz.worker_obj.globalCoordSysView();

    let global_entries = [];

    try {
        // console.warn(entry);
        let path_range = await waragraph_viz
            .worker_obj
            .pathRangeToStepRange(path_name, entry.start, entry.end);

        // console.warn(path_range);
        let slice = path_steps.slice(path_range.start, path_range.end);
        // console.warn(slice);

        // console.log("steps in ", bed_entry.name, ": ", slice.length);

        let seg_ranges = wasm_bindgen.path_slice_to_global_adj_partitions(slice);
        let seg_ranges_arr = seg_ranges.ranges_as_u32_array();
        let range_count = seg_ranges_arr.length / 2;

        for (let ri = 0; ri < range_count; ri++) {
            let start_seg = seg_ranges_arr.at(2 * ri);
            let end_seg = seg_ranges_arr.at(2 * ri + 1);

            if (start_seg !== undefined && end_seg !== undefined) {
                let start = await cs_view.segmentOffset(start_seg);
                let end = await cs_view.segmentOffset(end_seg);

                global_entries.push({start, end, color: entry.color});
            }
        }
    } catch (e) {
        // TODO
        throw "Error creating 1D highlight track: " + e;
    }

    let callback = CanvasTracks.createHighlightCallback(global_entries);

    return callback;
}

async function createDrawBedEntryFn2d(bed_entry, color_fn) {
    // console.log(bed_entry);
    let path_name = findPathName(bed_entry.chrom);
    // console.log(path_name);
    let path_offset_map = createPathOffsetMap(path_name);

    let seg_pos = waragraph_viz.graph_viewer.segment_positions;

    let graph_raw = await waragraph_viz.worker_obj.getGraph();
    let graph = wasm_bindgen.ArrowGFAWrapped.__wrap(graph_raw.__wbg_ptr);
    let path_steps = graph.path_steps(path_name);

    let path_cs = await waragraph_viz.worker_obj.pathCoordSys(path_name);

    let entry = { start: bed_entry.chromStart,
                  end: bed_entry.chromEnd,
                  label: bed_entry.name };

    entry.color = bedEntryColorOrFn(bed_entry, color_fn);

    let callback_2d = 
        preparePathHighlightOverlay(seg_pos,
                                    path_steps,
                                    path_cs,
                                    [entry]);

    return callback_2d;
}
*/

export interface BEDRecord {
  record_index: number;
  bed_record: any;

  path_name: string;
  path_range: PathRange;
  path_step_slice: Uint32Array;
}

class BEDFile {
  file_name: string;
  records: BEDRecord[];

  annotation_painter: AnnotationPainter | null;

  // constructor(file_name, record_lines) {
  constructor(file_name: string) {
    this.file_name = file_name;
    this.records = [];

    this.annotation_painter = null;

    // per record: 
    // { canvas_1d: Map<Path, bool>,
    //   canvas_2d: bool,
    //   svg_shared: svg element
  }

  async appendRecords(waragraph: Waragraph, record_lines) {

    const graph = waragraph.graph;

    for (const bed_record of record_lines) {
      if (Number.isNaN(bed_record.chromStart)
        || Number.isNaN(bed_record.chromEnd)) {
        continue;
      }

      const record_i = this.records.length;

      const path_name = findPathName(bed_record.chrom);
      const path_range = bedToPathRange(bed_record, path_name);

      const path_cs = await waragraph.getCoordinateSystem("path:" + path_name);

      if (path_cs === undefined) {
        throw new Error(`Could not find coordinate system for path ${path_name}`);
      }
      // const path_cs_raw = await waragraph_viz.worker_obj.pathCoordSys(path_name);
      // const path_cs = wrapWasmPtr(wasm_bindgen.CoordSys, path_cs_raw.__wbg_ptr);

      const path_steps = graph.path_steps(path_name);

      const step_range = path_cs.bp_to_step_range(BigInt(path_range.start), BigInt(path_range.end)) as { start: number, end: number };
      // const step_range = path_cs.bp_to_step_range(path_range.start, path_range.end);
      const path_step_slice = path_steps.slice(step_range.start, step_range.end);

      const record = {
        record_index: record_i,
        bed_record,
        path_name,
        path_range,
        path_step_slice,
      };

      this.records.push(record);

    }
  }

  async initializeAnnotationPainter(waragraph: Waragraph) {
    this.annotation_painter =
      new AnnotationPainter(waragraph, this.file_name, this.records);

    await this.annotation_painter.prepareRecords();

    let prev_view = waragraph.graph_viewer?.getView();

    document.getElementById('viz-svg-overlay')
      .append(this.annotation_painter.svg_root);

    // TODO: there are other cases when this should run, especially once
    // there's more than just 2D-focused SVG
    waragraph
      .graph_viewer?.view_subject
      .subscribe((view_2d) => {
        const { x, y, width, height } = view_2d;

        let update_pos = false;

        // add this back when paths' transforms are just updated on pan
        // if (prev_view.width !== width
        //     || prev_view.height !== height) {
        //     update_pos = true;

        //     this.annotation_painter.resample2DPaths(view_2d);
        // }

        if (prev_view.x != x || prev_view.y !== y || update_pos) {
          // update SVG path offsets;
          // should also happen on resample

          this.annotation_painter.resample2DPaths();
          this.annotation_painter.updateSVGPaths();
          prev_view = view_2d;
        }

      });

    const viewport = await waragraph.get1DViewport();

    viewport?.subject
      .pipe(rxjs.distinct(),
        rxjs.throttleTime(50),
      )
      .subscribe((view_1d) => {
        this.annotation_painter.updateSVG1D(view_1d);
      });


  }

  recordAnnotationVizState(record_index: number) {
    return this.annotation_painter.record_states[record_index];
  }

  createListElement(): HTMLDivElement {
    const entries_list = document.createElement('div');
    entries_list.classList.add('bed-file-entry');

    const name_el = document.createElement('div');
    name_el.innerHTML = this.file_name;
    name_el.classList.add('bed-file-name');
    name_el.style.setProperty('flex-basis', '30px');
    entries_list.append(name_el);

    for (const record of this.records) {

      const entry_div = document.createElement('div');
      entry_div.classList.add('bed-file-row');
      // entry_div.style.setProperty('flex-basis', '20px');

      const label_div = document.createElement('div');
      label_div.innerHTML = record.bed_record.name;
      label_div.classList.add('bed-row-label');
      // label_div.classList.add('highlight-enabled');


      // add checkboxes/buttons for toggling... or just selection?
      // still want something to signal visibility in 1d & 2d, e.g. "eye icons"

      const viz_states = this.annotation_painter.record_states;

      label_div.addEventListener('click', (ev) => {
        let state = viz_states[record.record_index];

        let svg_g_1d = state.svg_g.querySelector('.svg-overlay-1d') as SVGGElement;
        let svg_g_2d = state.svg_g.querySelector('.svg-overlay-2d') as SVGGElement;

        let ev_tgt = ev.target as HTMLElement;

        if (state.enabled) {
          ev_tgt.classList.remove('highlight-enabled');
          // svg_g_1d.setAttribute('display', 'none');
          // svg_g_2d.setAttribute('display', 'none');

          for (const child of state.svg_g.children) {
            child.setAttribute('display', 'none');
          }

          state.enabled = false;
        } else {
          ev_tgt.classList.add('highlight-enabled');
          // svg_g_1d.setAttribute('display', 'inline');
          // svg_g_2d.setAttribute('display', 'inline');

          for (const child of state.svg_g.children) {
            child.setAttribute('display', 'inline');
          }

          this.annotation_painter.resample2DPaths();
          this.annotation_painter.updateSVGPaths();

          state.enabled = true;
        }
      });

      // disabling context menu for now
      /*
      label_div.addEventListener('click', (ev) => {
          ev.stopPropagation();
          let ctx_menu_el = document.getElementById('sidebar-bed-context-menu');
          _context_menu_entry = { record };
          // _context_menu_entry = { bed_entry,
          //                         processed: entry,
          //                         path_name, 
          //                         path_range,
          //                       };

          computePosition(label_div, ctx_menu_el).then(({x, y}) => {
              ctx_menu_el.style.setProperty('display', 'flex');
              ctx_menu_el.focus();
              Object.assign(ctx_menu_el.style, {
                  left: `${x}px`,
                  top: `${y}px`,
              });
          });
      });
      */

      entry_div.append(label_div);

      entries_list.append(entry_div);
    }

    return entries_list;
  }


}

async function loadBedFile(waragraph: Waragraph, file: File) {
  const bed_file = new BEDFile(file.name);
  const bed_text = await file.text();

  const parser = new BED();
  const bed_lines = bed_text.split('\n').map(line => parser.parseLine(line));

  await bed_file.appendRecords(waragraph, bed_lines);

  bed_file.initializeAnnotationPainter(waragraph);

  const bed_list = document.getElementById('bed-file-list');

  const bed_list_el = bed_file.createListElement();
  bed_list.append(bed_list_el);

}

// Build controls panel
// TODO: extract to method reusable statements
async function controlSidebarPanel(waragraph) {
  const controls_div = document.createElement('div');
  controls_div.classList.add('bed-panel');

  const pane_title = document.createElement('h5');
  pane_title.innerHTML = 'Control Panel';
  pane_title.classList.add('mt-2');


  const break_el = document.createElement('hr');
  break_el.classList.add('my-1')

  const control_range_label = document.createElement('label');
  control_range_label.innerHTML = 'Jump to 1D range';

  const range_input_row = document.createElement('div');
  range_input_row.classList.add('row');

  const label_div = document.createElement('div');
  label_div.title = 'label-group';

  const input_div = document.createElement('div');
  input_div.title = 'input-group';

  const label_start = document.createElement('label');
  label_start.textContent = 'Start:';
  label_start.htmlFor = 'control-input-range-start';
  label_start.classList.add('full-width');
  label_start.style.height = '50%'

  const input_start = document.createElement('input');
  input_start.type = 'text';
  input_start.id = 'control-input-range-start';
  input_start.placeholder = 'Start';
  input_start.setAttribute('type', 'text');
  input_start.setAttribute('inputmode', 'numeric');
  input_start.setAttribute('pattern', '\\d*');
  input_start.setAttribute('min', '0')
  input_start.setAttribute('step', '1')
  input_start.classList.add('full-width')

  const label_end = document.createElement('label');
  label_end.textContent = 'End:';
  label_end.htmlFor = 'control-input-range-end';
  label_end.classList.add('full-width');
  label_end.style.height = '50%'

  const input_end = document.createElement('input');
  input_end.type = 'text';
  input_end.id = 'control-input-range-end';
  input_end.placeholder = 'End';
  input_end.setAttribute('type', 'text');
  input_end.setAttribute('inputmode', 'numeric');
  input_end.setAttribute('pattern', '\\d*');
  input_end.setAttribute('min', '0')
  input_end.setAttribute('step', '1')
  input_end.classList.add('full-width');

  label_div.appendChild(label_start);
  label_div.appendChild(label_end);
  label_div.classList.add('col-2');

  input_div.appendChild(input_start);
  input_div.appendChild(input_end);
  input_div.classList.add('col-10');

  range_input_row.appendChild(label_div);
  range_input_row.appendChild(input_div);

  const input_group = document.createElement('div');
  input_group.title = 'input-group';
  input_group.classList.add('col-12');

  const input_button = document.createElement('button');
  input_button.type = 'button';
  input_button.id = 'control-input-range-button';
  input_button.classList.add('full-width');
  input_button.textContent = 'Go'; 
  input_group.appendChild(input_button);

  controls_div.appendChild(pane_title);
  controls_div.appendChild(break_el);
  controls_div.appendChild(control_range_label);
  controls_div.appendChild(range_input_row);
  controls_div.appendChild(input_group);

  return controls_div;
}

async function bedSidebarPanel(waragraph) {
  const bed_pane = document.createElement('div');
  bed_pane.classList.add('bed-panel');

  const pane_title = document.createElement('h5');
  pane_title.innerHTML = 'BED Panel';
  pane_title.classList.add('mt-2');

  const break_el = document.createElement('hr');
  break_el.classList.add('my-1')

  const bed_list = document.createElement('div');
  bed_list.id = 'bed-file-list';


  const file_label = document.createElement('label');
  file_label.setAttribute('for', 'bed-file-input');
  file_label.innerHTML = 'Load BED file';

  const file_entry = document.createElement('input');
  file_entry.id = 'bed-file-input';
  file_entry.setAttribute('type', 'file');
  file_entry.setAttribute('name', 'bed-file-input');
  file_entry.setAttribute('accept', '.bed');

  const file_button = document.createElement('button');
  file_button.innerHTML = 'Load';

  file_button.addEventListener('click', (ev) => {
    for (const file of file_entry.files) {
      loadBedFile(waragraph, file);
    }
  });

  bed_pane.append(pane_title);
  bed_pane.append(break_el)
  bed_pane.append(file_label);
  bed_pane.append(file_entry);
  bed_pane.append(file_button);
  bed_pane.append(bed_list);

  {

    let graph = waragraph.graph;

    const context_menu_el = document.createElement('div');
    context_menu_el.id = 'sidebar-bed-context-menu';

    const copy_name_btn = document.createElement('button');

    copy_name_btn.innerHTML = 'Copy name';
    copy_name_btn.addEventListener('click', (ev) => {
      if (_context_menu_entry !== null) {
        let name = _context_menu_entry.record.name;
        if (typeof name === "string") {
          navigator.clipboard.writeText(name);
          // context_menu_el.style.setProperty('display', 'none');
          // _context_menu_entry = null;
        }
      }
    })

    // TODO context menu needs access to the viz. state
    /*
    const focus_2d_btn = document.createElement('button');
    focus_2d_btn.innerHTML = 'Focus 2D view';
    focus_2d_btn.addEventListener('click', async (ev) => {
        if (_context_menu_entry === null) {
            return;
        }

        // TODO use something sensible; not just the first step

        let path_name = _context_menu_entry.path_name;
        let path_slice = _context_menu_entry.path_step_slice;

        let first_node = path_slice[0] / 2;

        // let { start, end } = path_range;

        let path_steps = graph.path_steps(path_name);
        let seg = path_steps[first_node];

        waragraph_viz.centerViewOnSegment2d(seg);
    });
    */

    context_menu_el.append(copy_name_btn);
    // context_menu_el.append(focus_2d_btn);

    document.body.append(context_menu_el);
  }

  document.addEventListener('click', (ev) => {
    let tgt = ev.target as HTMLElement;
    let id = "sidebar-bed-context-menu";
    let ctx_menu_el = document.getElementById(id);
    let ctx_open = ctx_menu_el.style.display === 'flex';
    if (!tgt.closest('#' + id) && ctx_open) {
      ctx_menu_el.style.setProperty('display', 'none');
      _context_menu_entry = null;
    }
  });

  return bed_pane;
}



export async function initializeBedSidebarPanel(waragraph: Waragraph) {
  // waragraph_viz = warapi;

  if (!wasm) {
    wasm = await init_module(undefined, waragraph.wasm.memory);
    wasm_bindgen.set_panic_hook();
  }

  let path_id = 0;
  waragraph.graph.with_path_names((name: string) => {
    pathNamesMap.set(name, path_id);
    path_id += 1;
  });

  document
    .getElementById('sidebar-bed')!
    .append(await bedSidebarPanel(waragraph));

  document
    .getElementById('sidebar-controls')!
    .append(await controlSidebarPanel(waragraph));
}
