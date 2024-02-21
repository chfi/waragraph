import init_module, * as wasm_bindgen from 'waragraph';

import { preparePathHighlightOverlay } from './graph_viewer';
import { AnnotationGeometry, AnnotationPainter } from './annotations';
import * as CanvasTracks from './canvas_tracks';
import { wrapWasmPtr } from './wrap';

// import { type Waragraph } from './waragraph';
import { type Waragraph } from './waragraph_client';

import BED from '@gmod/bed';

import * as rxjs from 'rxjs';
import { computePosition } from '@floating-ui/dom';

import '../sidebar-bed.css';
import { PathInterval, PathNameInterval } from './types';
import { ArrowGFA } from './graph_api';

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


function bedToPathInterval(bed_entry, path_name: string): PathNameInterval {
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

  return { path_name, start: chromStart, end: chromEnd };
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

export interface BEDRecord {
  record_index: number;
  bed_record: any;

  path_name: string;
  path_interval: PathInterval;
  // path_step_slice: Uint32Array;
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

  async appendRecords(
    graph: ArrowGFA,
    prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>,
    record_lines
  ) {

    for (const bed_record of record_lines) {
      if (Number.isNaN(bed_record.chromStart)
        || Number.isNaN(bed_record.chromEnd)) {
        continue;
      }

      const record_i = this.records.length;

      const path_name = findPathName(bed_record.chrom);
      const path_name_interval = bedToPathInterval(bed_record, path_name);

      // const path_cs = await waragraph.getCoordinateSystem("path:" + path_name);

      // if (path_cs === undefined) {
      //   throw new Error(`Could not find coordinate system for path ${path_name}`);
      // }
      // const path_cs_raw = await waragraph_viz.worker_obj.pathCoordSys(path_name);
      // const path_cs = wrapWasmPtr(wasm_bindgen.CoordSys, path_cs_raw.__wbg_ptr);

      // const path_steps = graph.path_steps(path_name);

      // const step_range = path_cs.bp_to_step_range(BigInt(path_interval.start), BigInt(path_interval.end)) as { start: number, end: number };
      // const step_range = path_cs.bp_to_step_range(path_range.start, path_range.end);
      // const path_step_slice = path_steps.slice(step_range.start, step_range.end);

      const path_id = await graph.pathIdFromName(path_name);
      const path_interval = { path_id, start: path_name_interval.start, end: path_name_interval.end };

      const record = {
        record_index: record_i,
        bed_record,
        path_name,
        path_interval,
        // path_step_slice,
      };

      this.records.push(record);

    }
  }

  async initializeAnnotationPainter(
    waragraph: Waragraph,
    prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>,
  ) {
    this.annotation_painter =
      new AnnotationPainter(waragraph, prepareAnnotationRecords, this.file_name, this.records);

    await this.annotation_painter.prepareRecords();

    let prev_view = waragraph.graph_viewer?.getView();

    document.getElementById('viz-svg-overlay')!
      .append(this.annotation_painter.svg_root);

    // TODO: there are other cases when this should run, especially once
    // there's more than just 2D-focused SVG
    waragraph
      .graph_viewer?.view_subject
      .subscribe(async (view_2d) => {
        const { x, y, width, height } = view_2d;

        let update_pos = false;

        // add this back when paths' transforms are just updated on pan
        // if (prev_view.width !== width
        //     || prev_view.height !== height) {
        //     update_pos = true;

        //     this.annotation_painter.resample2DPaths(view_2d);
        // }

        if (prev_view?.x != x || prev_view?.y !== y || update_pos) {
          // update SVG path offsets;
          // should also happen on resample

          await this.annotation_painter!.resample2DPaths();
          await this.annotation_painter!.updateSVGPaths();
          prev_view = view_2d;
        }

      });

    const viewport = waragraph.global_viewport;

    viewport?.subject
      .pipe(rxjs.distinct(),
        rxjs.throttleTime(50),
      )
      .subscribe((view_1d) => {
        this.annotation_painter!.updateSVG1D(view_1d);
      });


  }

  recordAnnotationVizState(record_index: number) {
    return this.annotation_painter!.record_states[record_index];
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

      const viz_states = this.annotation_painter!.record_states;

      label_div.addEventListener('click', async (ev) => {
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

          await this.annotation_painter!.resample2DPaths();
          await this.annotation_painter!.updateSVGPaths();

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

async function loadBedFile(
  waragraph: Waragraph,
  prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>,
  file: File
) {
  const bed_file = new BEDFile(file.name);
  const bed_text = await file.text();

  const parser = new BED();
  const bed_lines = bed_text.split('\n').map(line => parser.parseLine(line));

  await bed_file.appendRecords(waragraph.graph, prepareAnnotationRecords, bed_lines);

  bed_file.initializeAnnotationPainter(waragraph, prepareAnnotationRecords);

  const bed_list = document.getElementById('bed-file-list');

  const bed_list_el = bed_file.createListElement();
  bed_list.append(bed_list_el);

}


async function bedSidebarPanel(
  waragraph: Waragraph,
  prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>,
) {
  const bed_pane = document.createElement('div');
  bed_pane.classList.add('bed-panel');

  const bed_list = document.createElement('div');
  bed_list.id = 'bed-file-list';

  bed_pane.append(bed_list);

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
      loadBedFile(waragraph, prepareAnnotationRecords, file);
    }
  });

  bed_pane.append(file_label);
  bed_pane.append(file_entry);
  bed_pane.append(file_button);

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
    let ctx_menu_el = document.getElementById(id)!;
    let ctx_open = ctx_menu_el.style.display === 'flex';
    if (!tgt.closest('#' + id) && ctx_open) {
      ctx_menu_el.style.setProperty('display', 'none');
      _context_menu_entry = null;
    }
  });

  return bed_pane;
}

// export async function initializeBedSidebarPanel(waragraph: Waragraph) {
export async function initializeBedSidebarPanel(
  waragraph: Waragraph,
  // prepareAnnotationRecords: (ranges: {path_id: number, start_bp: number, end_bp: number}) =>
  prepareAnnotationRecords: (intervals: PathInterval[]) => Promise<AnnotationGeometry[] | undefined>,
) {
  // waragraph_viz = warapi;

  // if (!wasm) {
  //   wasm = await init_module(undefined, waragraph.wasm.memory);
  //   wasm_bindgen.set_panic_hook();
  // }

  const pathMetadata = await waragraph.graph.pathMetadata();
  pathMetadata.forEach(path => {
    pathNamesMap.set(path.name, path.id);
  });

  document
    .getElementById('sidebar')!
    .append(await bedSidebarPanel(waragraph, prepareAnnotationRecords));
}
